mod core;

use crate::core::{
    ArchivePreview, ConfigFile, ConfigResolution, ExportSelection, ImportResolution, ImportSummary,
    ProjectInfo,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    message: String,
    current: u64,
    total: u64,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "no se encuentra el directorio home".to_string())
}

fn claude_dir() -> Result<PathBuf, String> {
    Ok(home()?.join(".claude"))
}

fn emit_progress(app: &AppHandle, message: &str, current: u64, total: u64) {
    let _ = app.emit(
        "transfer-progress",
        Progress { message: message.to_string(), current, total },
    );
}

/// Limita las emisiones a ~1 cada 80 ms (miles de archivos pequeños saturarían
/// el IPC); la emisión final (current == total) pasa siempre.
fn throttled_progress(app: AppHandle) -> impl FnMut(&str, u64, u64) {
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    move |msg: &str, cur: u64, tot: u64| {
        if cur == tot || last.elapsed().as_millis() >= 80 {
            last = std::time::Instant::now();
            emit_progress(&app, msg, cur, tot);
        }
    }
}

#[tauri::command]
async fn list_projects(exclusions: Vec<String>) -> Result<Vec<ProjectInfo>, String> {
    core::list_projects(&claude_dir()?, &exclusions).map_err(|e| e.to_string())
}

#[tauri::command]
async fn folder_sizes(paths: Vec<String>, exclusions: Vec<String>) -> Result<Vec<u64>, String> {
    Ok(core::folder_sizes(&paths, &exclusions))
}

#[tauri::command]
async fn list_config() -> Result<Vec<ConfigFile>, String> {
    Ok(core::list_config(&claude_dir()?))
}

#[tauri::command]
async fn export_projects(
    app: AppHandle,
    selections: Vec<ExportSelection>,
    exclusions: Vec<String>,
    config_files: Vec<String>,
    dest_path: String,
) -> Result<(), String> {
    let claude = claude_dir()?;
    let home = home()?.to_string_lossy().to_string();
    core::export_projects(
        &claude,
        &home,
        &selections,
        &exclusions,
        &config_files,
        PathBuf::from(dest_path).as_path(),
        throttled_progress(app),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn inspect_archive(zip_path: String) -> Result<ArchivePreview, String> {
    let claude = claude_dir()?;
    let home = home()?.to_string_lossy().to_string();
    core::inspect_archive(PathBuf::from(zip_path).as_path(), &claude, &home)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_projects(
    app: AppHandle,
    zip_path: String,
    resolutions: Vec<ImportResolution>,
    config: ConfigResolution,
) -> Result<ImportSummary, String> {
    let claude = claude_dir()?;
    let home = home()?.to_string_lossy().to_string();
    core::import_projects(
        PathBuf::from(zip_path).as_path(),
        &claude,
        &home,
        &resolutions,
        &config,
        throttled_progress(app),
    )
    .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_projects,
            list_config,
            folder_sizes,
            export_projects,
            inspect_archive,
            import_projects
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
