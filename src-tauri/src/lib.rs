pub mod bridge;
pub mod commands;
pub mod devices;
pub mod dsp;
pub mod engine;
pub mod params;
pub mod platform;
pub mod watcher;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            let audio = watcher::spawn(move |status| {
                let _ = handle.emit("audio-status", status);
            });
            app.manage(audio);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_audio,
            commands::select_devices,
            commands::set_params,
            commands::get_status,
            commands::get_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
