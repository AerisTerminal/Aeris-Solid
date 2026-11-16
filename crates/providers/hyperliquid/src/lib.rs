//! Hyperliquid public market-data adapter: translation only.
//!
//! Declares capabilities, decodes the public WebSocket and info endpoint
//! into canonical fixed-point events, and keeps no canonical state. Session
//! lifecycle, reconnect backoff, books and candles belong to the runtime.

mod book;
mod candles;
mod catalog;
mod decimal;
mod http;
mod session;
mod trades;
mod ws;

pub use book::{
    DecodedBook, MAXIMUM_HYPERLIQUID_BOOK_LEVELS, decode_bbo_quote, decode_book_snapshot,
};
pub use candles::{
    HyperliquidCandlePage, HyperliquidLiveCandle, MAXIMUM_HYPERLIQUID_CANDLES,
    chart_interval_for_hyperliquid, decode_candle_page, decode_live_candle,
    hyperliquid_interval_for_chart, hyperliquid_interval_for_period,
    period_for_hyperliquid_interval,
};
pub use decimal::{
    FUNDING_RATE_SCALE, MAXIMUM_HYPERLIQUID_DECIMALS, NORMALIZED_PRICE_SCALE,
    NORMALIZED_QUANTITY_SCALE, NOTIONAL_SCALE, RawDecimal, funding_for_rate,
    parse_aggregate_decimal_to_fixed, parse_decimal_to_fixed, price_increment_near,
    quantity_for_size, scale_for_market,
};
pub use trades::{DeliveredTradeIds, HyperliquidTradeBatch, decode_trades_batch};
pub use ws::{
    WsClientEvent, build_aggregated_l2_subscription, build_bbo_subscription,
    build_candle_subscription, build_l2_subscription, build_ping, build_trades_subscription,
    build_unsubscribe, parse_ws_frame,
};

use aeris_domain::{AssetClass, ChartAggregation, ChartInterval, InstrumentDescriptor, MarketBar};
use aeris_provider_api::{
    BuildTargets, Catalog, FeedEventSink, FeedSessionSpec, History, MarketFeedProvider,
    ProviderCapabilities, ProviderError, RateBudgetDeclaration, SignInMethod, StreamCapabilities,
};
use aeris_provider_kit::{RateBudget, RateBudgetConfig, RequestPriority};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, PoisonError, RwLock};
use std::time::{Duration, Instant};

/// Canonical provider identity stamped into every event.
pub const PROVIDER_ID: &str = "hyperliquid";
/// Public data entitlement; no account is involved.
pub const ENTITLEMENT_ID: &str = "hyperliquid-public";
/// Public multiplexed WebSocket endpoint for market-data subscriptions.
pub const WS_URL: &str = "wss://api.hyperliquid.xyz/ws";

/// How long one decoded catalog stays fresh before the next search refetches.
const CATALOG_TTL: Duration = Duration::from_secs(300);

/// The Hyperliquid adapter: one shared request budget, one cached catalog.
pub struct HyperliquidProvider {
    http: http::InfoClient,
    catalog: RwLock<Option<(Instant, Vec<catalog::CatalogEntry>)>>,
}

impl HyperliquidProvider {
    #[must_use]
    pub fn new() -> Self {
        let declaration = Self::capabilities().rate_budget;
        let budget = Arc::new(RateBudget::new(RateBudgetConfig {
            window: Duration::from_secs(u64::from(declaration.window_seconds)),
            window_weight: declaration.window_weight,
            foreground_reserve: declaration.foreground_reserve,
        }));
        Self {
            http: http::InfoClient::new(budget),
            catalog: RwLock::new(None),
        }
    }

    /// Everything the runtime and UI may know without naming the provider.
    #[must_use]
    pub fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Hyperliquid".to_string(),
            asset_classes: vec![AssetClass::CryptoPerp],
            streams: StreamCapabilities {
                trades: true,
                quotes: true,
                aggregate_depth_levels: Some(MAXIMUM_HYPERLIQUID_BOOK_LEVELS),
                order_level_instrument_cap: None,
            },
            history_intervals: vec![
                ChartInterval::Minute1,
                ChartInterval::Minute3,
                ChartInterval::Minute5,
                ChartInterval::Minute15,
                ChartInterval::Minute30,
                ChartInterval::Hour1,
                ChartInterval::Hour2,
                ChartInterval::Hour4,
                ChartInterval::Hour8,
                ChartInterval::Hour12,
                ChartInterval::Day1,
                ChartInterval::Day3,
                ChartInterval::Week1,
                ChartInterval::Month1,
            ],
            sign_in: SignInMethod::None,
            trading_venue: false,
            rate_budget: RateBudgetDeclaration {
                window_seconds: 60,
                window_weight: 1_000,
                foreground_reserve: 300,
            },
            targets: BuildTargets {
                native: true,
                web: false,
            },
        }
    }

    fn catalog_entries(&self) -> Result<Vec<catalog::CatalogEntry>, ProviderError> {
        if let Some((fetched_at, entries)) = self
            .catalog
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            && fetched_at.elapsed() < CATALOG_TTL
        {
            return Ok(entries.clone());
        }
        let response = self.http.post_json(
            &serde_json::json!({"type": "metaAndAssetCtxs"}),
            RequestPriority::Interactive,
            None,
        )?;
        let entries = catalog::decode_catalog(&response).map_err(ProviderError::Schema)?;
        *self.catalog.write().unwrap_or_else(PoisonError::into_inner) =
            Some((Instant::now(), entries.clone()));
        Ok(entries)
    }
}

impl Default for HyperliquidProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Catalog for HyperliquidProvider {
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<InstrumentDescriptor>, ProviderError> {
        let entries = self.catalog_entries()?;
        Ok(catalog::search(&entries, query, limit))
    }
}

impl History for HyperliquidProvider {
    fn bars(
        &self,
        instrument: &InstrumentDescriptor,
        interval: ChartInterval,
        start_unix_nanos: i64,
        end_unix_nanos: i64,
        limit: usize,
    ) -> Result<Vec<MarketBar>, ProviderError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        hyperliquid_interval_for_chart(interval).map_err(ProviderError::Unsupported)?;
        let seconds = match interval.aggregation() {
            ChartAggregation::FixedSeconds(seconds) => i64::from(seconds.get()),
            ChartAggregation::CalendarMonth => 30 * 86_400,
            ChartAggregation::Trades(_) => {
                return Err(ProviderError::Unsupported(
                    "hyperliquid history is unavailable for this interval".to_string(),
                ));
            }
        };
        let end_millis = end_unix_nanos.div_euclid(1_000_000);
        let window_millis = i64::try_from(limit)
            .unwrap_or(i64::MAX)
            .saturating_mul(seconds)
            .saturating_mul(1_000);
        let start_millis = start_unix_nanos
            .div_euclid(1_000_000)
            .max(end_millis.saturating_sub(window_millis))
            .max(0);
        if end_millis <= start_millis {
            return Err(ProviderError::Refused(
                "hyperliquid candle window is empty".to_string(),
            ));
        }
        let now_millis = aeris_provider_kit::unix_nanos_now().div_euclid(1_000_000);
        let stop = AtomicBool::new(false);
        let page = self.http.candle_snapshot(
            instrument,
            interval,
            start_millis,
            end_millis,
            now_millis,
            &stop,
        )?;
        let mut bars = page.bars;
        if bars.len() > limit {
            bars = bars.split_off(bars.len() - limit);
        }
        Ok(bars)
    }
}

impl MarketFeedProvider for HyperliquidProvider {
    fn run_session(&self, spec: FeedSessionSpec, sink: FeedEventSink, stop: Arc<AtomicBool>) {
        session::run_session(spec, sink, stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeris_domain::{InstrumentId, MarketEvent};
    use aeris_provider_api::{FeedEvent, InstrumentSubscription, StreamSet};
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;

    #[test]
    fn capabilities_declare_the_public_perp_surface() {
        let capabilities = HyperliquidProvider::capabilities();
        assert_eq!(capabilities.provider_id, PROVIDER_ID);
        assert_eq!(capabilities.streams.aggregate_depth_levels, Some(20));
        assert_eq!(capabilities.streams.order_level_instrument_cap, None);
        assert_eq!(capabilities.sign_in, SignInMethod::None);
        assert!(
            capabilities
                .history_intervals
                .contains(&ChartInterval::Minute1)
        );
        assert!(
            !capabilities
                .history_intervals
                .contains(&ChartInterval::Second1)
        );
    }

    #[test]
    #[ignore = "drives the live Hyperliquid public WebSocket"]
    fn live_session_streams_book_trades_and_candles() {
        let instrument = InstrumentDescriptor {
            instrument_id: InstrumentId::new("instrument:hyperliquid:BTC").expect("identity"),
            provider_id: PROVIDER_ID.to_string(),
            symbol: "BTC".to_string(),
            display: "BTC-USDC".to_string(),
            exchange: "Hyperliquid".to_string(),
            asset_class: AssetClass::CryptoPerp,
            price_scale: 8,
            quantity_scale: 8,
            price_increment: None,
            quantity_increment: None,
        };
        let spec = FeedSessionSpec {
            generation: 1,
            subscriptions: vec![InstrumentSubscription {
                instrument,
                streams: StreamSet {
                    trades: true,
                    quotes: true,
                    depth: true,
                    candle: Some(ChartInterval::Minute1),
                },
            }],
        };
        let (sender, receiver) = mpsc::sync_channel(1024);
        let sink = FeedEventSink::new(sender, Duration::from_secs(2));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            HyperliquidProvider::new().run_session(spec, sink, worker_stop);
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut books = 0;
        let mut trades = 0;
        let mut quotes = 0;
        let mut candles = 0;
        while books < 3 || trades < 1 || quotes < 1 || candles < 1 {
            assert!(Instant::now() < deadline, "live session timed out");
            match receiver.recv_timeout(Duration::from_secs(5)) {
                Ok(FeedEvent::Event(MarketEvent::DepthSnapshot(snapshot))) => {
                    assert!(!snapshot.bids.is_empty() && !snapshot.asks.is_empty());
                    assert!(snapshot.bids.len() <= MAXIMUM_HYPERLIQUID_BOOK_LEVELS);
                    books += 1;
                }
                Ok(FeedEvent::Event(MarketEvent::Trade(_))) => trades += 1,
                Ok(FeedEvent::Event(MarketEvent::Quote(_))) => quotes += 1,
                Ok(FeedEvent::Candle { interval, .. }) => {
                    assert_eq!(interval, ChartInterval::Minute1);
                    candles += 1;
                }
                Ok(FeedEvent::Ended(error)) => panic!("live session ended: {error}"),
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => panic!("session worker left"),
            }
        }
        stop.store(true, Ordering::Release);
        worker.join().expect("session worker joins");
        eprintln!(
            "live session: {books} books, {trades} trades, {quotes} quotes, {candles} candles"
        );
    }
}
