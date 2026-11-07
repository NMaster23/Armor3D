use std::{fs::File, io::{BufWriter, Write}, os::windows::ffi::OsStrExt, path::PathBuf};
use app_dirs2::*;
use curl::easy::{Easy, List};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use window_vibrancy::apply_acrylic;

const APP_INFO: AppInfo = AppInfo{name: "Armor3D", author: "NMCreator"};

const DOWNLOAD_URL: &str = "https://release-assets.githubusercontent.com/github-production-release-asset/1373728226/56d8ed43-5768-400b-b1de-ba7fc1254b94?sp=r&sv=2018-11-09&sr=b&spr=https&se=2026-11-07T20%3A08%3A34Z&rscd=attachment%3B+filename%3DArmor3D.exe&rsct=application%2Foctet-stream&skoid=96c2d410-5711-43a1-aedd-ab1947aa7ab0&sktid=398a6654-997b-47e9-b12b-9515b896b4de&skt=2026-11-07T19%3A07%3A43Z&ske=2026-11-07T20%3A08%3A34Z&sks=b&skv=2018-11-09&sig=MriPuXBPjgfMQbGNevK%2B6Gy%2FVtpO70Az2Yu4J60vEUo%3D&jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmVsZWFzZS1hc3NldHMuZ2l0aHVidXNlcmNvbnRlbnQuY29tIiwia2V5Ijoia2V5MSIsImV4cCI6MTc5NDA4MjM5MywibmJmIjoxNzk0MDc4NzkzLCJwYXRoIjoicmVsZWFzZWFzc2V0cHJvZHVjdGlvbi5ibG9iLmNvcmUud2luZG93cy5uZXQifQ.PKkCps786KeB_Atq9Lg4_mECV3Dj3yV-a0OnnXB82uk&response-content-disposition=attachment%3B%20filename%3DArmor3D.exe&response-content-type=application%2Foctet-stream";

#[derive(Serialize, Clone)]
pub struct Payload {
    pub progress: f32,
    pub msg: String,
}

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

#[derive(Serialize, Deserialize, Clone)]
pub struct InstallPaths {
    pub default_dir: String,
    pub desktop_dir: String,
    pub start_menu_dir: String,
}

unsafe extern "system" {
    fn GetDiskFreeSpaceExw(
        dir: *const u16,
        free_avalable: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> i32;
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
        .invoke_handler(tauri::generate_handler![greet, save_config, get_default_paths, check_disk_space, create_desktop_shortcut, launch_installed_app, install_app])
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

#[tauri::command]
fn get_default_paths() -> Result<InstallPaths, String> {
    let local = dirs::data_local_dir().ok_or("Could not find LocalAppData")?;
    let default_dir = local.join("Programs").join("Armor3D");
    let desktop_dir  = dirs::desktop_dir().ok_or("Could not find desktop")?;
    let start_menu_dir = local.join("Microsoft").join("Windows").join("Start Menu").join("Programs");
    Ok(InstallPaths {
        default_dir: default_dir.to_string_lossy().to_string(),
        desktop_dir: desktop_dir.to_string_lossy().to_string(),
        start_menu_dir: start_menu_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn check_disk_space(target_path: String) -> Result<u64, String> {
    let wide: Vec<u16> = std::ffi::OsStr::new(&target_path).encode_wide().chain(std::iter::once(0)).collect();
    let mut free_bytes: u64 = 0;
    let ret = unsafe {
        GetDiskFreeSpaceExw(wide.as_ptr(), &mut free_bytes, std::ptr::null_mut(), std::ptr::null_mut())
    };
    if ret == 0 {
        return Ok(10 * 1024 * 1024 * 1024);
    };
    if free_bytes < 500 * 1024 * 1024 {
        return Err("Not enough disk space. Please free space and then try again.".to_string())
    }
    Ok(free_bytes)
}

#[tauri::command]
fn create_desktop_shortcut(exe_path: String) -> Result<(), String> {
    let desktop = dirs::desktop_dir().ok_or("Could not find the desktop directory")?;
    let shortcut_path = desktop.join("Armor3D.lnk");
    let sl = mslnk::ShellLink::new(&exe_path).map_err(|e| format!("Error creating shell link: {}", e))?;
    sl.create_lnk(&shortcut_path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn launch_installed_app(exe_path: String) -> Result<(), String> {
    std::process::Command::new(exe_path)
        .spawn()
        .map_err(|e| format!("Error starting application: {}", e))?;
    Ok(())
}

#[tauri::command]
fn install_app(window: tauri::Window, target_dir: String) -> Result<(), String> {
    let _ = window.emit("install-progress", Payload {
        progress: 10.0,
        msg: "Preparing Installation".to_string(),
    });
    let dest = PathBuf::from(&target_dir);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let file = File::create(&dest.join("Armor3D.exe")).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(file);
    let mut easy = Easy::new();
    easy.url(DOWNLOAD_URL).map_err(|e| e.to_string())?;
    easy.follow_location(true).map_err(|e| e.to_string())?;
    let mut headers = List::new();
    let _ = headers.append("Armor3D Installer - Downloading EXE");
    easy.http_headers(headers).map_err(|e| e.to_string())?;
    let mut transfer = easy.transfer();
    transfer.write_function(|d| {
        writer.write_all(d).map_err(|_| curl::easy::WriteError::Pause)?;
        Ok(d.len())
    }).map_err(|e| e.to_string())?;
    transfer.perform().map_err(|e| e.to_string())?;
    drop(transfer);
    let response_code = easy.response_code().map_err(|e| e.to_string())?;
    if response_code != 200 {
        return Err(format!("Error downloading EXE with error code: {}", response_code));
    }
    let _ = window.emit("install-progress", Payload {
        progress: 50.0,
        msg: "Finished EXE download".to_string(),
    });
    save_config()?;
    let _ = window.emit("install-progress", Payload {
        progress: 95.0,
        msg: "Creating desktop shortcut".to_string(),
    });
    create_desktop_shortcut(dest.join("Armor3D.exe").to_string_lossy().to_string())?;
    let _ = window.emit("install-progress", Payload {
        progress: 100.0,
        msg: "Installation complete".to_string(),
    });
    Ok(())
}