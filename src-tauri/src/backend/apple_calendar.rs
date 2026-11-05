//! Apple Calendar integration through macOS Calendar automation.

use super::calendar::ActivityRecord;

#[cfg(target_os = "macos")]
fn run_script(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .map_err(|e| format!("Не удалось запустить Apple Calendar: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
pub fn list_calendars() -> Result<Vec<String>, String> {
    let result =
        run_script("Application('Calendar').calendars().map(c => c.name()).join('\\n')")?;
    Ok(result
        .lines()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect())
}

#[cfg(not(target_os = "macos"))]
pub fn list_calendars() -> Result<Vec<String>, String> {
    Err("Apple Calendar доступен только на macOS".into())
}

#[cfg(target_os = "macos")]
pub fn upsert(calendar_name: &str, record: &ActivityRecord) -> Result<(), String> {
    {
        let marker = format!("Demerzel activity UID: {}", record.uid);
        let payload = serde_json::json!({
            "calendar": calendar_name,
            "marker": marker,
            "summary": record.summary,
            "description": format!("{}\n\n{}", record.description, marker),
            "start": chrono::DateTime::parse_from_rfc3339(&record.started_at).map_err(|e| e.to_string())?.timestamp_millis(),
            "end": chrono::DateTime::parse_from_rfc3339(&record.finished_at).map_err(|e| e.to_string())?.timestamp_millis(),
        });
        let data = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        let script = format!(
            "const d = {}; const app = Application('Calendar'); const c = app.calendars.whose({{name: d.calendar}})()[0]; if (!c) throw new Error('Calendar not found: ' + d.calendar); const found = c.events().filter(e => (e.description() || '').includes(d.marker)); const props = {{summary: d.summary, startDate: new Date(d.start), endDate: new Date(Math.max(d.end, d.start + 1000)), description: d.description}}; if (found.length) {{ found[0].summary = props.summary; found[0].startDate = props.startDate; found[0].endDate = props.endDate; found[0].description = props.description; }} else {{ c.events.push(app.CalendarEvent(props)); }}",
            data
        );
        run_script(&script)?;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn upsert(calendar_name: &str, record: &ActivityRecord) -> Result<(), String> {
    let _ = (calendar_name, record);
    Err("Apple Calendar доступен только на macOS".into())
}
