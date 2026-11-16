//! Publication pump: canonical runtime evidence out, binary frames to the
//! shell. At most one frame per stream per flush; a consumer that falls
//! behind receives a fresh snapshot, never an unbounded queue.

use aeris_app_protocol::binary;
use aeris_app_protocol::views::SessionStateView;
use aeris_domain::{BarUpdate, ChartInterval, MarketBar, OrderBookPublication};
use aeris_market_runtime::{Publication, SessionState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::time::Duration;

/// One runtime publication tagged with its owning provider.
pub(crate) struct Tagged {
    pub provider_id: String,
    pub publication: Publication,
}

/// Service-to-pump evidence that does not flow through the market runtime.
pub(crate) enum PumpCommand {
    SeedBars {
        instrument_id: String,
        interval: ChartInterval,
        bars: Vec<MarketBar>,
    },
    Drop {
        instrument_id: String,
    },
    /// A fresh consumer attached: every stream owes it a full snapshot.
    Resync,
}

/// Flush cadence: one frame per stream at most every 8 ms.
const FLUSH_STEP: Duration = Duration::from_millis(8);
/// Closed bars retained per series before the head is trimmed and the
/// consumer is sent a fresh snapshot.
const MAXIMUM_CLOSED_BARS: usize = 5_000;

struct BookStream {
    current: OrderBookPublication,
    sent: Option<OrderBookPublication>,
    dirty: bool,
    force_snapshot: bool,
}

struct BarStream {
    closed: Vec<MarketBar>,
    forming: Option<MarketBar>,
    sent_closed: usize,
    sent_forming: Option<MarketBar>,
    generation: u64,
    dirty: bool,
    force_snapshot: bool,
    needs_reseed: bool,
    reseed_requested: bool,
}

struct SessionStream {
    state: SessionStateView,
    generation: u64,
    dirty: bool,
}

pub(crate) fn run(
    tagged: Receiver<Tagged>,
    commands: Receiver<PumpCommand>,
    reseeds: SyncSender<(String, ChartInterval)>,
    outbound: SyncSender<Vec<u8>>,
    shutdown: &AtomicBool,
) {
    let mut books: HashMap<String, BookStream> = HashMap::new();
    let mut bars: HashMap<(String, ChartInterval), BarStream> = HashMap::new();
    let mut sessions: HashMap<String, SessionStream> = HashMap::new();
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        match tagged.recv_timeout(FLUSH_STEP) {
            Ok(item) => ingest(item, &mut books, &mut bars, &mut sessions, &reseeds),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(command) = commands.try_recv() {
            match command {
                PumpCommand::SeedBars {
                    instrument_id,
                    interval,
                    bars: closed,
                } => {
                    let stream = bars
                        .entry((instrument_id, interval))
                        .or_insert_with(empty_bars);
                    stream.closed = closed;
                    stream.forming = None;
                    stream.sent_closed = 0;
                    stream.sent_forming = None;
                    stream.force_snapshot = true;
                    stream.dirty = true;
                    stream.needs_reseed = false;
                    stream.reseed_requested = false;
                }
                PumpCommand::Drop { instrument_id } => {
                    books.remove(&instrument_id);
                    bars.retain(|(id, _), _| *id != instrument_id);
                }
                PumpCommand::Resync => resync(&mut books, &mut bars, &mut sessions),
            }
        }
        flush(&mut books, &mut bars, &mut sessions, &reseeds, &outbound);
    }
}

fn empty_bars() -> BarStream {
    BarStream {
        closed: Vec::new(),
        forming: None,
        sent_closed: 0,
        sent_forming: None,
        generation: 0,
        dirty: false,
        force_snapshot: true,
        needs_reseed: false,
        reseed_requested: false,
    }
}

/// Marks every stream for a full snapshot on the next flush.
fn resync(
    books: &mut HashMap<String, BookStream>,
    bars: &mut HashMap<(String, ChartInterval), BarStream>,
    sessions: &mut HashMap<String, SessionStream>,
) {
    for stream in books.values_mut() {
        stream.force_snapshot = true;
        stream.dirty = true;
    }
    for stream in bars.values_mut() {
        stream.force_snapshot = true;
        stream.dirty = true;
    }
    for stream in sessions.values_mut() {
        stream.dirty = true;
    }
}

fn ingest(
    item: Tagged,
    books: &mut HashMap<String, BookStream>,
    bars: &mut HashMap<(String, ChartInterval), BarStream>,
    sessions: &mut HashMap<String, SessionStream>,
    reseeds: &SyncSender<(String, ChartInterval)>,
) {
    match item.publication {
        Publication::Session { generation, state } => {
            let view = match state {
                SessionState::Connecting => SessionStateView::Connecting,
                SessionState::Live => SessionStateView::Live,
                SessionState::Reconnecting { attempt } => {
                    SessionStateView::Reconnecting { attempt }
                }
                SessionState::Failed { message } => SessionStateView::Failed { message },
                SessionState::Stopped => SessionStateView::Stopped,
            };
            let stream = sessions.entry(item.provider_id).or_insert(SessionStream {
                state: view.clone(),
                generation,
                dirty: true,
            });
            stream.state = view;
            stream.generation = generation;
            stream.dirty = true;
        }
        Publication::Book(image) => {
            let stream = books
                .entry(image.instrument_id.clone())
                .or_insert(BookStream {
                    current: image.clone(),
                    sent: None,
                    dirty: true,
                    force_snapshot: true,
                });
            stream.current = image;
            stream.dirty = true;
        }
        Publication::Trade(_) | Publication::TradesDropped { .. } => {
            // Trade volume evidence already rides on the book images; the
            // tape stream joins when Aeris Charts consumes it.
        }
        Publication::Bar(update) => {
            let key = update.series().clone();
            let Some(interval) = interval_for_period(key.period) else {
                return;
            };
            let stream = bars
                .entry((key.instrument_id.clone(), interval))
                .or_insert_with(empty_bars);
            stream.generation = update.metadata().session_generation;
            match update {
                BarUpdate::Forming { bar, .. } => {
                    stream.forming = Some(bar);
                    stream.dirty = true;
                }
                BarUpdate::Completed { bar, .. } => {
                    let contiguous = stream
                        .closed
                        .last()
                        .is_none_or(|last| last.source_sequence + 1 == bar.source_sequence);
                    if !contiguous {
                        stream.force_snapshot = true;
                        stream.needs_reseed = true;
                        if reseeds
                            .try_send((key.instrument_id.clone(), interval))
                            .is_ok()
                        {
                            stream.reseed_requested = true;
                        }
                    }
                    stream.closed.push(bar);
                    if stream.closed.len() > MAXIMUM_CLOSED_BARS {
                        let excess = stream.closed.len() - MAXIMUM_CLOSED_BARS;
                        stream.closed.drain(..excess);
                        stream.sent_closed = 0;
                        stream.force_snapshot = true;
                    }
                    stream.dirty = true;
                }
            }
        }
        Publication::BarsResync {
            instrument_id,
            interval,
        } => {
            if let Some(stream) = bars.get_mut(&(instrument_id.clone(), interval)) {
                stream.force_snapshot = true;
                stream.needs_reseed = true;
                if reseeds.try_send((instrument_id.clone(), interval)).is_ok() {
                    stream.reseed_requested = true;
                }
            } else {
                let _ = reseeds.try_send((instrument_id, interval));
            }
        }
    }
}

fn interval_for_period(period: aeris_domain::BarPeriod) -> Option<ChartInterval> {
    ChartInterval::ALL.into_iter().find(|interval| {
        let candidate = match interval.aggregation() {
            aeris_domain::ChartAggregation::FixedSeconds(seconds) => {
                aeris_domain::BarPeriod::Time {
                    seconds: seconds.get(),
                }
            }
            aeris_domain::ChartAggregation::CalendarMonth => {
                aeris_domain::BarPeriod::Month { months: 1 }
            }
            aeris_domain::ChartAggregation::Trades(trades) => aeris_domain::BarPeriod::Tick {
                trades: trades.get(),
            },
        };
        candidate == period
            || (*interval == ChartInterval::Week1
                && period == aeris_domain::BarPeriod::Week { weeks: 1 })
    })
}

fn flush(
    books: &mut HashMap<String, BookStream>,
    bars: &mut HashMap<(String, ChartInterval), BarStream>,
    sessions: &mut HashMap<String, SessionStream>,
    reseeds: &SyncSender<(String, ChartInterval)>,
    outbound: &SyncSender<Vec<u8>>,
) {
    for (provider_id, stream) in sessions.iter_mut() {
        if !stream.dirty {
            continue;
        }
        let frame = binary::session_frame(provider_id, stream.generation, &stream.state);
        match outbound.try_send(frame) {
            Ok(()) => stream.dirty = false,
            Err(TrySendError::Full(_)) => return,
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
    for stream in books.values_mut() {
        if !stream.dirty {
            continue;
        }
        let snapshot_required = stream.force_snapshot
            || stream.sent.as_ref().is_none_or(|sent| {
                sent.session_generation != stream.current.session_generation
                    || stream.current.revision < sent.revision
            });
        if !snapshot_required && stream.sent.as_ref() == Some(&stream.current) {
            stream.dirty = false;
            continue;
        }
        let frame = match (&stream.sent, snapshot_required) {
            (Some(sent), false) => binary::book_delta_frame(&stream.current, sent),
            _ => binary::book_snapshot_frame(&stream.current),
        };
        match outbound.try_send(frame) {
            Ok(()) => {
                stream.sent = Some(stream.current.clone());
                stream.force_snapshot = false;
                stream.dirty = false;
            }
            Err(_) => {
                stream.force_snapshot = true;
                return;
            }
        }
    }
    for ((instrument_id, interval), stream) in bars.iter_mut() {
        if stream.needs_reseed {
            // Fresh history is on its way; ask once, then wait for the seed.
            if !stream.reseed_requested {
                if reseeds
                    .try_send((instrument_id.clone(), *interval))
                    .is_err()
                {
                    return;
                }
                stream.reseed_requested = true;
            }
            continue;
        }
        if !stream.dirty {
            continue;
        }
        let snapshot_required = stream.force_snapshot || stream.sent_closed > stream.closed.len();
        let completed = if snapshot_required {
            &[][..]
        } else {
            &stream.closed[stream.sent_closed..]
        };
        if !snapshot_required && completed.is_empty() && stream.forming == stream.sent_forming {
            stream.dirty = false;
            continue;
        }
        let frame = if snapshot_required {
            binary::bars_snapshot_frame(
                instrument_id,
                *interval,
                stream.generation,
                &stream.closed,
                stream.forming,
            )
        } else {
            binary::bars_update_frame(
                instrument_id,
                *interval,
                stream.generation,
                completed,
                stream.forming,
                false,
            )
        };
        match outbound.try_send(frame) {
            Ok(()) => {
                stream.sent_closed = stream.closed.len();
                stream.sent_forming = stream.forming;
                stream.force_snapshot = false;
                stream.dirty = false;
            }
            Err(_) => {
                stream.force_snapshot = true;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aeris_app_protocol::binary::{
        KIND_BOOK_DELTA, KIND_BOOK_SNAPSHOT, decode_bars_snapshot, decode_bars_update,
        decode_book_delta, decode_book_snapshot, parse_frame,
    };
    use aeris_domain::{
        AggressorTradeVolumes, BarPeriod, BarSeriesKey, DepthLevel, EventMetadata, OrderBookState,
        QualifiedTimestamp, QuoteLevel,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::mpsc;

    fn image(revision: u64, bid_quantity: i64) -> OrderBookPublication {
        OrderBookPublication {
            provider_id: "fixture".to_string(),
            instrument_id: "instrument:fixture:BTC".to_string(),
            entitlement_id: "fixture-public".to_string(),
            session_generation: 1,
            revision,
            source_watermark: revision,
            best_bid: Some(QuoteLevel {
                price: 100,
                quantity: Some(bid_quantity),
                order_count: None,
            }),
            best_ask: None,
            bbo_source_watermark: 0,
            bids: vec![DepthLevel {
                price: 100,
                quantity: bid_quantity,
                order_count: None,
            }],
            asks: vec![DepthLevel {
                price: 102,
                quantity: 4,
                order_count: None,
            }],
            traded_volumes: BTreeMap::new(),
            trade_source_watermark: 0,
            top_of_book_only: false,
            state: OrderBookState::Ready,
        }
    }

    fn tagged(publication: Publication) -> Tagged {
        Tagged {
            provider_id: "fixture".to_string(),
            publication,
        }
    }

    struct TestState {
        books: HashMap<String, BookStream>,
        bars: HashMap<(String, ChartInterval), BarStream>,
        sessions: HashMap<String, SessionStream>,
    }

    impl TestState {
        fn new() -> Self {
            Self {
                books: HashMap::new(),
                bars: HashMap::new(),
                sessions: HashMap::new(),
            }
        }

        fn ingest(
            &mut self,
            publication: Publication,
            reseeds: &SyncSender<(String, ChartInterval)>,
        ) {
            ingest(
                tagged(publication),
                &mut self.books,
                &mut self.bars,
                &mut self.sessions,
                reseeds,
            );
        }

        fn flush(
            &mut self,
            reseeds: &SyncSender<(String, ChartInterval)>,
            outbound: &SyncSender<Vec<u8>>,
        ) {
            flush(
                &mut self.books,
                &mut self.bars,
                &mut self.sessions,
                reseeds,
                outbound,
            );
        }

        fn resync(&mut self) {
            resync(&mut self.books, &mut self.bars, &mut self.sessions);
        }
    }

    #[test]
    fn book_stream_opens_with_a_snapshot_then_deltas() {
        let (outbound, receiver) = mpsc::sync_channel(8);
        let (reseeds, _reseed_receiver) = mpsc::sync_channel(4);
        let mut state = TestState::new();
        state.ingest(Publication::Book(image(1, 3)), &reseeds);
        state.flush(&reseeds, &outbound);
        let bytes = receiver.try_recv().expect("snapshot frame");
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_SNAPSHOT);
        let snapshot = decode_book_snapshot(frame.payload).expect("snapshot");
        assert_eq!(snapshot.bids[0].quantity, 3);

        state.ingest(Publication::Book(image(2, 8)), &reseeds);
        state.flush(&reseeds, &outbound);
        let bytes = receiver.try_recv().expect("delta frame");
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_DELTA);
        let delta = decode_book_delta(frame.payload).expect("delta");
        assert_eq!(delta.bid_changes.len(), 1);
        assert_eq!(delta.bid_changes[0].quantity, 8);
        assert!(delta.ask_changes.is_empty());
    }

    #[test]
    fn resync_owes_every_consumer_a_full_snapshot() {
        let (outbound, receiver) = mpsc::sync_channel(8);
        let (reseeds, _reseed_receiver) = mpsc::sync_channel(4);
        let mut state = TestState::new();
        state.ingest(Publication::Book(image(1, 3)), &reseeds);
        state.flush(&reseeds, &outbound);
        let _ = receiver.try_recv().expect("snapshot frame");
        state.ingest(Publication::Book(image(2, 8)), &reseeds);
        state.flush(&reseeds, &outbound);
        let bytes = receiver.try_recv().expect("delta frame");
        assert_eq!(parse_frame(&bytes).expect("frame").kind, KIND_BOOK_DELTA);

        state.resync();
        state.flush(&reseeds, &outbound);
        let bytes = receiver.try_recv().expect("resync frame");
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_SNAPSHOT);
        let snapshot = decode_book_snapshot(frame.payload).expect("snapshot");
        assert_eq!(snapshot.head.revision, 2);
        assert_eq!(snapshot.bids[0].quantity, 8);
    }

    #[test]
    fn a_slow_consumer_receives_a_fresh_snapshot() {
        let (outbound, receiver) = mpsc::sync_channel(1);
        let (reseeds, _reseed_receiver) = mpsc::sync_channel(4);
        let mut state = TestState::new();
        state.ingest(Publication::Book(image(1, 3)), &reseeds);
        state.flush(&reseeds, &outbound);
        // The consumer is absent: the next image cannot be delivered.
        state.ingest(Publication::Book(image(2, 8)), &reseeds);
        state.flush(&reseeds, &outbound);
        let first = receiver.try_recv().expect("first frame");
        assert_eq!(parse_frame(&first).expect("frame").kind, KIND_BOOK_SNAPSHOT);
        state.flush(&reseeds, &outbound);
        let bytes = receiver.try_recv().expect("recovery frame");
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.kind, KIND_BOOK_SNAPSHOT);
        let snapshot = decode_book_snapshot(frame.payload).expect("snapshot");
        assert_eq!(snapshot.head.revision, 2);
        assert_eq!(snapshot.bids[0].quantity, 8);
    }

    fn bar(sequence: u64, open_seconds: i64) -> MarketBar {
        MarketBar {
            source_sequence: sequence,
            exchange_timestamp_seconds: open_seconds,
            exchange_timestamp_unix_nanos: open_seconds * 1_000_000_000,
            open: 100,
            high: 110,
            low: 90,
            close: 105,
            volume: 10,
        }
    }

    fn bar_update(completed: bool, sequence: u64, open_seconds: i64) -> Publication {
        let series = BarSeriesKey {
            provider_id: "fixture".to_string(),
            instrument_id: "instrument:fixture:BTC".to_string(),
            entitlement_id: "fixture-public".to_string(),
            period: BarPeriod::Time { seconds: 60 },
            definition_version: 1,
        };
        let bar = bar(sequence, open_seconds);
        let metadata = EventMetadata {
            provider_id: "fixture".to_string(),
            instrument_id: "instrument:fixture:BTC".to_string(),
            entitlement_id: "fixture-public".to_string(),
            source_sequence: sequence,
            session_generation: 1,
            timestamps: QualifiedTimestamp {
                exchange_unix_nanos: Some(bar.exchange_timestamp_unix_nanos),
                provider_unix_nanos: Some(bar.exchange_timestamp_unix_nanos),
                received_unix_nanos: 1_700_000_000_000_000_000,
            },
        };
        Publication::Bar(if completed {
            BarUpdate::Completed {
                series,
                metadata,
                bar,
            }
        } else {
            BarUpdate::Forming {
                series,
                metadata,
                bar,
            }
        })
    }

    #[test]
    fn seeded_history_flows_as_snapshot_then_updates() {
        let (tagged_sender, tagged_receiver) = mpsc::sync_channel(16);
        let (command_sender, command_receiver) = mpsc::sync_channel(16);
        let (reseeds, _reseed_receiver) = mpsc::sync_channel(16);
        let (outbound, receiver) = mpsc::sync_channel(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let pump = std::thread::spawn({
            let shutdown = Arc::clone(&shutdown);
            move || {
                run(
                    tagged_receiver,
                    command_receiver,
                    reseeds,
                    outbound,
                    &shutdown,
                );
            }
        });
        command_sender
            .send(PumpCommand::SeedBars {
                instrument_id: "instrument:fixture:BTC".to_string(),
                interval: ChartInterval::Minute1,
                bars: vec![bar(1, 60)],
            })
            .expect("seed command");
        let bytes = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("bars snapshot");
        let frame = parse_frame(&bytes).expect("frame");
        assert_eq!(frame.key, "bars:instrument:fixture:BTC:1m");
        let snapshot = decode_bars_snapshot(frame.payload).expect("snapshot");
        assert_eq!(snapshot.closed.len(), 1);
        assert!(snapshot.forming.is_none());

        tagged_sender
            .send(tagged(bar_update(true, 2, 120)))
            .expect("completed");
        tagged_sender
            .send(tagged(bar_update(false, 3, 180)))
            .expect("forming");
        let mut completed = Vec::new();
        let mut forming = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while forming.is_none() && std::time::Instant::now() < deadline {
            let bytes = receiver
                .recv_timeout(Duration::from_millis(200))
                .expect("bars update");
            let frame = parse_frame(&bytes).expect("frame");
            let update = decode_bars_update(frame.payload).expect("update");
            completed.extend(update.completed);
            forming = update.forming;
        }
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].sequence, 2);
        assert_eq!(forming.expect("forming").sequence, 3);
        shutdown.store(true, Ordering::Release);
        pump.join().expect("pump joins");
    }

    #[test]
    fn session_frames_coalesce_to_the_latest_state() {
        let (tagged_sender, tagged_receiver) = mpsc::sync_channel(16);
        let (_command_sender, command_receiver) = mpsc::sync_channel(16);
        let (reseeds, _reseed_receiver) = mpsc::sync_channel(16);
        let (outbound, receiver) = mpsc::sync_channel(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let pump = std::thread::spawn({
            let shutdown = Arc::clone(&shutdown);
            move || {
                run(
                    tagged_receiver,
                    command_receiver,
                    reseeds,
                    outbound,
                    &shutdown,
                );
            }
        });
        tagged_sender
            .send(tagged(Publication::Session {
                generation: 1,
                state: SessionState::Connecting,
            }))
            .expect("connecting");
        tagged_sender
            .send(tagged(Publication::Session {
                generation: 1,
                state: SessionState::Live,
            }))
            .expect("live");
        let mut saw_live = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let Ok(bytes) = receiver.recv_timeout(Duration::from_millis(100)) else {
                break;
            };
            let frame = parse_frame(&bytes).expect("frame");
            assert_eq!(frame.key, "session:fixture");
            let decoded =
                aeris_app_protocol::binary::decode_session(frame.payload).expect("session");
            if decoded.state == 1 {
                saw_live = true;
            }
        }
        assert!(saw_live);
        shutdown.store(true, Ordering::Release);
        pump.join().expect("pump joins");
    }

    #[test]
    fn trade_volume_rides_on_book_images() {
        let mut publication = image(1, 3);
        publication.traded_volumes = [(100, AggressorTradeVolumes { buy: 5, sell: 2 })]
            .into_iter()
            .collect();
        let bytes = aeris_app_protocol::binary::book_snapshot_frame(&publication);
        let frame = parse_frame(&bytes).expect("frame");
        let snapshot = decode_book_snapshot(frame.payload).expect("snapshot");
        assert_eq!(snapshot.volumes, vec![(100, 5, 2)]);
    }
}
