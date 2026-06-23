#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod port_scanner;
mod process_killer;

use port_scanner::PortInfo;
use process_killer::{KillResult, KillTarget};

#[tauri::command]
fn scan_ports() -> Result<Vec<PortInfo>, String> {
    port_scanner::scan_ports().map_err(|e| e.to_string())
}

#[tauri::command]
fn kill_processes(targets: Vec<KillTarget>, force: bool) -> Result<Vec<KillResult>, String> {
    let current_ports = port_scanner::scan_ports().map_err(|e| e.to_string())?;
    Ok(process_killer::kill_processes(
        &targets,
        &current_ports,
        force,
    ))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![scan_ports, kill_processes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
