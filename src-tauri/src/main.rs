#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
compile_error!(
    "kill-port 只支持 Windows 与 macOS：请为本平台补充 port_scanner / process_killer 实现"
);

mod common;

#[cfg(target_os = "windows")]
mod app_status;
#[cfg(target_os = "windows")]
mod port_scanner;
#[cfg(target_os = "windows")]
mod process_killer;

#[cfg(target_os = "macos")]
mod macos;

use common::{AppStatus, KillResult, KillTarget, PortInfo};

#[tauri::command]
fn scan_ports() -> Result<Vec<PortInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        port_scanner::scan_ports().map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        macos::port_scanner::scan_ports().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn kill_processes(targets: Vec<KillTarget>, force: bool) -> Result<Vec<KillResult>, String> {
    #[cfg(target_os = "windows")]
    {
        let current_ports = port_scanner::scan_ports().map_err(|e| e.to_string())?;
        Ok(process_killer::kill_processes(
            &targets,
            &current_ports,
            force,
        ))
    }
    #[cfg(target_os = "macos")]
    {
        let current_ports = macos::port_scanner::scan_ports().map_err(|e| e.to_string())?;
        Ok(macos::process_killer::kill_processes(
            &targets,
            &current_ports,
            force,
        ))
    }
}

#[tauri::command]
fn get_app_status() -> AppStatus {
    #[cfg(target_os = "windows")]
    {
        app_status::current_status()
    }
    #[cfg(target_os = "macos")]
    {
        macos::app_status::current_status()
    }
}

fn main() {
    tauri::Builder::default()
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
        .invoke_handler(tauri::generate_handler![
            scan_ports,
            kill_processes,
            get_app_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
