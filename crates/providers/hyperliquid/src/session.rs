//! One multiplexed public WebSocket session.
//!
//! Connects, subscribes the session's fixed demand, and translates frames
//! into canonical events until the peer closes, a failure occurs, or the
//! runtime raises `stop`. Reconnect backoff and generation fencing belong to
//! the runtime; this loop only reports why the session ended.

use crate::book::{decode_bbo_quote, decode_book_snapshot};
use crate::candles::{chart_interval_for_hyperliquid, decode_live_candle};
use crate::trades::{DeliveredTradeIds, decode_trades_batch};
use crate::ws::{
    WsClientEvent, build_bbo_subscription, build_candle_subscription, build_l2_subscription,
    build_ping, build_trades_subscription, parse_ws_frame,
};
use crate::{ENTITLEMENT_ID, WS_URL};
use aeris_domain::{ChartInterval, MarketEvent};
use aeris_provider_api::ProviderError;
use aeris_provider_api::{FeedEvent, FeedEventSink, FeedSessionSpec, FeedSinkError, LiveCandle};
use aeris_provider_kit::{WebSocket, WsMessage, unix_nanos_now};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// JSON ping cadence documented by the provider.
const PING_INTERVAL: Duration = Duration::from_secs(20);
/// A feed silent this long is treated as broken.
const SILENCE_TIMEOUT: Duration = Duration::from_secs(45);
/// Longest single read wait, so cancellation is observed promptly.
const READ_POLL: Duration = Duration::from_millis(100);
/// Bounded handshake time.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Consecutive undecodable frames tolerated before the session fails closed.
const MAXIMUM_DECODE_FAILURES: u32 = 100;

/// Per-coin ingestion state: local sequence and replay deduplication.
struct CoinState {
    instrument_id: String,
    price_scale: u32,
    quantity_scale: u32,
    /// Next local ingestion sequence. Never presented as exchange continuity.
    sequence: u64,
    delivered: DeliveredTradeIds,
    candle_interval: Option<ChartInterval>,
}

impl CoinState {
    /// Reserves `count` local sequences, returning the first.
    fn take_sequence(&mut self, count: u64) -> Result<u64, ProviderError> {
        let first = self.sequence;
        self.sequence = first
            .checked_add(count)
            .ok_or_else(|| ProviderError::Schema("hyperliquid sequence overflowed".to_string()))?;
        Ok(first)
    }
}

pub(crate) fn run_session(spec: FeedSessionSpec, sink: FeedEventSink, stop: Arc<AtomicBool>) {
    let error = connect_and_pump(&spec, &sink, &stop)
        .err()
        .unwrap_or(ProviderError::Closed);
    let _ = sink.send(FeedEvent::Ended(error));
}

fn sink_error(error: FeedSinkError) -> ProviderError {
    match error {
        FeedSinkError::Closed => ProviderError::Closed,
        FeedSinkError::Full => {
            ProviderError::Transport("hyperliquid event queue stayed full".to_string())
        }
    }
}

fn connect_and_pump(
    spec: &FeedSessionSpec,
    sink: &FeedEventSink,
    stop: &AtomicBool,
) -> Result<(), ProviderError> {
    if spec.generation == 0 || spec.subscriptions.is_empty() {
        return Err(ProviderError::Refused(
            "hyperliquid session demand is invalid".to_string(),
        ));
    }
    let mut socket = WebSocket::connect(WS_URL, CONNECT_TIMEOUT)?;
    let mut coins: HashMap<String, CoinState> = HashMap::new();
    for subscription in &spec.subscriptions {
        let coin = subscription.instrument.symbol.clone();
        let streams = &subscription.streams;
        if streams.trades {
            socket.send_text(&build_trades_subscription(&coin).map_err(ProviderError::Refused)?)?;
        }
        if streams.quotes {
            socket.send_text(&build_bbo_subscription(&coin).map_err(ProviderError::Refused)?)?;
        }
        if streams.depth {
            socket.send_text(&build_l2_subscription(&coin).map_err(ProviderError::Refused)?)?;
        }
        if let Some(interval) = streams.candle {
            let interval_text = crate::candles::hyperliquid_interval_for_chart(interval)
                .map_err(ProviderError::Unsupported)?;
            socket.send_text(
                &build_candle_subscription(&coin, interval_text).map_err(ProviderError::Refused)?,
            )?;
        }
        coins.insert(
            coin,
            CoinState {
                instrument_id: subscription.instrument.instrument_id.as_str().to_string(),
                price_scale: u32::from(subscription.instrument.price_scale),
                quantity_scale: u32::from(subscription.instrument.quantity_scale),
                sequence: 1,
                delivered: DeliveredTradeIds::default(),
                candle_interval: streams.candle,
            },
        );
    }
    sink.send(FeedEvent::Connected).map_err(sink_error)?;

    let mut last_activity = Instant::now();
    let mut last_ping = Instant::now();
    let mut decode_failures: u32 = 0;
    loop {
        if stop.load(Ordering::Acquire) {
            socket.close();
            return Ok(());
        }
        let now = Instant::now();
        if now.duration_since(last_ping) >= PING_INTERVAL {
            socket.send_text(&build_ping())?;
            last_ping = now;
        }
        if now.duration_since(last_activity) >= SILENCE_TIMEOUT {
            socket.close();
            return Err(ProviderError::Transport(
                "hyperliquid feed went silent".to_string(),
            ));
        }
        let Some(message) = socket.recv(now + READ_POLL)? else {
            continue;
        };
        let received_unix_nanos = unix_nanos_now();
        last_activity = Instant::now();
        let text = match message {
            WsMessage::Text(text) => text,
            WsMessage::Close => {
                socket.close();
                return Err(ProviderError::Closed);
            }
        };
        let event = match parse_ws_frame(&text) {
            Ok(event) => {
                decode_failures = 0;
                event
            }
            Err(error) => {
                decode_failures += 1;
                if decode_failures >= MAXIMUM_DECODE_FAILURES {
                    return Err(ProviderError::Schema(error));
                }
                continue;
            }
        };
        let outcome = translate(
            event,
            &mut coins,
            spec.generation,
            received_unix_nanos,
            sink,
            &mut decode_failures,
        );
        match outcome {
            Ok(()) => {}
            Err(error) => {
                socket.close();
                return Err(error);
            }
        }
    }
}

fn translate(
    event: WsClientEvent,
    coins: &mut HashMap<String, CoinState>,
    generation: u64,
    received_unix_nanos: i64,
    sink: &FeedEventSink,
    decode_failures: &mut u32,
) -> Result<(), ProviderError> {
    let mut record_decode_failure = |error: String| -> Result<(), ProviderError> {
        *decode_failures += 1;
        if *decode_failures >= MAXIMUM_DECODE_FAILURES {
            return Err(ProviderError::Schema(error));
        }
        Ok(())
    };
    match event {
        WsClientEvent::Pong
        | WsClientEvent::Subscribed { .. }
        | WsClientEvent::Context { .. }
        | WsClientEvent::AllMids { .. } => Ok(()),
        WsClientEvent::Book { coin, book } => {
            let Some(state) = coins.get_mut(&coin) else {
                return Ok(());
            };
            let sequence = state.take_sequence(1)?;
            match decode_book_snapshot(
                &book,
                &coin,
                &state.instrument_id,
                ENTITLEMENT_ID,
                generation,
                sequence,
                received_unix_nanos,
            ) {
                Ok(decoded) => sink
                    .send(FeedEvent::Event(MarketEvent::DepthSnapshot(
                        decoded.snapshot,
                    )))
                    .map_err(sink_error),
                Err(error) => record_decode_failure(error),
            }
        }
        WsClientEvent::Bbo { coin, bbo } => {
            let Some(state) = coins.get_mut(&coin) else {
                return Ok(());
            };
            let sequence = state.take_sequence(1)?;
            match decode_bbo_quote(
                &bbo,
                &coin,
                &state.instrument_id,
                ENTITLEMENT_ID,
                generation,
                sequence,
                received_unix_nanos,
            ) {
                Ok(quote) => sink
                    .send(FeedEvent::Event(MarketEvent::Quote(quote)))
                    .map_err(sink_error),
                Err(error) => record_decode_failure(error),
            }
        }
        WsClientEvent::Trades { coin, trades } => {
            let Some(state) = coins.get_mut(&coin) else {
                return Ok(());
            };
            let first_sequence = state.take_sequence(1)?;
            match decode_trades_batch(
                &trades,
                &coin,
                &state.instrument_id,
                ENTITLEMENT_ID,
                generation,
                received_unix_nanos,
                first_sequence,
            ) {
                Ok(batch) => {
                    let count = u64::try_from(batch.trades.len()).unwrap_or(u64::MAX);
                    // Reserve the remaining batch sequences; the first was reserved above.
                    state.take_sequence(count.saturating_sub(1))?;
                    let mut trades = batch.trades;
                    state.delivered.retain_undelivered(&mut trades);
                    for trade in trades {
                        sink.send(FeedEvent::Event(MarketEvent::Trade(trade)))
                            .map_err(sink_error)?;
                    }
                    Ok(())
                }
                Err(error) => record_decode_failure(error),
            }
        }
        WsClientEvent::Candle {
            coin,
            interval,
            candles,
        } => {
            let Some(state) = coins.get_mut(&coin) else {
                return Ok(());
            };
            let Some(wanted) = state.candle_interval else {
                return Ok(());
            };
            if chart_interval_for_hyperliquid(&interval) != Some(wanted) {
                return Ok(());
            }
            match decode_live_candle(&candles, state.price_scale, state.quantity_scale) {
                Ok(live) => sink
                    .send(FeedEvent::Candle {
                        provider_id: crate::PROVIDER_ID.to_string(),
                        instrument_id: state.instrument_id.clone(),
                        entitlement_id: ENTITLEMENT_ID.to_string(),
                        generation,
                        interval: wanted,
                        candle: LiveCandle {
                            open_unix_nanos: live.open_nanos,
                            open: live.open,
                            high: live.high,
                            low: live.low,
                            close: live.close,
                            volume: live.volume,
                        },
                    })
                    .map_err(sink_error),
                Err(error) => record_decode_failure(error),
            }
        }
    }
}
