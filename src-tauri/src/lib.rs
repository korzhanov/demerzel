// The backend modules are pure Rust and do not depend on Tauri. See backend/mod.rs.
pub mod backend;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use std::path::{Path, PathBuf};

use backend::alerts::{AlertEvent, DeliveryDecision, DeliveryGate};
use backend::notify::{self, NotifyPayload, OpenProjectPayload, EXTRA_PROJECT, OPEN_PROJECT_EVENT};
use backend::speak::{self, SaySpeaker};
use backend::registry::{Registry, Row};
use backend::scanner::{self, Project};
use backend::logs::LogLine;
use backend::icloud_sync::{self, SyncResult as ICloudSyncResult};
use backend::summary::Summary;
use backend::widget;
use backend::widget_refresh::{Throttle, DEFAULT_MIN_INTERVAL_SECS};
use backend::widgetkit_fallback::{self, SyncResult as WidgetKitSyncResult};
use chrono::Utc;
use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_widgets::{models::WidgetConfig, WidgetExt};

const APP_GROUP: &str = "group.app.demerzel.orchestrator";

/// Реестр запущенных процессов.
pub struct Processes(pub Mutex<Registry>);

/// Состояние троттлинга виджета, общее для всего приложения.
pub struct WidgetState(pub Mutex<Throttle>);

/// Stateful `DeliveryGate` для алертов (#21–#23).
pub struct AlertGate(pub Mutex<DeliveryGate>);

/// In-memory реестр agent runs для UI ревью (#32).
pub struct AgentRuns(pub Mutex<backend::agents::AgentRunRegistry>);

/// Лимитер параллельных агентов (#29 / #33).
pub struct AgentLimit(pub Mutex<backend::agents::AgentLimiter>);

/// Результат приёма события алерта (для UI / тестов).
#[derive(Debug, Clone, Serialize)]
pub struct IngestAlertResult {
    pub matched: bool,
    pub decision: Option<DeliveryDecisionDto>,
    pub notified: bool,
    pub spoken: bool,
    pub project: Option<String>,
    /// Предложение автофикса в режиме suggest (#96).
    pub autofix_offer: Option<backend::autofix::AutofixOffer>,
    /// Автозапуск в режиме auto (#96); run уходит в ревью, не в main.
    pub autofix_started: Option<backend::autofix::StartAgentFromAlertResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryDecisionDto {
    pub emit: bool,
    pub speak: bool,
    pub notify: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationTestResult {
    pub notified: bool,
    pub spoken: bool,
    pub speak_error: Option<String>,
}

impl From<DeliveryDecision> for DeliveryDecisionDto {
    fn from(d: DeliveryDecision) -> Self {
        Self {
            emit: d.emit,
            speak: d.speak,
            notify: d.notify,
        }
    }
}

fn show_os_notification(app: &tauri::AppHandle, payload: &NotifyPayload) -> Result<(), String> {
    let mut builder = app
        .notification()
        .builder()
        .title(payload.title.clone())
        .body(payload.body.clone());
    if let Some(project) = &payload.project {
        builder = builder.extra(EXTRA_PROJECT, project);
    }
    builder.show().map_err(|e| e.to_string())
}

fn focus_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn handle_notification_action(app: &tauri::AppHandle, action_id: &str, project: Option<String>) {
    if action_id != "tap" {
        return;
    }
    focus_main_window(app);
    if let Some(project) = project.filter(|p| !p.is_empty()) {
        let _ = app.emit(OPEN_PROJECT_EVENT, OpenProjectPayload { project });
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Обновляет виджет по сводке, если содержимое изменилось и прошёл интервал.
/// Возвращает `true`, если обновление выполнено.
pub fn refresh_widget(app: &tauri::AppHandle, summary: &Summary) -> Result<bool, String> {
    // small+medium IR (#98); throttle по тексту — без reload при неизменном пульсе/статусе.
    let json = widget::config(summary);
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
    // iCloud sync best-effort: ошибка не блокирует macOS-виджет (#37).
    let _ = sync_summary_to_icloud(app.clone(), summary.clone());
    match refresh_widget(&app, &summary) {
        Ok(updated) => {
            // Опциональная запись в App Group для native WidgetKit (#38).
            let _ = sync_summary_to_widgetkit(app.clone(), summary.clone());
            Ok(updated)
        }
        Err(plugin_err) => {
            // Плагин недоступен → forced write summary.json в App Group (fallback).
            match sync_summary_to_widgetkit_forced(app, summary) {
                Ok(r)
                    if matches!(
                        r.outcome,
                        widgetkit_fallback::SyncOutcome::Written
                            | widgetkit_fallback::SyncOutcome::Unchanged
                    ) =>
                {
                    Ok(matches!(r.outcome, widgetkit_fallback::SyncOutcome::Written))
                }
                Ok(_) => Err(format!(
                    "tauri-plugin-widgets unavailable ({plugin_err}); widgetkit fallback disabled"
                )),
                Err(e) => Err(format!(
                    "tauri-plugin-widgets unavailable ({plugin_err}); widgetkit fallback failed: {e}"
                )),
            }
        }
    }
}

/// Пишет `summary.json` в iCloud Drive, если `settings.icloud.enabled`.
#[tauri::command]
fn sync_summary_to_icloud(
    app: tauri::AppHandle,
    summary: Summary,
) -> Result<ICloudSyncResult, String> {
    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let home = icloud_sync::user_home().ok_or_else(|| "HOME is not set".to_string())?;
    icloud_sync::sync_summary(&home, &settings.icloud, &summary).map_err(|e| e.to_string())
}

/// Пишет `summary.json` в App Group, если `settings.widgetkit.enabled` (#38).
#[tauri::command]
fn sync_summary_to_widgetkit(
    app: tauri::AppHandle,
    summary: Summary,
) -> Result<WidgetKitSyncResult, String> {
    sync_widgetkit_inner(&app, &summary, false)
}

fn sync_summary_to_widgetkit_forced(
    app: tauri::AppHandle,
    summary: Summary,
) -> Result<WidgetKitSyncResult, String> {
    sync_widgetkit_inner(&app, &summary, true)
}

fn sync_widgetkit_inner(
    app: &tauri::AppHandle,
    summary: &Summary,
    force: bool,
) -> Result<WidgetKitSyncResult, String> {
    let settings = backend::settings::load(&settings_path(app)?).map_err(|e| e.to_string())?;
    let home = widgetkit_fallback::user_home().ok_or_else(|| "HOME is not set".to_string())?;
    widgetkit_fallback::sync_summary(&home, &settings.widgetkit, summary, force)
        .map_err(|e| e.to_string())
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join(backend::settings::SETTINGS_FILE))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<backend::settings::Settings, String> {
    backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    gate: tauri::State<'_, AlertGate>,
    settings: backend::settings::Settings,
) -> Result<(), String> {
    backend::settings::save(&settings_path(&app)?, &settings).map_err(|e| e.to_string())?;
    // Новая политика доставки; история дедупа сбрасывается вместе с конфигом.
    let mut g = gate.0.lock().map_err(|e| e.to_string())?;
    *g = DeliveryGate::from_config(&settings.alerts);
    // App Intents (#101): первый scan_root → App Group siri-config.json.
    if let Some(home) = backend::widgetkit_fallback::user_home() {
        let root = backend::siri_config::root_from_scan_roots(&settings.scan_roots);
        if let Err(e) = backend::siri_config::sync_ecosystem_root(&home, root) {
            eprintln!("[demerzel] siri-config sync failed: {e}");
        }
    }
    Ok(())
}

#[tauri::command]
fn scan_projects(app: tauri::AppHandle, roots: Vec<String>) -> Result<Vec<Project>, String> {
    let settings = backend::settings::load(&settings_path(&app)?)
        .unwrap_or_default();
    let roots = if roots.is_empty() {
        settings.scan_roots.clone()
    } else {
        backend::settings::normalize_roots(&roots)
    };
    let mut all = Vec::new();
    for root in roots {
        let path = PathBuf::from(&root);
        let rules =
            backend::settings::resolve_rules_for_path(&root, &settings.scan_entries);
        let opts = scanner::ScanOpts::from_rules(rules);
        let found = scanner::scan_with_opts(&path, &opts).map_err(|e| {
            scanner::access_hint(&e, &path).unwrap_or_else(|| format!("{}: {e}", path.display()))
        })?;
        all.extend(found);
    }
    Ok(all)
}

#[derive(serde::Serialize)]
struct DirChildDto {
    name: String,
    path: String,
    is_dir: bool,
    orchestration: String,
}

fn orchestration_status(dir: &Path) -> &'static str {
    if dir.join("orchestrator.json").is_file() || dir.join("demerzel.yaml").is_file() {
        "configured"
    } else {
        "missing"
    }
}

/// Дети каталога для дерева в UI настроек скана.
#[tauri::command]
fn list_dir_children(path: String) -> Result<Vec<DirChildDto>, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if scanner::is_ignored(&name) {
            continue;
        }
        let child = entry.path();
        out.push(DirChildDto {
            name,
            path: child.to_string_lossy().into_owned(),
            is_dir: true,
            orchestration: orchestration_status(&child).into(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// `.app` с `TauriWidgetExtension.appex` — для копирования в `/Applications`.
#[cfg(target_os = "macos")]
fn find_widget_source_app() -> Result<std::path::PathBuf, String> {
    use std::path::{Path, PathBuf};

    fn has_widget_appex(app: &Path) -> bool {
        app.join("Contents/PlugIns/TauriWidgetExtension.appex").is_dir()
    }

    /// `…/Demerzel.app/Contents/MacOS/demerzel` → `…/Demerzel.app`
    fn bundle_from_exe(exe: &Path) -> Option<PathBuf> {
        let macos = exe.parent()?;
        if macos.file_name()?.to_str()? != "MacOS" {
            return None;
        }
        let contents = macos.parent()?;
        if contents.file_name()?.to_str()? != "Contents" {
            return None;
        }
        let app = contents.parent()?;
        if app.extension()?.to_str()? != "app" {
            return None;
        }
        Some(app.to_path_buf())
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    // Предпочитаем свежие результаты сборки: при запуске из /Applications
    // текущий бандл сам указывает туда и иначе заглушал бы обновление виджета.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest.join("target/debug/bundle/macos/Demerzel.app"));
    candidates.push(manifest.join("target/release/bundle/macos/Demerzel.app"));

    // cwd: корень репо или src-tauri.
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri/target/debug/bundle/macos/Demerzel.app"));
        candidates.push(cwd.join("src-tauri/target/release/bundle/macos/Demerzel.app"));
        candidates.push(cwd.join("target/debug/bundle/macos/Demerzel.app"));
        candidates.push(cwd.join("target/release/bundle/macos/Demerzel.app"));
    }

    // Только fallback: текущий бандл, в том числе установленный в /Applications.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(app) = bundle_from_exe(&exe) {
            candidates.push(app);
        }
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("bundle/macos/Demerzel.app"));
        }
    }

    for c in &candidates {
        if c.is_dir() && has_widget_appex(c) {
            return Ok(c.clone());
        }
    }

    Err(format!(
        "Не найден Demerzel.app с TauriWidgetExtension.appex.\n\
Соберите: bun run tauri build --debug\n\
Искали, например: {}",
        candidates
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(пусто)".into())
    ))
}

#[cfg(target_os = "macos")]
fn find_sign_macos_script() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut paths = Vec::new();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    paths.push(manifest.join("../scripts/sign-macos.sh"));
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("scripts/sign-macos.sh"));
        paths.push(cwd.join("../scripts/sign-macos.sh"));
    }
    paths.into_iter().find(|p| p.is_file())
}

/// Установка сборки в `/Applications` + ad-hoc sign, чтобы виджеты появились в Gallery.
#[tauri::command]
fn install_widgets() -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("Установка виджетов доступна только на macOS".into());
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let src = find_widget_source_app()?;
        let dest = std::path::PathBuf::from("/Applications/Demerzel.app");

        // Уже запущено из /Applications с виджетом — только подсказка.
        if src == dest {
            let _ = Command::new("open").arg("/Applications/Demerzel.app").status();
            return Ok(
                "Виджеты уже в /Applications/Demerzel.app. Notification Center → Edit Widgets → Demerzel."
                    .into(),
            );
        }

        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|e| format!("rm /Applications: {e}"))?;
        }
        let status = Command::new("cp")
            .args(["-R", &src.to_string_lossy(), "/Applications/Demerzel.app"])
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("cp в /Applications не удался".into());
        }

        if let Some(sign) = find_sign_macos_script() {
            let sign_status = Command::new("bash")
                .arg(&sign)
                .arg("/Applications/Demerzel.app")
                .status()
                .map_err(|e| format!("sign-macos: {e}"))?;
            if !sign_status.success() {
                return Err("Ad-hoc подпись не удалась (scripts/sign-macos.sh)".into());
            }
        } else {
            // Fallback: codesign без скрипта (dev / установленный бандл без репо)
            let _ = Command::new("codesign")
                .args([
                    "--force",
                    "--deep",
                    "--sign",
                    "-",
                    "/Applications/Demerzel.app",
                ])
                .status();
        }

        let _ = Command::new("open")
            .arg("/Applications/Demerzel.app")
            .status();
        Ok(format!(
            "Скопировано из {} → /Applications. Edit Widgets → Demerzel.",
            src.display()
        ))
    }
}

/// Статус оркестрации для одной папки (`configured` | `missing`).
#[tauri::command]
fn get_orchestration(path: String) -> Result<String, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    Ok(orchestration_status(&dir).into())
}

/// Создаёт минимальный `orchestrator.json`, если его ещё нет.
#[tauri::command]
fn init_orchestrator(path: String) -> Result<String, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let file = dir.join("orchestrator.json");
    if file.is_file() {
        return Ok(file.to_string_lossy().into_owned());
    }
    let name = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let stub = serde_json::json!({
        "name": name,
        "commands": { "dev": "bun run dev" }
    });
    std::fs::write(
        &file,
        serde_json::to_string_pretty(&stub).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(file.to_string_lossy().into_owned())
}

#[tauri::command]
fn resolve_dev_command(path: String) -> Option<String> {
    backend::config::resolve_dev_command(&PathBuf::from(path))
}

#[tauri::command]
fn start_process(
    processes: tauri::State<'_, Processes>,
    project: String,
    path: String,
    command: String,
) -> Result<u64, String> {
    let cwd = PathBuf::from(&path);
    // Пустое поле или устаревший дефолт UI «bun run dev» → commands.dev с диска.
    let cmd = {
        let trimmed = command.trim();
        let use_disk = trimmed.is_empty() || trimmed == "bun run dev";
        if use_disk {
            if let Some(resolved) = backend::config::resolve_dev_command(&cwd) {
                resolved
            } else if trimmed.is_empty() {
                return Err(format!(
                    "нет commands.dev в {} и нет scripts.dev в package.json — укажите команду в поле «команда»",
                    backend::config::CONFIG_FILE
                ));
            } else {
                trimmed.to_string()
            }
        } else {
            trimmed.to_string()
        }
    };
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    // Диапазон из orchestrator.json → аренда → env PORT (#13).
    reg.start_configured(&project, &cmd, &cwd)
}

#[tauri::command]
fn stop_process(processes: tauri::State<'_, Processes>, id: u64) -> Result<Option<backend::process::Status>, String> {
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.stop(id, std::time::Duration::from_secs(5)).map_err(|e| e.to_string())
}

/// Статусы выбранных автозадач проекта по реестру процессов (#153, этап B).
#[tauri::command]
fn automation_statuses(
    processes: tauri::State<'_, Processes>,
    path: String,
) -> Result<Vec<backend::automation::AutomationStatus>, String> {
    let dir = std::path::PathBuf::from(&path);
    let discovered = backend::automation::discover(&dir);
    let selected = backend::config::load_checked(&dir)?
        .and_then(|c| c.automation)
        .map(|a| a.selected)
        .unwrap_or_default();
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.list().map_err(|e| e.to_string())?;
    let runs = reg.runs();
    Ok(backend::automation::match_statuses(&dir, &selected, &discovered, &runs))
}

/// Запуск выбранной автозадачи с лимитом на проект (#153, этап C).
#[tauri::command]
fn start_automation(
    processes: tauri::State<'_, Processes>,
    project: String,
    path: String,
    source: String,
    name: String,
) -> Result<u64, String> {
    let dir = std::path::PathBuf::from(&path);
    let discovered = backend::automation::discover(&dir);
    let command = discovered
        .iter()
        .find(|d| d.source == source && d.name == name)
        .and_then(|d| d.command.clone())
        .ok_or_else(|| format!("автозадача {name} не найдена в {path}"))?;
    let selected = backend::config::load_checked(&dir)?
        .and_then(|c| c.automation)
        .map(|a| a.selected)
        .unwrap_or_default();
    let mut reg = processes.0.lock().map_err(|e| e.to_string())?;
    reg.list().map_err(|e| e.to_string())?;
    let runs = reg.runs();
    let statuses = backend::automation::match_statuses(&dir, &selected, &discovered, &runs);
    backend::automation::check_start_allowed(&dir, &statuses, backend::automation::MAX_RUNNING_PER_PROJECT)?;
    reg.start_configured(&project, &command, &dir)
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
fn list_ports(
    processes: tauri::State<'_, Processes>,
    paths: Option<Vec<String>>,
) -> Result<Vec<backend::ports::PortRow>, String> {
    let known: Vec<PathBuf> = paths
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    processes
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .port_inventory(&known)
}

#[tauri::command]
fn project_tasks(path: String) -> Result<Vec<backend::tasks::Task>, String> {
    use backend::tasks::TaskSource;
    let root = PathBuf::from(&path);
    let cfg = backend::config::load_checked(&root)?;
    let task_config = cfg.and_then(|c| c.tasks);
    let source = task_config.as_ref().map(|t| t.source.as_str()).unwrap_or("todo");
    if source == "github" {
        let tasks = task_config.as_ref().ok_or("missing tasks configuration")?;
        return backend::github_tasks::GithubSource {
            repository: tasks.repository.clone().ok_or("tasks.repository must be owner/name")?,
            label: tasks.label.clone(),
        }.fetch(&path);
    }
    if source == "notion" {
        let tasks = task_config.as_ref().ok_or("missing tasks configuration")?;
        return backend::notion_tasks::NotionSource {
            database_id: tasks
                .database_id
                .clone()
                .ok_or("tasks.database_id is required for source=notion")?,
            title_property: tasks.title_property.clone(),
            status_property: tasks.status_property.clone(),
        }
        .fetch(&path);
    }
    backend::tasks::FileSource { root, json: source == "tasks-json" }.fetch(&path)
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn list_agent_runs(
    runs: tauri::State<'_, AgentRuns>,
) -> Result<Vec<backend::agents::AgentRunRecord>, String> {
    let reg = runs.0.lock().map_err(|e| e.to_string())?;
    Ok(reg.list())
}

#[tauri::command]
fn get_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AgentRunRecord, String> {
    let reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.get(&id)
        .cloned()
        .ok_or_else(|| format!("agent run not found: {id}"))
}

/// Запись / обновление run (сиды для UI и будущий запуск из задачи #33).
#[tauri::command]
fn upsert_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    record: backend::agents::AgentRunRecord,
) -> Result<(), String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.upsert(record)
}

#[tauri::command]
fn accept_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AcceptAgentRunResult, String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.accept(&id)
}

#[tauri::command]
fn discard_agent_run(
    runs: tauri::State<'_, AgentRuns>,
    id: String,
) -> Result<backend::agents::AgentRunRecord, String> {
    let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
    reg.discard(&id)
}

/// Обнаруженные автозадачи и уже выбранные пользователем (#153, этап B).
#[derive(serde::Serialize)]
struct AutomationsView {
    discovered: Vec<backend::automation::DiscoveredAutomation>,
    selected: Vec<backend::config::AutomationSelection>,
    schedules: Vec<backend::config::AutomationSchedule>,
    restart: backend::config::RestartRule,
}

#[tauri::command]
fn load_automations(path: String) -> Result<AutomationsView, String> {
    let dir = std::path::Path::new(&path);
    let discovered = backend::automation::discover(dir);
    let automation = backend::config::load_checked(dir)?.and_then(|c| c.automation);
    let selected = automation
        .as_ref()
        .map(|a| a.selected.clone())
        .unwrap_or_default();
    let schedules = automation
        .as_ref()
        .map(|a| a.schedules.clone())
        .unwrap_or_default();
    let restart = automation.map(|a| a.restart).unwrap_or_default();
    Ok(AutomationsView {
        discovered,
        selected,
        schedules,
        restart,
    })
}

/// Записывает выбор в `orchestrator.json` → `automation.selected`, остальные поля не трогает (#153).
#[tauri::command]
fn save_automation_selection(
    path: String,
    selected: Vec<backend::config::AutomationSelection>,
) -> Result<(), String> {
    backend::automation::save_selection(std::path::Path::new(&path), &selected)
}

/// Сохраняет внутреннее расписание для одной автозадачи (#153, этап E).
#[tauri::command]
fn save_automation_schedule(
    path: String,
    schedule: backend::config::AutomationSchedule,
) -> Result<(), String> {
    backend::automation::save_schedule(std::path::Path::new(&path), &schedule)
}

/// Правило перезапуска автозадач (#153, этап D): сохраняет только секцию restart.
#[tauri::command]
fn save_restart_rule(path: String, rule: backend::config::RestartRule) -> Result<(), String> {
    backend::automation::save_restart_rule(std::path::Path::new(&path), &rule)
}

/// Кандидаты автозадач проекта: только чтение, ничего не запускает (#153, этап A).
#[tauri::command]
fn discover_automations(path: String) -> Vec<backend::automation::DiscoveredAutomation> {
    backend::automation::discover(std::path::Path::new(&path))
}

/// Реестр CLI-агентов на машине: PATH + `--version`, подсказки установки (#95).
#[tauri::command]
fn list_available_agents() -> Vec<backend::agent_catalog::AgentCatalogEntry> {
    backend::agent_install::enrich_catalog(None)
}

/// Открыть Terminal.app с командой установки агента (#140). UI подтверждает до вызова.
#[tauri::command]
fn open_agent_install_terminal(runner: String) -> Result<String, String> {
    let runner = backend::agents::AgentRunner::parse(&runner)?;
    let cmd = backend::agent_install::install_command(runner).ok_or_else(|| {
        format!(
            "для '{}' нет команды установки в config/agent-install.json",
            runner.as_str()
        )
    })?;
    backend::agent_install::open_install_in_terminal(&cmd)?;
    Ok(cmd)
}

/// Первый включённый+установленный агент по `agents.catalog` (#140).
#[tauri::command]
fn resolve_preferred_agent(app: tauri::AppHandle) -> Result<String, String> {
    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let preferred = backend::agent_install::pick_preferred_runner(&settings.agents.catalog, None)
        .unwrap_or(settings.agents.runner);
    Ok(preferred.as_str().to_string())
}

/// Раннер по умолчанию для проекта: `orchestrator.json` → settings (#95).
#[tauri::command]
fn resolve_project_agent_runner(
    app: tauri::AppHandle,
    project_path: String,
) -> Result<String, String> {
    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let runner = backend::agents::resolve_runner_for_project(
        Path::new(&project_path),
        settings.agents.runner,
    );
    Ok(runner.as_str().to_string())
}

/// Запуск агента из задачи (#33 / #30 / #95): статус `in_progress`, worktree; CLI в фоне.
#[tauri::command]
fn start_agent_from_task(
    app: tauri::AppHandle,
    runs: tauri::State<'_, AgentRuns>,
    limit: tauri::State<'_, AgentLimit>,
    task: backend::tasks::Task,
    runner: Option<String>,
) -> Result<backend::agents::StartAgentRunResult, String> {
    use backend::agents::{
        complete_agent_run, prepare_agent_from_task, resolve_runner_for_project, task_prompt_text,
        AgentRunner, DEFAULT_CLAUDE_TIMEOUT, DEFAULT_OLLAMA_TIMEOUT,
    };
    use std::thread;

    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let project_path = PathBuf::from(&task.project);
    let runner = match runner.as_deref() {
        Some(raw) => AgentRunner::parse(raw)?,
        None => resolve_runner_for_project(&project_path, settings.agents.runner),
    };
    if let Some(err) = backend::agent_catalog::runner_readiness_error(runner, None) {
        return Err(err);
    }
    let ollama_model = settings.agents.ollama_model.clone();
    let timeout = match runner {
        AgentRunner::Ollama => DEFAULT_OLLAMA_TIMEOUT,
        _ => DEFAULT_CLAUDE_TIMEOUT,
    };
    let prompt = task_prompt_text(&task);
    let prepared = {
        let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
        let mut lim = limit.0.lock().map_err(|e| e.to_string())?;
        prepare_agent_from_task(
            &task,
            &project_path,
            &mut reg,
            &mut lim,
            Some("HEAD"),
            None,
            runner,
        )?
    };

    if prepared.queued {
        return Ok(prepared);
    }

    let run_id = prepared.run.id.clone();
    let app_handle = app.clone();
    thread::spawn(move || {
        let runs_state = app_handle.state::<AgentRuns>();
        let limit_state = app_handle.state::<AgentLimit>();
        let Ok(mut reg) = runs_state.0.lock() else {
            return;
        };
        let Ok(mut lim) = limit_state.0.lock() else {
            return;
        };
        if let Err(e) = complete_agent_run(
            &run_id,
            &prompt,
            &mut reg,
            &mut lim,
            timeout,
            None,
            Some(ollama_model.as_str()),
        ) {
            eprintln!("[agents] complete_agent_run failed: {e}");
        }
    });

    Ok(prepared)
}

/// Собирает контекст автофикса (#96) из процессов / agent runs.
fn gather_autofix_context(
    event: &AlertEvent,
    processes: &backend::registry::Registry,
    runs: &backend::agents::AgentRunRegistry,
) -> Option<backend::autofix::AlertAutofixContext> {
    use backend::autofix::context_from_parts;
    match event {
        AlertEvent::ProcessExited { project, .. } => {
            let (path, command, logs) = processes.autofix_context_by_project(project)?;
            context_from_parts(&path, event, Some(command), Some(&logs), None).ok()
        }
        AlertEvent::PortConflict { port } => {
            let (path, command, logs) = processes.autofix_context_by_port(*port)?;
            context_from_parts(&path, event, Some(command), Some(&logs), None).ok()
        }
        AlertEvent::TestsFailed { run_id } => {
            let run = runs.get(run_id)?;
            let path = PathBuf::from(&run.project_path);
            let logs_text = run.tests.as_ref().map(|t| t.log.clone()).filter(|s| !s.is_empty());
            let diff = run.diff.clone().filter(|s| !s.is_empty());
            let command = run.tests.as_ref().map(|t| t.command.clone()).filter(|s| !s.is_empty());
            // Лог тестов уже строка — кладём как хвост без Stream.
            let mut ctx = context_from_parts(&path, event, command, None, diff).ok()?;
            if let Some(log) = logs_text {
                let lines: Vec<_> = log
                    .lines()
                    .rev()
                    .take(backend::autofix::DEFAULT_LOG_TAIL_LINES)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .map(|text| backend::logs::LogLine {
                        stream: backend::logs::Stream::Stdout,
                        text: text.to_string(),
                    })
                    .collect();
                ctx.log_tail = Some(backend::autofix::format_log_tail(
                    &lines,
                    backend::autofix::DEFAULT_LOG_TAIL_LINES,
                ))
                .filter(|s| !s.is_empty());
            }
            Some(ctx)
        }
        AlertEvent::AgentFailed { .. } => None,
    }
}

/// Оценивает событие → gate → OS notify (#22) и `say` (#23); автофикс (#96).
#[tauri::command]
fn ingest_alert_event(
    app: tauri::AppHandle,
    gate: tauri::State<'_, AlertGate>,
    processes: tauri::State<'_, Processes>,
    runs: tauri::State<'_, AgentRuns>,
    limit: tauri::State<'_, AgentLimit>,
    event: AlertEvent,
) -> Result<IngestAlertResult, String> {
    use backend::agents::{AutofixMode, DEFAULT_CLAUDE_TIMEOUT, DEFAULT_OLLAMA_TIMEOUT};
    use backend::autofix::{
        build_alert_autofix_prompt, decide_autofix, prepare_agent_from_alert, resolve_autofix_mode,
        runner_for_autofix, AutofixDecision, AUTOFIX_OFFER_EVENT,
    };
    use std::thread;

    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let project = notify::project_hint(&event);
    let Some(candidate) = backend::alerts::evaluate(&settings.alerts.rules, &event) else {
        return Ok(IngestAlertResult {
            matched: false,
            decision: None,
            notified: false,
            spoken: false,
            project,
            autofix_offer: None,
            autofix_started: None,
        });
    };
    let decision = {
        let mut g = gate.0.lock().map_err(|e| e.to_string())?;
        g.decide(&candidate, Utc::now())
    };
    let notified = if let Some(payload) = notify::maybe_notify(&candidate, &decision) {
        show_os_notification(&app, &payload)?;
        true
    } else {
        false
    };
    // Ошибка `say` не валит ingest: gate уже учёл speak; логируем в stderr.
    let spoken = match speak::deliver_speak(
        &candidate,
        &decision,
        &settings.alerts.voice,
        &SaySpeaker,
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[alerts] say failed: {e}");
            false
        }
    };

    let mut autofix_offer = None;
    let mut autofix_started = None;
    if decision.emit {
        let ctx = {
            let reg = processes.0.lock().map_err(|e| e.to_string())?;
            let runs_g = runs.0.lock().map_err(|e| e.to_string())?;
            gather_autofix_context(&candidate.event, &reg, &runs_g)
        };
        let project_path = ctx
            .as_ref()
            .map(|c| PathBuf::from(&c.project_path));
        let mode = resolve_autofix_mode(project_path.as_deref(), settings.agents.autofix);
        match decide_autofix(&candidate, ctx, mode) {
            AutofixDecision::Skip => {}
            AutofixDecision::Suggest(offer) => {
                let _ = app.emit(AUTOFIX_OFFER_EVENT, &offer);
                autofix_offer = Some(offer);
            }
            AutofixDecision::AutoStart(ctx) => {
                let path = PathBuf::from(&ctx.project_path);
                let runner = runner_for_autofix(&path, settings.agents.runner);
                if let Some(err) = backend::agent_catalog::runner_readiness_error(runner, None) {
                    eprintln!("[autofix] skip auto: {err}");
                } else {
                    let prompt = build_alert_autofix_prompt(&ctx);
                    let ollama_model = settings.agents.ollama_model.clone();
                    let timeout = match runner {
                        backend::agents::AgentRunner::Ollama => DEFAULT_OLLAMA_TIMEOUT,
                        _ => DEFAULT_CLAUDE_TIMEOUT,
                    };
                    let prepared = {
                        let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
                        let mut lim = limit.0.lock().map_err(|e| e.to_string())?;
                        prepare_agent_from_alert(
                            &path,
                            &mut reg,
                            &mut lim,
                            Some("HEAD"),
                            None,
                            runner,
                            AutofixMode::Auto,
                        )?
                    };
                    if !prepared.queued {
                        let run_id = prepared.run.id.clone();
                        let app_handle = app.clone();
                        thread::spawn(move || {
                            let runs_state = app_handle.state::<AgentRuns>();
                            let limit_state = app_handle.state::<AgentLimit>();
                            let Ok(mut reg) = runs_state.0.lock() else {
                                return;
                            };
                            let Ok(mut lim) = limit_state.0.lock() else {
                                return;
                            };
                            if let Err(e) = backend::agents::complete_agent_run(
                                &run_id,
                                &prompt,
                                &mut reg,
                                &mut lim,
                                timeout,
                                None,
                                Some(ollama_model.as_str()),
                            ) {
                                eprintln!("[autofix] complete_agent_run failed: {e}");
                            }
                        });
                    }
                    autofix_started = Some(prepared);
                }
            }
        }
    }

    Ok(IngestAlertResult {
        matched: true,
        decision: Some(decision.into()),
        notified,
        spoken,
        project: notify::project_hint(&candidate.event).or(project),
        autofix_offer,
        autofix_started,
    })
}

/// Диагностическая проверка системного уведомления и macOS `say`, без gate/dedupe.
#[tauri::command]
fn test_notification(app: tauri::AppHandle) -> Result<NotificationTestResult, String> {
    let payload = NotifyPayload {
        title: "Demerzel · Проверка уведомлений".into(),
        body: "Если вы видите это сообщение, системные уведомления работают.".into(),
        project: None,
        severity: backend::alerts::Severity::Info,
        rule_id: "notification_test".into(),
    };
    show_os_notification(&app, &payload)?;

    #[cfg(target_os = "macos")]
    {
        use backend::speak::VoiceSpeaker;
        return Ok(
            match speak::SaySpeaker.speak("Milena", "Проверка голосовых уведомлений Demerzel.") {
                Ok(()) => NotificationTestResult {
                    notified: true,
                    spoken: true,
                    speak_error: None,
                },
                Err(e) => NotificationTestResult {
                    notified: true,
                    spoken: false,
                    speak_error: Some(e.to_string()),
                },
            },
        );
    }

    #[cfg(not(target_os = "macos"))]
    Ok(NotificationTestResult {
        notified: true,
        spoken: false,
        speak_error: Some("Голосовая проверка доступна только в macOS".into()),
    })
}

/// Один клик «Исправить» из предложения алерта (#96).
#[tauri::command]
fn start_agent_from_alert(
    app: tauri::AppHandle,
    runs: tauri::State<'_, AgentRuns>,
    limit: tauri::State<'_, AgentLimit>,
    project_path: String,
    prompt: String,
    runner: Option<String>,
) -> Result<backend::autofix::StartAgentFromAlertResult, String> {
    use backend::agents::{
        complete_agent_run, AgentRunner, AutofixMode, DEFAULT_CLAUDE_TIMEOUT, DEFAULT_OLLAMA_TIMEOUT,
    };
    use backend::autofix::{prepare_agent_from_alert, runner_for_autofix};
    use std::thread;

    let settings = backend::settings::load(&settings_path(&app)?).map_err(|e| e.to_string())?;
    let path = PathBuf::from(&project_path);
    let runner = match runner.as_deref() {
        Some(raw) => AgentRunner::parse(raw)?,
        None => runner_for_autofix(&path, settings.agents.runner),
    };
    if let Some(err) = backend::agent_catalog::runner_readiness_error(runner, None) {
        return Err(err);
    }
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("autofix prompt must not be empty".into());
    }
    let ollama_model = settings.agents.ollama_model.clone();
    let timeout = match runner {
        AgentRunner::Ollama => DEFAULT_OLLAMA_TIMEOUT,
        _ => DEFAULT_CLAUDE_TIMEOUT,
    };
    let prepared = {
        let mut reg = runs.0.lock().map_err(|e| e.to_string())?;
        let mut lim = limit.0.lock().map_err(|e| e.to_string())?;
        prepare_agent_from_alert(
            &path,
            &mut reg,
            &mut lim,
            Some("HEAD"),
            None,
            runner,
            AutofixMode::Suggest,
        )?
    };
    if prepared.queued {
        return Ok(prepared);
    }
    let run_id = prepared.run.id.clone();
    let app_handle = app.clone();
    thread::spawn(move || {
        let runs_state = app_handle.state::<AgentRuns>();
        let limit_state = app_handle.state::<AgentLimit>();
        let Ok(mut reg) = runs_state.0.lock() else {
            return;
        };
        let Ok(mut lim) = limit_state.0.lock() else {
            return;
        };
        if let Err(e) = complete_agent_run(
            &run_id,
            &prompt,
            &mut reg,
            &mut lim,
            timeout,
            None,
            Some(ollama_model.as_str()),
        ) {
            eprintln!("[autofix] complete_agent_run failed: {e}");
        }
    });
    Ok(prepared)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_widgets::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(WidgetState(Mutex::new(Throttle::new(DEFAULT_MIN_INTERVAL_SECS))))
        .manage(Processes(Mutex::new(Registry::default())))
        .manage(AlertGate(Mutex::new(DeliveryGate::new_default())))
        .manage(AgentRuns(Mutex::new(backend::agents::AgentRunRegistry::new())))
        .manage(AgentLimit(Mutex::new(backend::agents::AgentLimiter::from_defaults())))
        .setup(|app| {
            let handle = app.handle().clone();
            app.notification().on_action(move |performed| {
                let project = performed
                    .notification()
                    .and_then(|n| notify::project_from_extra(n.extra()));
                handle_notification_action(&handle, performed.action_id(), project);
            })?;
            // Подтянуть quiet/dedupe и agents limiter из settings при старте.
            if let Ok(path) = settings_path(app.handle())
                && let Ok(settings) = backend::settings::load(&path)
            {
                if let Ok(mut gate) = app.state::<AlertGate>().0.lock() {
                    *gate = DeliveryGate::from_config(&settings.alerts);
                }
                if let Ok(mut lim) = app.state::<AgentLimit>().0.lock() {
                    *lim = backend::agents::AgentLimiter::new(settings.agents);
                }
                // App Intents (#101): проброс ecosystem root в App Group.
                if let Some(home) = backend::widgetkit_fallback::user_home() {
                    let root = backend::siri_config::root_from_scan_roots(&settings.scan_roots);
                    let _ = backend::siri_config::sync_ecosystem_root(&home, root);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            refresh_widget_from_summary,
            sync_summary_to_icloud,
            sync_summary_to_widgetkit,
            scan_projects,
            list_dir_children,
            install_widgets,
            get_orchestration,
            init_orchestrator,
            get_settings,
            save_settings,
            resolve_dev_command,
            start_process,
            stop_process,
            list_processes,
            process_logs,
            list_ports,
            project_tasks,
            list_agent_runs,
            get_agent_run,
            upsert_agent_run,
            accept_agent_run,
            discard_agent_run,
            list_available_agents,
            discover_automations,
            open_agent_install_terminal,
            resolve_preferred_agent,
            load_automations,
            save_restart_rule,
            automation_statuses,
            start_automation,
            save_automation_selection,
            save_automation_schedule,
            resolve_project_agent_runner,
            start_agent_from_task,
            start_agent_from_alert,
            ingest_alert_event,
            test_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
