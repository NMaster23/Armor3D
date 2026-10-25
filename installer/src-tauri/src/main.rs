use tauri::Manager;
use window_vibrancy::{apply_acrylic, apply_vibrancy, apply_mica, apply_blur};

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#[cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    installer_lib::run()
}
