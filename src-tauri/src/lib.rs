// The backend modules are pure Rust and do not depend on Tauri. See backend/mod.rs.
pub mod backend;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use std::path::PathBuf;

use backend::registry::{Registry, Row};
use backend::scanner::{self, Project};
use backend::logs::LogLine;
use backend::summary::Summary;
use backend::widget;
use backend::widget_refresh::{Throttle, DEFAULT_MIN_INTERVAL_SECS};
use tauri::Manager;
use tauri_plugin_widgets::{models::WidgetConfig, WidgetExt};

const APP_GROUP: &str = "group.app.demerzel.orchestrator";

/// Реестр запущенных процессов.
pub struct Processes(pub Mutex<Registry>);

/// Состояние троттлинга виджета, общее для всего приложения.
pub struct WidgetState(pub Mutex<Throttle>);

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Обновляет виджет по сводке, если содержимое изменилось и прошёл интервал.
/// Возвращает `true`, если обновление выполнено.
pub fn refresh_widget(app: &tauri::AppHandle, summary: &Summary) -> Result<bool, String> {
    let json = widget::small(summary);
    let content = json.to_string();
    let state = app.state::<WidgetState>();
    let mut throttle = state.0.lock().map_err(|e| e.to_string())?;
    let now = now_secs();
    if !throttle.should_refresh(now, &content) {
        return Ok(false);
    }
    let cfg: WidgetConfig = serde_json::from_value(json).map_err(|e| e.to_string())?;
    let handle = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let w = handle.widget();
        let res = w
            .set_widget_config(&cfg, APP_GROUP, widget::WIDGET_ID, true)
            .map_err(|e| e.to_string())
            .and_then(|_| w.reload_all_timelines().map_err(|e| e.to_string()));
        let _ = tx.send(res);
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())??;
    throttle.mark(now, &content);
    Ok(true)
}

#[tauri::command]
fn refresh_widget_from_summary(app: tauri::AppHandle, summary: Summary) -> Result<bool, String> {
    refresh_widget(&app, &summary)
}

#[tauri::command]
fn scan_projects(roots: Vec<String>) -> Result<Vec<Project>, String> {
    let mut all = Vec::new();
    for root in roots {
        all.extend(scanner::scan(&PathBuf::from(root)).map_err(|e| e.to_string())?);
    }
    Ok(all)
}

#[tauri::command]
fn start_process(
    processes: tauri::State<'_, Processes>,
    project: String,
    path: String,
    command: String,
) -> Result<u64, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.start(&project, &command, &PathBuf::from(path), &Default::default(), None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_process(processes: tauri::State<'_, Processes>, id: u64) -> Result<Option<backend::process::Status>, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.stop(id, std::time::Duration::from_secs(5)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_processes(processes: tauri::State<'_, Processes>) -> Result<Vec<Row>, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.list().map_err(|e| e.to_string())
}

#[tauri::command]
fn process_logs(processes: tauri::State<'_, Processes>, id: u64) -> Result<Option<Vec<LogLine>>, String> {
    let reg = processes.0.lock().map_err(|e| e.to_string())?;
    Ok(reg.logs(id))
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_widgets::init())
        .manage(WidgetState(Mutex::new(Throttle::new(DEFAULT_MIN_INTERVAL_SECS))))
        .manage(Processes(Mutex::new(Registry::default())))
        .invoke_handler(tauri::generate_handler![
            greet,
            refresh_widget_from_summary,
            scan_projects,
            start_process,
            stop_process,
            list_processes,
            process_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
