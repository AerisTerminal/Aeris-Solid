//! Runtime behavior against a scripted provider: lifecycle, fencing,
//! reconnect, book images and live candle sequencing.

use aeris_domain::{
    AggressorSide, AssetClass, BarUpdate, ChartInterval, DepthLevel, DepthSnapshot, EventMetadata,
    InstrumentDescriptor, InstrumentId, MarketBar, MarketEvent, MarketTrade, OrderBookState,
    QualifiedTimestamp, QuoteLevel, TopOfBookQuote,
};
use aeris_market_runtime::{MarketRuntime, Publication, RuntimeConfig, SessionState};
use aeris_provider_api::{
    FeedEvent, FeedEventSink, FeedSessionSpec, InstrumentSubscription, LiveCandle,
    MarketFeedProvider, ProviderError, StreamSet,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

const INSTRUMENT: &str = "instrument:fixture:BTC";
const PROVIDER: &str = "fixture";
const ENTITLEMENT: &str = "fixture-public";
const NANOS: i64 = 1_000_000_000;

struct ScriptedProvider {
    scripts: Mutex<VecDeque<Vec<FeedEvent>>>,
}

impl ScriptedProvider {
    fn new(scripts: Vec<Vec<FeedEvent>>) -> Arc<Self> {
        Arc::new(Self {
            scripts: Mutex::new(scripts.into()),
        })
    }
}

impl MarketFeedProvider for ScriptedProvider {
    fn run_session(&self, _spec: FeedSessionSpec, sink: FeedEventSink, stop: Arc<AtomicBool>) {
        // Let command intake (history seeds) settle before scripted events.
        std::thread::sleep(Duration::from_millis(100));
        let script = self
            .scripts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_default();
        let mut ended = false;
        for event in script {
            ended |= matches!(event, FeedEvent::Ended(_));
            if sink.send(event).is_err() {
                return;
            }
        }
        if ended {
            return;
        }
        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = sink.send(FeedEvent::Ended(ProviderError::Closed));
    }
}

fn descriptor() -> InstrumentDescriptor {
    InstrumentDescriptor {
        instrument_id: InstrumentId::new(INSTRUMENT).expect("identity"),
        provider_id: PROVIDER.to_string(),
        symbol: "BTC".to_string(),
        display: "BTC-USDC".to_string(),
        exchange: "Fixture".to_string(),
        asset_class: AssetClass::CryptoPerp,
        price_scale: 8,
        quantity_scale: 8,
        price_increment: Some(1),
        quantity_increment: Some(1),
    }
}

fn subscription() -> InstrumentSubscription {
    InstrumentSubscription {
        instrument: descriptor(),
        streams: StreamSet {
            trades: true,
            quotes: true,
            depth: true,
            candle: Some(ChartInterval::Minute1),
        },
    }
}

fn metadata(sequence: u64, generation: u64) -> EventMetadata {
    EventMetadata {
        provider_id: PROVIDER.to_string(),
        instrument_id: INSTRUMENT.to_string(),
        entitlement_id: ENTITLEMENT.to_string(),
        source_sequence: sequence,
        session_generation: generation,
        timestamps: QualifiedTimestamp {
            exchange_unix_nanos: Some(sequence as i64 * NANOS),
            provider_unix_nanos: Some(sequence as i64 * NANOS),
            received_unix_nanos: 1_700_000_000 * NANOS,
        },
    }
}

fn snapshot(sequence: u64, generation: u64) -> FeedEvent {
    FeedEvent::Event(MarketEvent::DepthSnapshot(DepthSnapshot {
        metadata: metadata(sequence, generation),
        bids: vec![DepthLevel {
            price: 100,
            quantity: 3,
            order_count: None,
        }],
        asks: vec![DepthLevel {
            price: 102,
            quantity: 4,
            order_count: None,
        }],
    }))
}

fn candle(open_seconds: i64, close: i64) -> FeedEvent {
    FeedEvent::Candle {
        provider_id: PROVIDER.to_string(),
        instrument_id: INSTRUMENT.to_string(),
        entitlement_id: ENTITLEMENT.to_string(),
        generation: 1,
        interval: ChartInterval::Minute1,
        candle: LiveCandle {
            open_unix_nanos: open_seconds * NANOS,
            open: 100,
            high: 110,
            low: 90,
            close,
            volume: 10,
        },
    }
}

fn bar(open_seconds: i64) -> MarketBar {
    MarketBar {
        source_sequence: 1,
        exchange_timestamp_seconds: open_seconds,
        exchange_timestamp_unix_nanos: open_seconds * NANOS,
        open: 100,
        high: 110,
        low: 90,
        close: 105,
        volume: 10,
    }
}

fn start(provider: Arc<ScriptedProvider>) -> (MarketRuntime, Receiver<Publication>) {
    let (publisher, publications) = sync_channel(256);
    let runtime = MarketRuntime::start(provider, RuntimeConfig::default(), publisher);
    (runtime, publications)
}

fn next(publications: &Receiver<Publication>) -> Publication {
    publications
        .recv_timeout(Duration::from_secs(5))
        .expect("publication within deadline")
}

fn wait_for(
    publications: &Receiver<Publication>,
    predicate: impl Fn(&Publication) -> bool,
) -> Publication {
    loop {
        let publication = next(publications);
        if predicate(&publication) {
            return publication;
        }
    }
}

fn is_ready_book(publication: &Publication) -> bool {
    matches!(
        publication,
        Publication::Book(book) if book.state == OrderBookState::Ready && !book.bids.is_empty()
    )
}

#[test]
fn snapshot_quote_and_trade_reach_a_ready_book_image() {
    let provider = ScriptedProvider::new(vec![vec![
        FeedEvent::Connected,
        snapshot(1, 1),
        FeedEvent::Event(MarketEvent::Quote(TopOfBookQuote {
            metadata: metadata(2, 1),
            bid: Some(QuoteLevel {
                price: 100,
                quantity: Some(3),
                order_count: None,
            }),
            ask: Some(QuoteLevel {
                price: 102,
                quantity: Some(4),
                order_count: None,
            }),
        })),
        FeedEvent::Event(MarketEvent::Trade(MarketTrade {
            metadata: metadata(3, 1),
            trade_id: "trade-1".to_string(),
            price: 100,
            quantity: 5,
            aggressor: AggressorSide::Buy,
        })),
    ]]);
    let (runtime, publications) = start(provider);
    runtime.set_demand(vec![subscription()]);

    wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Session {
                state: SessionState::Live,
                ..
            }
        )
    });
    wait_for(
        &publications,
        |publication| matches!(publication, Publication::Trade(trade) if trade.trade_id == "trade-1"),
    );
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Book(book)
                if book.state == OrderBookState::Ready
                    && book.best_bid.is_some_and(|bid| bid.price == 100)
                    && book.traded_volumes.get(&100).is_some_and(|volume| volume.buy == 5)
        )
    });
    let Publication::Book(book) = publication else {
        unreachable!("predicate matched a book")
    };
    assert_eq!(book.session_generation, 1);
    assert_eq!(book.bbo_source_watermark, 2);
    assert_eq!(book.trade_source_watermark, 3);
    runtime.shutdown();
}

#[test]
fn events_from_other_generations_are_fenced_out() {
    let provider = ScriptedProvider::new(vec![vec![
        FeedEvent::Connected,
        snapshot(1, 1),
        FeedEvent::Event(MarketEvent::DepthDelta(aeris_domain::DepthDelta {
            metadata: metadata(2, 1),
            side: aeris_domain::BookSide::Bid,
            level: DepthLevel {
                price: 99,
                quantity: 7,
                order_count: None,
            },
        })),
        FeedEvent::Event(MarketEvent::DepthDelta(aeris_domain::DepthDelta {
            metadata: metadata(3, 5),
            side: aeris_domain::BookSide::Bid,
            level: DepthLevel {
                price: 98,
                quantity: 9,
                order_count: None,
            },
        })),
    ]]);
    let (runtime, publications) = start(provider);
    runtime.set_demand(vec![subscription()]);

    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Book(book)
                if book.state == OrderBookState::Ready && book.bids.len() == 2
        )
    });
    let Publication::Book(book) = publication else {
        unreachable!("predicate matched a book")
    };
    assert!(book.bids.iter().all(|level| level.price != 98));
    assert_eq!(book.source_watermark, 2);
    runtime.shutdown();
}

#[test]
fn transport_failure_reconnects_with_a_fresh_snapshot() {
    let provider = ScriptedProvider::new(vec![
        vec![
            FeedEvent::Connected,
            snapshot(1, 1),
            FeedEvent::Ended(ProviderError::Transport("fixture drop".to_string())),
        ],
        vec![FeedEvent::Connected, snapshot(1, 2)],
    ]);
    let (runtime, publications) = start(provider);
    runtime.set_demand(vec![subscription()]);

    wait_for(&publications, is_ready_book);
    wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Session {
                state: SessionState::Reconnecting { attempt: 1 },
                ..
            }
        )
    });
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Book(book)
                if book.state == OrderBookState::Ready && book.session_generation == 2
        )
    });
    let Publication::Book(book) = publication else {
        unreachable!("predicate matched a book")
    };
    assert!(!book.bids.is_empty());
    runtime.shutdown();
}

#[test]
fn permanent_failure_halts_without_reconnect() {
    let provider = ScriptedProvider::new(vec![vec![FeedEvent::Ended(ProviderError::Schema(
        "fixture schema".to_string(),
    ))]]);
    let (runtime, publications) = start(provider);
    runtime.set_demand(vec![subscription()]);

    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Session {
                state: SessionState::Failed { .. },
                ..
            }
        )
    });
    let Publication::Session {
        state: SessionState::Failed { message },
        ..
    } = publication
    else {
        unreachable!("predicate matched a failed session")
    };
    assert!(message.contains("schema"));
    // Halt must let shutdown join both threads promptly.
    runtime.shutdown();
}

#[test]
fn seeded_history_merges_live_candles_into_completed_and_forming() {
    let provider = ScriptedProvider::new(vec![vec![
        FeedEvent::Connected,
        candle(120, 105),
        candle(120, 107),
        candle(180, 101),
    ]]);
    let (runtime, publications) = start(provider);
    runtime.seed_bars(INSTRUMENT, ChartInterval::Minute1, vec![bar(60)]);
    runtime.set_demand(vec![subscription()]);

    // Same-open replacement keeps the forming sequence.
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Bar(BarUpdate::Forming { bar, .. })
                if bar.exchange_timestamp_unix_nanos == 120 * NANOS && bar.close == 107
        )
    });
    let Publication::Bar(update) = publication else {
        unreachable!("predicate matched a bar")
    };
    assert_eq!(update.bar().source_sequence, 2);
    assert_eq!(update.validate(), Ok(()));

    // Rollover completes the open period and opens the next one.
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Bar(BarUpdate::Completed { bar, .. })
                if bar.exchange_timestamp_unix_nanos == 120 * NANOS
        )
    });
    let Publication::Bar(completed) = publication else {
        unreachable!("predicate matched a bar")
    };
    assert_eq!(completed.bar().source_sequence, 2);
    assert_eq!(completed.bar().close, 107);
    assert_eq!(completed.validate(), Ok(()));
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Bar(BarUpdate::Forming { bar, .. })
                if bar.exchange_timestamp_unix_nanos == 180 * NANOS
        )
    });
    let Publication::Bar(forming) = publication else {
        unreachable!("predicate matched a bar")
    };
    assert_eq!(forming.bar().source_sequence, 3);
    assert_eq!(forming.validate(), Ok(()));
    runtime.shutdown();
}

#[test]
fn demand_change_opens_a_new_generation() {
    let provider = ScriptedProvider::new(vec![
        vec![FeedEvent::Connected, snapshot(1, 1)],
        vec![FeedEvent::Connected, snapshot(1, 2)],
    ]);
    let (runtime, publications) = start(provider);
    runtime.set_demand(vec![subscription()]);
    wait_for(&publications, is_ready_book);

    runtime.set_demand(vec![subscription()]);
    let publication = wait_for(&publications, |publication| {
        matches!(
            publication,
            Publication::Book(book)
                if book.state == OrderBookState::Ready && book.session_generation == 2
        )
    });
    let Publication::Book(book) = publication else {
        unreachable!("predicate matched a book")
    };
    assert!(!book.bids.is_empty());
    runtime.shutdown();
}

#[test]
fn slow_consumer_coalesces_to_the_latest_book_image() {
    let mut script = vec![FeedEvent::Connected, snapshot(1, 1)];
    for sequence in 2..=40_u64 {
        script.push(FeedEvent::Event(MarketEvent::DepthDelta(
            aeris_domain::DepthDelta {
                metadata: metadata(sequence, 1),
                side: aeris_domain::BookSide::Bid,
                level: DepthLevel {
                    price: 100,
                    quantity: i64::try_from(sequence).unwrap_or(i64::MAX),
                    order_count: None,
                },
            },
        )));
    }
    let provider = ScriptedProvider::new(vec![script]);
    let (publisher, publications) = sync_channel(1);
    let runtime = MarketRuntime::start(provider, RuntimeConfig::default(), publisher);
    runtime.set_demand(vec![subscription()]);

    // Let the whole script land while the consumer is absent, then read.
    std::thread::sleep(Duration::from_millis(500));
    let publication = wait_for(&publications, is_ready_book);
    let Publication::Book(book) = publication else {
        unreachable!("predicate matched a book")
    };
    assert_eq!(book.state, OrderBookState::Ready);
    assert!(book.source_watermark >= 2);
    runtime.shutdown();
}
