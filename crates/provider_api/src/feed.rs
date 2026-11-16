//! Live market feed contract: demand in, normalized events out.
//!
//! The runtime owns session lifecycle, reconnect backoff, demand leases and
//! generation fencing. An adapter runs one connection on the calling thread
//! using the shared kit transports and emits only canonical events and
//! lifecycle evidence through a bounded sink. It keeps no canonical state
//! and makes no product decision.

use crate::error::ProviderError;
use aeris_domain::{ChartInterval, InstrumentDescriptor, MarketEvent};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::time::{Duration, Instant};

/// Which normalized streams one instrument demand carries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StreamSet {
    pub trades: bool,
    pub quotes: bool,
    pub depth: bool,
    /// Live candle updates at one interval, when requested.
    pub candle: Option<ChartInterval>,
}

impl StreamSet {
    /// Whether the demand carries no stream at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.trades && !self.quotes && !self.depth && self.candle.is_none()
    }
}

/// One instrument's demand inside a feed session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstrumentSubscription {
    pub instrument: InstrumentDescriptor,
    pub streams: StreamSet,
}

/// Everything one feed session carries. Demand is fixed for the session's
/// lifetime; a demand change opens a new session so every book receives a
/// fresh covering snapshot.
#[derive(Clone, Debug)]
pub struct FeedSessionSpec {
    /// Fenced session generation stamped into every event's metadata.
    pub generation: u64,
    pub subscriptions: Vec<InstrumentSubscription>,
}

/// One live candle replacement without canonical sequencing. The runtime
/// owns bar sequencing: the same open timestamp keeps the forming sequence
/// while a newer period extends it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveCandle {
    /// Candle-open timestamp in unix nanos.
    pub open_unix_nanos: i64,
    pub open: i64,
    pub high: i64,
    pub low: i64,
    pub close: i64,
    pub volume: i64,
}

/// Lifecycle and data evidence from one feed session.
#[derive(Clone, Debug)]
pub enum FeedEvent {
    /// A new session generation is starting with this demand.
    Started {
        generation: u64,
        subscriptions: Vec<InstrumentSubscription>,
    },
    /// The socket connected and subscriptions were sent.
    Connected,
    /// One canonical event ready for runtime ingestion.
    Event(MarketEvent),
    /// One live candle replacement for a subscribed instrument and interval.
    Candle {
        provider_id: String,
        instrument_id: String,
        entitlement_id: String,
        generation: u64,
        interval: ChartInterval,
        candle: LiveCandle,
    },
    /// The session ended; the runtime decides whether to reconnect.
    Ended(ProviderError),
}

/// Why a sink emission failed. Both ends are terminal for the session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeedSinkError {
    /// The runtime dropped the session.
    Closed,
    /// The bounded queue stayed full; a fresh session must re-snapshot.
    Full,
}

/// Bounded event channel from one session to the runtime. A session that
/// cannot deliver within the bound ends instead of growing an unbounded queue.
#[derive(Clone)]
pub struct FeedEventSink {
    sender: SyncSender<FeedEvent>,
    emission_bound: Duration,
}

impl FeedEventSink {
    #[must_use]
    pub const fn new(sender: SyncSender<FeedEvent>, emission_bound: Duration) -> Self {
        Self {
            sender,
            emission_bound,
        }
    }

    /// Delivers one event, waiting at most the emission bound for capacity.
    ///
    /// # Errors
    ///
    /// Returns `Closed` when the runtime is gone and `Full` when the queue
    /// stayed saturated for the whole bound.
    pub fn send(&self, event: FeedEvent) -> Result<(), FeedSinkError> {
        let started = Instant::now();
        let mut event = event;
        loop {
            match self.sender.try_send(event) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err(FeedSinkError::Closed),
                Err(TrySendError::Full(returned)) => {
                    if started.elapsed() >= self.emission_bound {
                        return Err(FeedSinkError::Full);
                    }
                    event = returned;
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }
}

/// Optional trait for providers with a live market feed.
pub trait MarketFeedProvider: Send + Sync {
    /// Runs one connection to completion on the calling thread: connect,
    /// subscribe the demand, translate frames into the sink until the peer
    /// closes, a failure occurs, or `stop` is raised. Reconnect backoff and
    /// generation fencing belong to the runtime, never to the adapter.
    fn run_session(&self, spec: FeedSessionSpec, sink: FeedEventSink, stop: Arc<AtomicBool>);
}
