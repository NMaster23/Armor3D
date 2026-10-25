use std::{fs::File, io::BufWriter, path::PathBuf};
use app_dirs2::*;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use window_vibrancy::apply_acrylic;

const APP_INFO: AppInfo = AppInfo{name: "Armor3D", author: "NMCreator"};

#[derive(Serialize, Deserialize, Clone)]
pub struct DefaultStartup {
    pub osnap: bool,
    pub ortho: bool,
    pub grid_snap: bool,
    pub gumball: bool,
    pub default_units: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    is_dark: bool,
    pub install: PathBuf,
    startup: DefaultStartup,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, save_config])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            {
                let _ = apply_acrylic(&window, Some((18, 18, 24, 125)));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn save_config() -> Result<String, String> {
    let dir = get_app_root(AppDataType::UserConfig, &APP_INFO)
        .map_err(|e| format!("Error getting config dir: {}", e))?;
    let default_startup = DefaultStartup {
        osnap: false,
        ortho: false,
        grid_snap: false,
        gumball: false,
        default_units: "mm".to_string(),
    };
    let default = Config {
        is_dark: true,
        install: dir.clone(),
        startup: default_startup,
    };

    let file = File::create(dir.join("config.json")).map_err(|e| format!("Error creating file: {}", e))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &default)
        .map_err(|e| format!("Error writing default config to json: {}", e))?;

    Ok("Config written succesfully".to_string())
}