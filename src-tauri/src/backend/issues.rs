//! Задачи проекта как GitHub issues: создание через `gh api` (сессия пользователя)
//! и заготовки «fix» из ошибок и предупреждений в логе процесса.

use std::collections::HashSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;

use super::logs::LogLine;

/// Ограничение длины заголовка fix-задачи (символов).
pub const TITLE_MAX: usize = 80;
/// Сколько последних строк-проблем кладём в описание.
pub const MAX_PROBLEM_LINES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueRef {
    /// owner/name
    pub repository: String,
    pub number: u64,
    pub url: String,
}

/// owner/name из адреса GitHub remote: `https://github.com/o/r(.git)`, `git@github.com:o/r.git`, `ssh://git@github.com/o/r`.
pub fn parse_github_remote(url: &str) -> Option<String> {
    let t = url.trim();
    let rest = if let Some(r) = t.strip_prefix("git@github.com:") {
        r
    } else {
        let i = t.find("github.com/")?;
        &t[i + "github.com/".len()..]
    };
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() || owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

/// Репозиторий для задач: явный `tasks.repository` из конфига, иначе `origin` проекта.
pub fn repository_for(project_path: &Path, configured: Option<&str>) -> Result<String, String> {
    if let Some(repo) = configured.map(str::trim).filter(|r| !r.is_empty()) {
        super::github_tasks::validate_repository(repo)?;
        return Ok(repo.to_string());
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err("у проекта нет remote origin: укажите tasks.repository в orchestrator.json".into());
    }
    let url = String::from_utf8_lossy(&out.stdout).into_owned();
    parse_github_remote(&url)
        .ok_or_else(|| "remote origin не похож на GitHub: укажите tasks.repository в orchestrator.json".into())
}

/// Тело запроса на создание issue.
pub fn issue_payload(title: &str, body: &str) -> String {
    serde_json::json!({ "title": title, "body": body }).to_string()
}

/// Разбирает ответ GitHub на создание issue.
pub fn parse_issue_response(repository: &str, text: &str) -> Result<IssueRef, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("GitHub response: {e}"))?;
    let number = v["number"].as_u64().ok_or("GitHub response: нет номера issue")?;
    let url = v["html_url"].as_str().ok_or("GitHub response: нет html_url")?.to_string();
    Ok(IssueRef { repository: repository.to_string(), number, url })
}

/// Создаёт issue через `gh api` (тело передаём через stdin, чтобы не упереться в длину аргументов).
pub fn create_issue(repository: &str, title: &str, body: &str) -> Result<IssueRef, String> {
    super::github_tasks::validate_repository(repository)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("у задачи должно быть название".into());
    }
    let mut child = Command::new("gh")
        .args([
            "api",
            "--hostname",
            "github.com",
            "--method",
            "POST",
            &format!("repos/{repository}/issues"),
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("GitHub: установите gh и выполните gh auth login: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(issue_payload(title, body.trim()).as_bytes())
            .map_err(|e| format!("GitHub: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("GitHub: {e}"))?;
    if !out.status.success() {
        return Err(format!("GitHub: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    parse_issue_response(repository, &String::from_utf8_lossy(&out.stdout))
}

/// Убирает ANSI-цвета из строки лога.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Строка лога похожа на ошибку или предупреждение.
pub fn is_problem_line(text: &str) -> bool {
    let lower = strip_ansi(text).to_lowercase();
    ["error", "warn", "exception", "failed", "panic", "traceback", "fatal"]
        .iter()
        .any(|k| lower.contains(k))
}

/// Проблемные строки лога: без дублей и пустых, не больше `MAX_PROBLEM_LINES` последних.
pub fn problem_lines(lines: &[LogLine]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for line in lines.iter().filter(|l| is_problem_line(&l.text)) {
        let text = strip_ansi(&line.text).trim().to_string();
        if text.is_empty() || !seen.insert(text.clone()) {
            continue;
        }
        out.push(text);
    }
    if out.len() > MAX_PROBLEM_LINES {
        let drop = out.len() - MAX_PROBLEM_LINES;
        out.drain(..drop);
    }
    out
}

/// Короткий заголовок fix-задачи из самой свежей проблемной строки.
pub fn fix_title(problems: &[String]) -> String {
    let last = problems.last().map(String::as_str).unwrap_or("неизвестная ошибка");
    let room = TITLE_MAX.saturating_sub("fix: ".chars().count());
    let shortened: String = if last.chars().count() > room {
        let mut s: String = last.chars().take(room.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        last.to_string()
    };
    format!("fix: {shortened}")
}

/// Описание fix-задачи: контекст процесса и фрагмент лога.
pub fn fix_body(project_name: &str, command: &str, problems: &[String]) -> String {
    format!(
        "Демерзель нашла ошибки и предупреждения в логе процесса.\n\nПроект: {project_name}\nКоманда: `{command}`\n\n```\n{}\n```\n",
        problems.join("\n")
    )
}

/// Промпт агенту: задача из issue плюс просьба исправить причину.
pub fn autofix_prompt(title: &str, body: &str, url: &str) -> String {
    format!(
        "{title}\n\n{body}\n\nИсточник: {url}\n\nНайди причину ошибки в этом проекте и исправь её минимальным изменением. В конце кратко напиши, что сделано."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::logs::Stream;

    fn line(text: &str) -> LogLine {
        LogLine { stream: Stream::Stderr, text: text.into() }
    }

    #[test]
    fn parses_github_remote_forms() {
        assert_eq!(parse_github_remote("https://github.com/korzhanov/demerzel.git").as_deref(), Some("korzhanov/demerzel"));
        assert_eq!(parse_github_remote("git@github.com:korzhanov/demerzel.git\n").as_deref(), Some("korzhanov/demerzel"));
        assert_eq!(parse_github_remote("ssh://git@github.com/o/r").as_deref(), Some("o/r"));
        assert_eq!(parse_github_remote("https://gitlab.com/o/r"), None);
        assert_eq!(parse_github_remote("https://github.com/o"), None);
    }

    #[test]
    fn problem_lines_keep_latest_unique_and_strip_colors() {
        let lines = vec![
            line("ready"),
            line("\u{1b}[31mError: boom\u{1b}[0m"),
            line("Error: boom"),
            line("warning: unused variable"),
        ];
        assert_eq!(problem_lines(&lines), vec!["Error: boom", "warning: unused variable"]);
    }

    #[test]
    fn problem_lines_are_capped() {
        let lines: Vec<LogLine> = (0..30).map(|i| line(&format!("error {i}"))).collect();
        let got = problem_lines(&lines);
        assert_eq!(got.len(), MAX_PROBLEM_LINES);
        assert_eq!(got.last().map(String::as_str), Some("error 29"));
    }

    #[test]
    fn fix_title_is_short_and_prefixed() {
        let long = "x".repeat(200);
        let title = fix_title(&[long]);
        assert!(title.starts_with("fix: "));
        assert!(title.chars().count() <= TITLE_MAX);
        assert_eq!(fix_title(&[]), "fix: неизвестная ошибка");
    }

    #[test]
    fn parses_issue_response_and_payload() {
        let issue = parse_issue_response("o/r", r#"{"number": 12, "html_url": "https://github.com/o/r/issues/12"}"#).unwrap();
        assert_eq!(issue.number, 12);
        assert_eq!(issue.repository, "o/r");
        assert!(parse_issue_response("o/r", "{}").is_err());
        let payload: serde_json::Value = serde_json::from_str(&issue_payload("t", "b")).unwrap();
        assert_eq!(payload["title"], "t");
        assert_eq!(payload["body"], "b");
    }

    #[test]
    fn create_issue_rejects_empty_title_before_calling_gh() {
        assert!(create_issue("o/r", "   ", "body").is_err());
    }
}
