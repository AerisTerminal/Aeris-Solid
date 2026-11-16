//! Canonical ingestion: books, quotes, traded volumes and live candles.
//!
//! One thread owns all canonical state. Publications are full images on a
//! bounded channel: a slow consumer coalesces to the latest image and is
//! told explicitly when evidence was dropped, never fed an unbounded queue.

use crate::Decision;
use aeris_domain::{
    AggressorTradeVolumes, BarPeriod, BarSeriesKey, BarUpdate, ChartAggregation, ChartInterval,
    EventMetadata, MarketBar, MarketEvent, MarketTrade, OrderBook, OrderBookApplyOutcome,
    OrderBookPublication, QualifiedTimestamp, QuoteLevel, TopOfBookQuote, merge_live_candle,
};
use aeris_provider_api::{FeedEvent, InstrumentSubscription, LiveCandle, ProviderError};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Session lifecycle evidence for the UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionState {
    Connecting,
    Live,
    Reconnecting { attempt: u32 },
    Failed { message: String },
    Stopped,
}

/// One bounded publication to the composition root.
#[derive(Clone, Debug)]
pub enum Publication {
    Session {
        generation: u64,
        state: SessionState,
    },
    Book(OrderBookPublication),
    Trade(MarketTrade),
    /// Trades the bounded channel could not carry; their volume still
    /// reached the book. The tape shows an explicit gap, never silent loss.
    TradesDropped {
        instrument_id: String,
        count: u64,
    },
    Bar(BarUpdate),
    /// Closed-bar evidence overflowed; the consumer must refetch history.
    BarsResync {
        instrument_id: String,
        interval: ChartInterval,
    },
}

pub(crate) enum Command {
    SeedBars {
        instrument_id: String,
        interval: ChartInterval,
        bars: Vec<MarketBar>,
    },
    Shutdown,
}

/// Longest reconnect backoff; a live connection resets it.
const MAXIMUM_BACKOFF: Duration = Duration::from_secs(10);
/// Base reconnect backoff, doubled per consecutive failed attempt.
const BASE_BACKOFF: Duration = Duration::from_millis(250);
/// Completed bars held for a slow consumer before demanding a resync.
const MAXIMUM_PENDING_COMPLETIONS: usize = 64;
/// Poll step for command intake between feed events.
const FEED_POLL: Duration = Duration::from_millis(50);

struct BookState {
    book: OrderBook,
    best_bid: Option<QuoteLevel>,
    best_ask: Option<QuoteLevel>,
    bbo_source_watermark: u64,
    volumes: BTreeMap<i64, AggressorTradeVolumes>,
    trade_source_watermark: u64,
    dropped_trades: u64,
    dirty: bool,
}

impl BookState {
    fn new(depth_levels: NonZeroUsize) -> Self {
        Self {
            book: OrderBook::new(depth_levels),
            best_bid: None,
            best_ask: None,
            bbo_source_watermark: 0,
            volumes: BTreeMap::new(),
            trade_source_watermark: 0,
            dropped_trades: 0,
            dirty: false,
        }
    }

    fn image(&self) -> OrderBookPublication {
        let mut image = self.book.publication();
        image.best_bid = self.best_bid;
        image.best_ask = self.best_ask;
        image.bbo_source_watermark = self.bbo_source_watermark;
        image.trade_source_watermark = self.trade_source_watermark;
        let retained: Vec<i64> = image
            .bids
            .iter()
            .chain(image.asks.iter())
            .map(|level| level.price)
            .collect();
        image.traded_volumes = self
            .volumes
            .iter()
            .filter(|(price, _)| retained.contains(price))
            .map(|(price, volume)| (*price, *volume))
            .collect();
        image
    }
}

struct CandleSeries {
    provider_id: String,
    entitlement_id: String,
    closed: Vec<MarketBar>,
    forming: Option<MarketBar>,
    pending_completed: VecDeque<MarketBar>,
    forming_dirty: bool,
    resync: bool,
}

impl CandleSeries {
    fn empty() -> Self {
        Self {
            provider_id: String::new(),
            entitlement_id: String::new(),
            closed: Vec::new(),
            forming: None,
            pending_completed: VecDeque::new(),
            forming_dirty: false,
            resync: false,
        }
    }
}

struct State {
    depth_levels: NonZeroUsize,
    publisher: SyncSender<Publication>,
    generation: u64,
    demanded: Vec<InstrumentSubscription>,
    books: HashMap<String, BookState>,
    series: HashMap<(String, ChartInterval), CandleSeries>,
    attempt: u32,
    pending_session: Option<Publication>,
}

pub(crate) struct Flags<'a> {
    pub connected: &'a AtomicBool,
    pub demand_changed: &'a AtomicBool,
    pub shutdown: &'a AtomicBool,
}

pub(crate) fn run(
    feed: Receiver<FeedEvent>,
    commands: Receiver<Command>,
    decisions: Sender<Decision>,
    publisher: SyncSender<Publication>,
    depth_levels: NonZeroUsize,
    flags: Flags<'_>,
) {
    let mut state = State {
        depth_levels,
        publisher,
        generation: 0,
        demanded: Vec::new(),
        books: HashMap::new(),
        series: HashMap::new(),
        attempt: 0,
        pending_session: None,
    };
    loop {
        if flags.shutdown.load(Ordering::Acquire) {
            break;
        }
        match feed.recv_timeout(FEED_POLL) {
            Ok(event) => state.handle_feed(event, &decisions, &flags),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let mut halt = false;
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::SeedBars {
                    instrument_id,
                    interval,
                    bars,
                } => state.seed_bars(&instrument_id, interval, bars),
                Command::Shutdown => {
                    halt = true;
                    break;
                }
            }
        }
        if halt {
            break;
        }
        state.flush();
    }
    state.publish_session(SessionState::Stopped);
    state.flush();
}

impl State {
    fn handle_feed(&mut self, event: FeedEvent, decisions: &Sender<Decision>, flags: &Flags<'_>) {
        match event {
            FeedEvent::Started {
                generation,
                subscriptions,
            } => self.started(generation, subscriptions, flags),
            FeedEvent::Connected => {
                flags.connected.store(true, Ordering::Release);
                self.attempt = 0;
                self.publish_session(SessionState::Live);
            }
            FeedEvent::Event(event) => self.ingest_event(event),
            FeedEvent::Candle {
                provider_id,
                instrument_id,
                entitlement_id,
                generation,
                interval,
                candle,
            } => {
                if generation == self.generation {
                    self.ingest_candle(
                        &provider_id,
                        &instrument_id,
                        &entitlement_id,
                        generation,
                        interval,
                        candle,
                    );
                }
            }
            FeedEvent::Ended(error) => self.ended(error, decisions, flags),
        }
    }

    fn started(
        &mut self,
        generation: u64,
        subscriptions: Vec<InstrumentSubscription>,
        flags: &Flags<'_>,
    ) {
        self.generation = generation;
        self.demanded = subscriptions;
        flags.connected.store(false, Ordering::Release);
        // The demand this session carries is now in force; only a later
        // change may shortcut the next reconnect backoff.
        flags.demand_changed.swap(false, Ordering::AcqRel);
        let demanded_ids: Vec<&str> = self
            .demanded
            .iter()
            .map(|subscription| subscription.instrument.instrument_id.as_str())
            .collect();
        self.books
            .retain(|id, _| demanded_ids.contains(&id.as_str()));
        self.series
            .retain(|(id, _), _| demanded_ids.contains(&id.as_str()));
        for id in &demanded_ids {
            let book = self
                .books
                .entry((*id).to_string())
                .or_insert_with(|| BookState::new(self.depth_levels));
            book.book.mark_stale();
            book.dirty = true;
        }
        self.publish_session(SessionState::Connecting);
    }

    fn ended(&mut self, error: ProviderError, decisions: &Sender<Decision>, flags: &Flags<'_>) {
        flags.connected.store(false, Ordering::Release);
        for book in self.books.values_mut() {
            book.book.mark_stale();
            book.dirty = true;
        }
        if flags.shutdown.load(Ordering::Acquire) {
            let _ = decisions.send(Decision::Halt);
            return;
        }
        if flags.demand_changed.swap(false, Ordering::AcqRel) {
            let _ = decisions.send(Decision::Retry(Duration::ZERO));
            return;
        }
        if !error.reconnectable() {
            self.publish_session(SessionState::Failed {
                message: error.to_string(),
            });
            let _ = decisions.send(Decision::Halt);
            return;
        }
        self.attempt = self.attempt.saturating_add(1);
        let delay = BASE_BACKOFF
            .saturating_mul(2_u32.saturating_pow(self.attempt.saturating_sub(1)))
            .min(MAXIMUM_BACKOFF);
        self.publish_session(SessionState::Reconnecting {
            attempt: self.attempt,
        });
        let _ = decisions.send(Decision::Retry(delay));
    }

    fn ingest_event(&mut self, event: MarketEvent) {
        if event.metadata().session_generation != self.generation {
            return;
        }
        let instrument_id = event.metadata().instrument_id.clone();
        match event {
            MarketEvent::DepthSnapshot(snapshot) => {
                let Some(book) = self.books.get_mut(&instrument_id) else {
                    return;
                };
                match book.book.install_snapshot(&snapshot) {
                    Ok(OrderBookApplyOutcome::IgnoredStale) => {}
                    Ok(_) => book.dirty = true,
                    Err(error) => {
                        tracing::warn!(%error, instrument = %instrument_id, "depth snapshot rejected");
                        book.dirty = true;
                    }
                }
            }
            MarketEvent::DepthDelta(delta) => {
                let Some(book) = self.books.get_mut(&instrument_id) else {
                    return;
                };
                match book.book.apply_delta(&delta) {
                    Ok(OrderBookApplyOutcome::IgnoredStale) => {}
                    Ok(_) => book.dirty = true,
                    Err(error) => {
                        tracing::warn!(%error, instrument = %instrument_id, "depth delta rejected");
                        book.dirty = true;
                    }
                }
            }
            MarketEvent::Quote(quote) => self.ingest_quote(&instrument_id, quote),
            MarketEvent::Trade(trade) => self.ingest_trade(&instrument_id, trade),
            MarketEvent::OrderLevels(_) => {
                tracing::debug!(instrument = %instrument_id, "order-level data has no runtime consumer yet");
            }
        }
    }

    fn ingest_quote(&mut self, instrument_id: &str, quote: TopOfBookQuote) {
        let Some(book) = self.books.get_mut(instrument_id) else {
            return;
        };
        if quote.metadata.source_sequence <= book.bbo_source_watermark {
            return;
        }
        book.bbo_source_watermark = quote.metadata.source_sequence;
        if let Some(bid) = quote.bid {
            book.best_bid = Some(bid);
        }
        if let Some(ask) = quote.ask {
            book.best_ask = Some(ask);
        }
        book.dirty = true;
    }

    fn ingest_trade(&mut self, instrument_id: &str, trade: MarketTrade) {
        let Some(book) = self.books.get_mut(instrument_id) else {
            return;
        };
        if trade.metadata.source_sequence <= book.trade_source_watermark {
            return;
        }
        book.trade_source_watermark = trade.metadata.source_sequence;
        let volume = book.volumes.entry(trade.price).or_default();
        match trade.aggressor {
            aeris_domain::AggressorSide::Buy => {
                volume.buy = volume.buy.saturating_add(trade.quantity)
            }
            aeris_domain::AggressorSide::Sell => {
                volume.sell = volume.sell.saturating_add(trade.quantity);
            }
            aeris_domain::AggressorSide::Unknown => {}
        }
        book.dirty = true;
        // Volume evidence stays bounded to the retained price lattice.
        if book.volumes.len() > 4 * self.depth_levels.get() {
            let prices: Vec<i64> = book
                .book
                .bid_levels()
                .chain(book.book.ask_levels())
                .map(|level| level.price)
                .collect();
            book.volumes.retain(|price, _| prices.contains(price));
        }
        match self.publisher.try_send(Publication::Trade(trade)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                book.dropped_trades = book.dropped_trades.saturating_add(1)
            }
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    fn ingest_candle(
        &mut self,
        provider_id: &str,
        instrument_id: &str,
        entitlement_id: &str,
        generation: u64,
        interval: ChartInterval,
        candle: LiveCandle,
    ) {
        let series = self
            .series
            .entry((instrument_id.to_string(), interval))
            .or_insert_with(CandleSeries::empty);
        series.provider_id = provider_id.to_string();
        series.entitlement_id = entitlement_id.to_string();
        let sequence = match &series.forming {
            Some(open) if open.exchange_timestamp_unix_nanos == candle.open_unix_nanos => {
                open.source_sequence
            }
            Some(_) => series
                .closed
                .last()
                .map_or(0, |bar| bar.source_sequence)
                .saturating_add(2),
            None => series
                .closed
                .last()
                .map_or(0, |bar| bar.source_sequence)
                .saturating_add(1),
        };
        let update = MarketBar {
            source_sequence: sequence,
            exchange_timestamp_seconds: candle.open_unix_nanos.div_euclid(1_000_000_000),
            exchange_timestamp_unix_nanos: candle.open_unix_nanos,
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
        };
        let rolled = match merge_live_candle(&mut series.closed, &mut series.forming, update) {
            Ok(rolled) => rolled,
            Err(error) => {
                tracing::warn!(%error, instrument = %instrument_id, "live candle rejected");
                return;
            }
        };
        let key = BarSeriesKey {
            provider_id: provider_id.to_string(),
            instrument_id: instrument_id.to_string(),
            entitlement_id: entitlement_id.to_string(),
            period: period_for(interval),
            definition_version: 1,
        };
        if rolled && let Some(completed) = series.closed.last().copied() {
            let update = BarUpdate::Completed {
                series: key.clone(),
                metadata: bar_metadata(&key, generation, completed),
                bar: completed,
            };
            match self.publisher.try_send(Publication::Bar(update)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    series.pending_completed.push_back(completed);
                    if series.pending_completed.len() > MAXIMUM_PENDING_COMPLETIONS {
                        series.pending_completed.clear();
                        series.resync = true;
                    }
                }
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
        if let Some(forming) = series.forming {
            let update = BarUpdate::Forming {
                metadata: bar_metadata(&key, generation, forming),
                series: key,
                bar: forming,
            };
            match self.publisher.try_send(Publication::Bar(update)) {
                Ok(()) => series.forming_dirty = false,
                Err(TrySendError::Full(_)) => series.forming_dirty = true,
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }

    fn seed_bars(&mut self, instrument_id: &str, interval: ChartInterval, bars: Vec<MarketBar>) {
        let mut closed = Vec::with_capacity(bars.len());
        for (index, mut bar) in bars.into_iter().enumerate() {
            if bar.validate().is_err() {
                tracing::warn!(instrument = %instrument_id, "seed history rejected");
                return;
            }
            if closed.last().is_some_and(|last: &MarketBar| {
                last.exchange_timestamp_unix_nanos >= bar.exchange_timestamp_unix_nanos
            }) {
                tracing::warn!(instrument = %instrument_id, "seed history is not ordered");
                return;
            }
            bar.source_sequence = u64::try_from(index + 1).unwrap_or(u64::MAX);
            closed.push(bar);
        }
        let series = self
            .series
            .entry((instrument_id.to_string(), interval))
            .or_insert_with(CandleSeries::empty);
        series.closed = closed;
        series.forming = None;
        series.pending_completed.clear();
        series.resync = false;
    }

    fn publish_session(&mut self, state: SessionState) {
        let publication = Publication::Session {
            generation: self.generation,
            state,
        };
        match self.publisher.try_send(publication.clone()) {
            Ok(()) => self.pending_session = None,
            Err(TrySendError::Full(_)) => self.pending_session = Some(publication),
            Err(TrySendError::Disconnected(_)) => {}
        }
    }

    /// Retries coalesced evidence while the consumer has capacity. Books and
    /// forming bars are full images, so the latest one heals any earlier drop.
    fn flush(&mut self) {
        if let Some(publication) = self.pending_session.take() {
            match self.publisher.try_send(publication.clone()) {
                Ok(()) => {}
                Err(_) => {
                    self.pending_session = Some(publication);
                    return;
                }
            }
        }
        let book_ids: Vec<String> = self.books.keys().cloned().collect();
        for id in book_ids {
            let Some(book) = self.books.get_mut(&id) else {
                continue;
            };
            if book.dropped_trades > 0 {
                let count = book.dropped_trades;
                match self.publisher.try_send(Publication::TradesDropped {
                    instrument_id: id.clone(),
                    count,
                }) {
                    Ok(()) => book.dropped_trades = 0,
                    Err(_) => return,
                }
            }
            if !book.dirty {
                continue;
            }
            let image = book.image();
            match self.publisher.try_send(Publication::Book(image)) {
                Ok(()) => book.dirty = false,
                Err(_) => return,
            }
        }
        let keys: Vec<(String, ChartInterval)> = self.series.keys().cloned().collect();
        for key in keys {
            let Some(series) = self.series.get_mut(&key) else {
                continue;
            };
            if series.resync {
                match self.publisher.try_send(Publication::BarsResync {
                    instrument_id: key.0.clone(),
                    interval: key.1,
                }) {
                    Ok(()) => series.resync = false,
                    Err(_) => return,
                }
            }
            while let Some(completed) = series.pending_completed.front().copied() {
                let series_key = BarSeriesKey {
                    provider_id: series.provider_id.clone(),
                    instrument_id: key.0.clone(),
                    entitlement_id: series.entitlement_id.clone(),
                    period: period_for(key.1),
                    definition_version: 1,
                };
                let update = BarUpdate::Completed {
                    series: series_key.clone(),
                    metadata: bar_metadata(&series_key, self.generation, completed),
                    bar: completed,
                };
                match self.publisher.try_send(Publication::Bar(update)) {
                    Ok(()) => {
                        series.pending_completed.pop_front();
                    }
                    Err(_) => return,
                }
            }
            if series.forming_dirty
                && let Some(forming) = series.forming
            {
                let series_key = BarSeriesKey {
                    provider_id: series.provider_id.clone(),
                    instrument_id: key.0.clone(),
                    entitlement_id: series.entitlement_id.clone(),
                    period: period_for(key.1),
                    definition_version: 1,
                };
                let update = BarUpdate::Forming {
                    series: series_key.clone(),
                    metadata: bar_metadata(&series_key, self.generation, forming),
                    bar: forming,
                };
                match self.publisher.try_send(Publication::Bar(update)) {
                    Ok(()) => series.forming_dirty = false,
                    Err(_) => return,
                }
            }
        }
    }
}

fn period_for(interval: ChartInterval) -> BarPeriod {
    match interval {
        ChartInterval::Tick100 => BarPeriod::Tick { trades: 100 },
        ChartInterval::Week1 => BarPeriod::Week { weeks: 1 },
        ChartInterval::Month1 => BarPeriod::Month { months: 1 },
        other => match other.aggregation() {
            ChartAggregation::FixedSeconds(seconds) => BarPeriod::Time {
                seconds: seconds.get(),
            },
            ChartAggregation::CalendarMonth => BarPeriod::Month { months: 1 },
            ChartAggregation::Trades(trades) => BarPeriod::Tick {
                trades: trades.get(),
            },
        },
    }
}

fn bar_metadata(key: &BarSeriesKey, generation: u64, bar: MarketBar) -> EventMetadata {
    EventMetadata {
        provider_id: key.provider_id.clone(),
        instrument_id: key.instrument_id.clone(),
        entitlement_id: key.entitlement_id.clone(),
        source_sequence: bar.source_sequence,
        session_generation: generation,
        timestamps: QualifiedTimestamp {
            exchange_unix_nanos: Some(bar.exchange_timestamp_unix_nanos),
            provider_unix_nanos: Some(bar.exchange_timestamp_unix_nanos),
            received_unix_nanos: unix_nanos_now(),
        },
    }
}

fn unix_nanos_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
