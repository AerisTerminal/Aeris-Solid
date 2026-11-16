//! Generic market runtime: session lifecycle, demand leases, generation
//! fencing, canonical books and candles. Knows no provider by name; behavior
//! follows the adapter contract and capability declarations.

mod ingest;

pub use ingest::{Publication, SessionState};

use aeris_domain::{ChartInterval, MarketBar};
use aeris_provider_api::{
    FeedEvent, FeedEventSink, FeedSessionSpec, InstrumentSubscription, MarketFeedProvider,
};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// Runtime tuning bounds.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig {
    /// Retained aggregate depth levels per side.
    pub depth_levels: NonZeroUsize,
    /// Capacity of the bounded feed queue.
    pub feed_capacity: usize,
    /// Longest one feed emission waits for queue capacity.
    pub emission_bound: Duration,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            depth_levels: NonZeroUsize::new(20).unwrap_or(NonZeroUsize::MIN),
            feed_capacity: 4_096,
            emission_bound: Duration::from_secs(2),
        }
    }
}

/// Reconnect decision from the ingestion state machine to the session loop.
pub(crate) enum Decision {
    Retry(Duration),
    Halt,
}

/// Handle to a running market runtime.
pub struct MarketRuntime {
    commands: Sender<ingest::Command>,
    demand: Arc<Mutex<Vec<InstrumentSubscription>>>,
    current_stop: Arc<Mutex<Arc<AtomicBool>>>,
    wake: Arc<AtomicBool>,
    demand_changed: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    session_thread: Option<JoinHandle<()>>,
    ingest_thread: Option<JoinHandle<()>>,
}

impl MarketRuntime {
    /// Starts the session and ingestion threads. Publications flow to
    /// `publisher`; a slow consumer receives fresh full images, never an
    /// unbounded queue.
    #[must_use]
    pub fn start(
        provider: Arc<dyn MarketFeedProvider>,
        config: RuntimeConfig,
        publisher: SyncSender<Publication>,
    ) -> Self {
        let (feed_sender, feed_receiver) = mpsc::sync_channel(config.feed_capacity);
        let (commands, command_receiver) = mpsc::channel();
        let (decision_sender, decision_receiver) = mpsc::channel();
        let demand = Arc::new(Mutex::new(Vec::new()));
        let current_stop = Arc::new(Mutex::new(Arc::new(AtomicBool::new(false))));
        let wake = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        let demand_changed = Arc::new(AtomicBool::new(false));

        let session_thread = std::thread::spawn({
            let demand = Arc::clone(&demand);
            let current_stop = Arc::clone(&current_stop);
            let wake = Arc::clone(&wake);
            let shutdown = Arc::clone(&shutdown);
            let connected = Arc::clone(&connected);
            move || {
                session_loop(
                    provider,
                    feed_sender,
                    config.emission_bound,
                    &demand,
                    &current_stop,
                    &wake,
                    &shutdown,
                    &connected,
                    &decision_receiver,
                );
            }
        });
        let ingest_thread = std::thread::spawn({
            let shutdown = Arc::clone(&shutdown);
            let connected = Arc::clone(&connected);
            let demand_changed = Arc::clone(&demand_changed);
            move || {
                ingest::run(
                    feed_receiver,
                    command_receiver,
                    decision_sender,
                    publisher,
                    config.depth_levels,
                    ingest::Flags {
                        connected: &connected,
                        demand_changed: &demand_changed,
                        shutdown: &shutdown,
                    },
                );
            }
        });
        Self {
            commands,
            demand,
            current_stop,
            wake,
            demand_changed,
            shutdown,
            session_thread: Some(session_thread),
            ingest_thread: Some(ingest_thread),
        }
    }

    /// Replaces the whole demand lease. The current session ends and a new
    /// generation opens with fresh covering snapshots for every instrument.
    pub fn set_demand(&self, subscriptions: Vec<InstrumentSubscription>) {
        *self.demand.lock().unwrap_or_else(PoisonError::into_inner) = subscriptions;
        self.demand_changed.store(true, Ordering::Release);
        self.raise_stop();
        self.wake.store(true, Ordering::Release);
    }

    /// Seeds closed history for one live candle series, fetched by the
    /// composition root through the provider's history contract.
    pub fn seed_bars(&self, instrument_id: &str, interval: ChartInterval, bars: Vec<MarketBar>) {
        let _ = self.commands.send(ingest::Command::SeedBars {
            instrument_id: instrument_id.to_string(),
            interval,
            bars,
        });
    }

    /// Stops both threads and waits for them to leave.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.raise_stop();
        self.wake.store(true, Ordering::Release);
        let _ = self.commands.send(ingest::Command::Shutdown);
        if let Some(thread) = self.session_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.ingest_thread.take() {
            let _ = thread.join();
        }
    }

    fn raise_stop(&self) {
        self.current_stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .store(true, Ordering::Release);
    }
}

impl Drop for MarketRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Poll step so shutdown and demand changes interrupt waits promptly.
const WAIT_STEP: Duration = Duration::from_millis(50);

#[allow(clippy::too_many_arguments)]
fn session_loop(
    provider: Arc<dyn MarketFeedProvider>,
    feed_sender: SyncSender<FeedEvent>,
    emission_bound: Duration,
    demand: &Mutex<Vec<InstrumentSubscription>>,
    current_stop: &Mutex<Arc<AtomicBool>>,
    wake: &AtomicBool,
    shutdown: &AtomicBool,
    connected: &AtomicBool,
    decisions: &Receiver<Decision>,
) {
    let mut generation: u64 = 0;
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        while decisions.try_recv().is_ok() {}
        let subscriptions = demand
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if subscriptions.is_empty() {
            std::thread::sleep(WAIT_STEP);
            continue;
        }
        generation = generation.saturating_add(1).max(1);
        let stop = Arc::new(AtomicBool::new(false));
        *current_stop.lock().unwrap_or_else(PoisonError::into_inner) = Arc::clone(&stop);
        // A demand change that landed before this stop was stored raised the
        // previous stop; honor it on the fresh one so no session outlives its
        // lease.
        let latest = demand
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if latest != subscriptions {
            stop.store(true, Ordering::Release);
        }
        let sink = FeedEventSink::new(feed_sender.clone(), emission_bound);
        let _ = sink.send(FeedEvent::Started {
            generation,
            subscriptions: subscriptions.clone(),
        });
        connected.store(false, Ordering::Release);
        provider.run_session(
            FeedSessionSpec {
                generation,
                subscriptions,
            },
            sink,
            stop,
        );
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let Some(decision) = wait_decision(decisions, wake, shutdown) else {
            return;
        };
        match decision {
            Decision::Halt => return,
            Decision::Retry(delay) => {
                if !interruptible_sleep(delay, wake, shutdown) {
                    return;
                }
            }
        }
    }
}

/// Waits for the ingestion decision, leaving early on shutdown or a demand
/// change. A lost decision (full feed queue) defaults to a prompt retry.
fn wait_decision(
    decisions: &Receiver<Decision>,
    wake: &AtomicBool,
    shutdown: &AtomicBool,
) -> Option<Decision> {
    loop {
        match decisions.recv_timeout(WAIT_STEP) {
            Ok(decision) => return Some(decision),
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if shutdown.load(Ordering::Acquire) {
                    return None;
                }
                if wake.swap(false, Ordering::AcqRel) {
                    return Some(Decision::Retry(Duration::ZERO));
                }
            }
        }
    }
}

/// Sleeps in bounded steps; returns false when the runtime is shutting down.
fn interruptible_sleep(delay: Duration, wake: &AtomicBool, shutdown: &AtomicBool) -> bool {
    let started = std::time::Instant::now();
    while started.elapsed() < delay {
        if shutdown.load(Ordering::Acquire) {
            return false;
        }
        if wake.swap(false, Ordering::AcqRel) {
            return true;
        }
        std::thread::sleep(WAIT_STEP.min(delay.saturating_sub(started.elapsed())));
    }
    !shutdown.load(Ordering::Acquire)
}
