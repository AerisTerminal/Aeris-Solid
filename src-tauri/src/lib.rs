//! Desktop shell: Tauri commands and channels over `app_core`. No logic.

mod market_bridge;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(market_bridge::MarketBridge::start());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            market_bridge::market_command,
            market_bridge::open_market_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}