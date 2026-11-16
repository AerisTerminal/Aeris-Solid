//! Composition root: builds the runtimes, registers providers, routes UI
//! commands and batches per-frame binary updates. The shells (Tauri, web
//! worker) carry no logic; they forward commands and bytes.

mod providers;
mod pump;
mod service;

use aeris_app_protocol::{Command, CommandError, CommandErrorKind, CommandResult};
use aeris_market_runtime::{MarketRuntime, Publication, RuntimeConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Handle to the running application backend.
pub struct AppCore {
    requests: SyncSender<service::UiRequest>,
    pump_commands: SyncSender<pump::PumpCommand>,
    shutdown: Arc<AtomicBool>,
    runtimes: Vec<Arc<MarketRuntime>>,
    threads: Vec<JoinHandle<()>>,
}

impl AppCore {
    /// Starts the backend. Binary frames flow to `outbound`; a slow shell
    /// coalesces to fresh snapshots per stream.
    #[must_use]
    pub fn start(outbound: SyncSender<Vec<u8>>) -> Self {
        let providers = providers::registered();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (requests, request_receiver) = mpsc::sync_channel(64);
        let (pump_commands, pump_command_receiver) = mpsc::sync_channel(64);
        let (tagged_sender, tagged_receiver) = mpsc::sync_channel(1_024);
        let (reseeds, reseed_receiver) = mpsc::sync_channel(16);

        let mut runtimes: Vec<Arc<MarketRuntime>> = Vec::new();
        let mut runtime_map: HashMap<usize, Arc<MarketRuntime>> = HashMap::new();
        let mut threads: Vec<JoinHandle<()>> = Vec::new();
        for (index, provider) in providers.iter().enumerate() {
            let Some(feed) = &provider.feed else {
                continue;
            };
            let (publisher, publication_receiver) = mpsc::sync_channel(1_024);
            let runtime = Arc::new(MarketRuntime::start(
                Arc::clone(feed),
                RuntimeConfig::default(),
                publisher,
            ));
            let provider_id = provider.capabilities.provider_id.clone();
            let tagged_sender = tagged_sender.clone();
            let shutdown = Arc::clone(&shutdown);
            threads.push(std::thread::spawn(move || {
                forward(publication_receiver, tagged_sender, provider_id, &shutdown);
            }));
            runtime_map.insert(index, Arc::clone(&runtime));
            runtimes.push(runtime);
        }
        drop(tagged_sender);

        threads.push(std::thread::spawn({
            let shutdown = Arc::clone(&shutdown);
            let pump_commands = pump_commands.clone();
            move || {
                service::run(
                    request_receiver,
                    reseed_receiver,
                    pump_commands,
                    providers,
                    runtime_map,
                    shutdown,
                );
            }
        }));
        threads.push(std::thread::spawn({
            let shutdown = Arc::clone(&shutdown);
            move || {
                pump::run(
                    tagged_receiver,
                    pump_command_receiver,
                    reseeds,
                    outbound,
                    &shutdown,
                );
            }
        }));

        Self {
            requests,
            pump_commands,
            shutdown,
            runtimes,
            threads,
        }
    }

    /// Executes one typed UI command, waiting for its typed answer.
    ///
    /// # Errors
    ///
    /// Returns a typed command failure; never branches on provider text.
    pub fn execute(&self, command: Command) -> Result<CommandResult, CommandError> {
        let (reply, answer) = mpsc::channel();
        let mut request = service::UiRequest { command, reply };
        let started = Instant::now();
        loop {
            match self.requests.try_send(request) {
                Ok(()) => break,
                Err(TrySendError::Full(returned)) => {
                    if started.elapsed() >= Duration::from_secs(2) {
                        return Err(CommandError::new(
                            CommandErrorKind::Internal,
                            "command queue stayed saturated",
                        ));
                    }
                    request = returned;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err(CommandError::new(
                        CommandErrorKind::Internal,
                        "backend is gone",
                    ));
                }
            }
        }
        answer
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|_| {
                Err(CommandError::new(
                    CommandErrorKind::Internal,
                    "command timed out",
                ))
            })
    }

    /// Tells the pump that a fresh consumer attached: every stream owes it
    /// a full snapshot. Best effort under saturation; if the bounded command
    /// queue stays full for two seconds the request is dropped and the
    /// pump's own snapshot-on-full recovery still applies.
    pub fn resync(&self) {
        let started = Instant::now();
        loop {
            match self.pump_commands.try_send(pump::PumpCommand::Resync) {
                Ok(()) => return,
                Err(TrySendError::Full(_)) => {
                    if started.elapsed() >= Duration::from_secs(2) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }

    /// Stops every thread and waits for it to leave.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
        // The service thread held the other runtime references; dropping
        // these last ones joins the runtime threads through Drop.
        self.runtimes.clear();
    }
}

impl Drop for AppCore {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Tags one runtime's publications with its provider and hands them to the
/// pump, escaping promptly on shutdown.
fn forward(
    publications: Receiver<Publication>,
    tagged: SyncSender<pump::Tagged>,
    provider_id: String,
    shutdown: &AtomicBool,
) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let publication = match publications.recv_timeout(Duration::from_millis(50)) {
            Ok(publication) => publication,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        let mut item = pump::Tagged {
            provider_id: provider_id.clone(),
            publication,
        };
        loop {
            match tagged.try_send(item) {
                Ok(()) => break,
                Err(TrySendError::Full(returned)) => {
                    if shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    item = returned;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }
}
