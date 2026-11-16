//! The market bridge: typed commands in, binary frames out. No logic lives
//! here; `app_core` owns every decision.

use aeris_app_core::AppCore;
use aeris_app_protocol::{Command, CommandError, CommandErrorKind, CommandResult};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

/// Backend handle plus the currently attached UI frame channel.
pub struct MarketBridge {
    core: Arc<AppCore>,
    ui: Arc<Mutex<Option<Channel<InvokeResponseBody>>>>,
}

impl MarketBridge {
    /// Starts the backend and the frame forwarder. Startup invariant: the
    /// process cannot run without them.
    pub fn start() -> Self {
        let (outbound, frames) = std::sync::mpsc::sync_channel(1_024);
        let core = Arc::new(AppCore::start(outbound));
        let ui: Arc<Mutex<Option<Channel<InvokeResponseBody>>>> = Arc::new(Mutex::new(None));
        std::thread::spawn({
            let ui = Arc::clone(&ui);
            move || forward_frames(frames, &ui)
        });
        Self { core, ui }
    }
}

/// Executes one typed UI command on a blocking worker, away from the async
/// runtime and the UI thread.
#[tauri::command]
pub async fn market_command(
    command: Command,
    state: State<'_, MarketBridge>,
) -> Result<CommandResult, CommandError> {
    let core = Arc::clone(&state.core);
    tauri::async_runtime::spawn_blocking(move || core.execute(command))
        .await
        .map_err(|_| CommandError::new(CommandErrorKind::Internal, "command task failed"))?
}

/// Attaches the UI's binary frame channel. The previous channel, if any, is
/// dropped and every stream owes the new consumer a full snapshot.
#[tauri::command]
pub fn open_market_stream(on_frame: Channel<InvokeResponseBody>, state: State<'_, MarketBridge>) {
    *state.ui.lock().unwrap_or_else(PoisonError::into_inner) = Some(on_frame);
    state.core.resync();
}

/// Hands each binary frame to the attached channel as raw bytes. Frames
/// produced while no channel is attached are dropped; `resync` on attach
/// guarantees the consumer starts from full snapshots.
fn forward_frames(frames: Receiver<Vec<u8>>, ui: &Mutex<Option<Channel<InvokeResponseBody>>>) {
    loop {
        let bytes = match frames.recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => bytes,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let channel = ui
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let Some(channel) = channel else {
            continue;
        };
        if channel.send(InvokeResponseBody::Raw(bytes)).is_err() {
            // The webview is gone; detach until a new one attaches.
            *ui.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
    }
}