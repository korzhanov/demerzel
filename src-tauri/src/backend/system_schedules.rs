//! Системные расписания: crontab и launchd — только чтение (#153, этап E).
//! По умолчанию выключено (`settings.system_schedules.enabled`). Demerzel ничего не запускает
//! и не меняет в системных расписаниях: только показывает, что там есть.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemJob {
    /// `crontab` или `launchd`.
    pub source: String,
    /// Имя задачи: для cron — команда, для launchd — метка (label).
    pub name: String,
    /// Расписание для cron (например `*/5 * * * *`), для launchd — `None`.
    pub schedule: Option<String>,
    /// Текущее состояние для launchd: PID, если запущено; для cron — `None`.
    pub pid: Option<u32>,
    /// Последний код выхода для launchd; для cron — `None`.
    pub last_exit: Option<i32>,
}

const CRON_SPECIALS: &[&str] = &["@reboot", "@yearly", "@annually", "@monthly", "@weekly", "@daily", "@midnight", "@hourly"];

/// Разбор `crontab -l`. Пропускает комментарии, пустые строки и строки `NAME=value`.
pub fn parse_crontab(text: &str) -> Vec<SystemJob> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `KEY=value` без пробелов в ключе — переменная окружения, не задача.
        if let Some(eq) = line.find('=')
            && !line[..eq].is_empty()
            && line[..eq].chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        if let Some(first) = line.split_whitespace().next()
            && CRON_SPECIALS.contains(&first)
        {
            let mut parts = line.splitn(2, char::is_whitespace);
            let schedule = parts.next().map(str::to_string);
            let command = parts.next().unwrap_or("").trim().to_string();
            out.push(SystemJob { source: "crontab".into(), name: command, schedule, pid: None, last_exit: None });
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 6 {
            continue;
        }
        let schedule = fields[..5].join(" ");
        let command = fields[5..].join(" ");
        out.push(SystemJob { source: "crontab".into(), name: command, schedule: Some(schedule), pid: None, last_exit: None });
    }
    out
}

/// Разбор `launchctl list`: колонки PID, Status, Label. Системные `com.apple.*` скрываются.
pub fn parse_launchctl_list(text: &str) -> Vec<SystemJob> {
    let mut out = Vec::new();
    for raw in text.lines().skip(1) {
        let cols: Vec<&str> = raw.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }
        let label = cols[2..].join(" ");
        if label.starts_with("com.apple.") {
            continue;
        }
        let pid = cols[0].parse::<u32>().ok();
        let last_exit = cols[1].parse::<i32>().ok();
        out.push(SystemJob { source: "launchd".into(), name: label, schedule: None, pid, last_exit });
    }
    out
}

/// Читает системные расписания. Вызывается только при включённой настройке.
#[cfg(unix)]
pub fn read_system_jobs() -> Vec<SystemJob> {
    let mut jobs = Vec::new();
    if let Ok(out) = std::process::Command::new("crontab").arg("-l").output()
        && out.status.success()
    {
        jobs.extend(parse_crontab(&String::from_utf8_lossy(&out.stdout)));
    }
    if let Ok(out) = std::process::Command::new("launchctl").arg("list").output()
        && out.status.success()
    {
        jobs.extend(parse_launchctl_list(&String::from_utf8_lossy(&out.stdout)));
    }
    jobs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crontab_skips_comments_env_and_parses_jobs() {
        let text = "# комментарий\nMAILTO=me@example.com\n*/5 * * * * /usr/bin/backup --fast\n@daily /bin/cleanup\n\n";
        let jobs = parse_crontab(text);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].schedule.as_deref(), Some("*/5 * * * *"));
        assert_eq!(jobs[0].name, "/usr/bin/backup --fast");
        assert_eq!(jobs[1].schedule.as_deref(), Some("@daily"));
        assert_eq!(jobs[1].name, "/bin/cleanup");
    }

    #[test]
    fn launchctl_hides_apple_and_reads_pid_and_exit() {
        let text = "PID\tStatus\tLabel\n4312\t0\tapp.demerzel.worker\n-\t78\tcom.apple.something\n-\t0\tcom.example.nightly job\n";
        let jobs = parse_launchctl_list(text);
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert_eq!(names, vec!["app.demerzel.worker", "com.example.nightly job"]);
        assert_eq!(jobs[0].pid, Some(4312));
        assert_eq!(jobs[0].last_exit, Some(0));
        assert_eq!(jobs[1].pid, None);
    }
}
