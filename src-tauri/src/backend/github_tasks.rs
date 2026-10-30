//! GitHub Issues over REST, authenticated by the user's `gh` session.
use super::tasks::{Task, TaskSource, TaskStatus};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct CachedPage {
    etag: Option<String>,
    issues: Vec<serde_json::Value>,
    fetched: Instant,
}
static CACHE: OnceLock<Mutex<BTreeMap<String, CachedPage>>> = OnceLock::new();
const MIN_REFRESH: Duration = Duration::from_secs(60);

pub struct GithubSource {
    pub repository: String,
    pub label: Option<String>,
}

fn encode(value: &str) -> String {
    value.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
        (b as char).to_string()
    } else { format!("%{b:02X}") }).collect()
}

fn validate_repository(repo: &str) -> Result<(), String> {
    let parts: Vec<_> = repo.split('/').collect();
    if parts.len() != 2 || parts.iter().any(|s| s.is_empty() || s.starts_with('.') || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))) {
        return Err("GitHub repository must be owner/name".into());
    }
    Ok(())
}

fn page(endpoint: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut cache = CACHE.get_or_init(Default::default).lock().map_err(|e| e.to_string())?;
    if let Some(entry) = cache.get(endpoint) {
        if entry.fetched.elapsed() < MIN_REFRESH { return Ok(entry.issues.clone()); }
    }
    let mut cmd = std::process::Command::new("gh");
    cmd.args(["api", "--hostname", "github.com", "--include", "--method", "GET", endpoint,
        "-H", "Accept: application/vnd.github+json", "-H", "X-GitHub-Api-Version: 2022-11-28"]);
    if let Some(etag) = cache.get(endpoint).and_then(|c| c.etag.as_ref()) {
        cmd.args(["-H", &format!("If-None-Match: {etag}")]);
    }
    let out = cmd.output().map_err(|e| format!("GitHub: install gh and run gh auth login: {e}"))?;
    let raw = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    let (headers, body) = raw.split_once("\n\n").ok_or_else(|| format!("GitHub: {}", String::from_utf8_lossy(&out.stderr)))?;
    let status = headers.lines().next().and_then(|s| s.split_whitespace().nth(1)).unwrap_or("");
    if status == "304" {
        let cached = cache.get_mut(endpoint).ok_or("GitHub returned 304 without a cached response")?;
        cached.fetched = Instant::now();
        return Ok(cached.issues.clone());
    }
    if !out.status.success() || status != "200" {
        return Err(format!("GitHub REST {status}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let issues: Vec<serde_json::Value> = serde_json::from_str(body).map_err(|e| format!("GitHub response: {e}"))?;
    let etag = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("etag").then(|| value.trim().into())
    });
    cache.insert(endpoint.into(), CachedPage { etag, issues: issues.clone(), fetched: Instant::now() });
    Ok(issues)
}

pub fn normalize(issues: &[serde_json::Value], project: &str, repo: &str) -> Vec<Task> {
    issues.iter().filter(|v| v.get("pull_request").is_none()).filter_map(|issue| {
        let number = issue.get("number")?.as_u64()?;
        Some(Task {
            id: format!("github:{repo}#{number}"), project: project.into(),
            title: issue.get("title")?.as_str()?.into(),
            status: if issue.get("state")?.as_str()? == "closed" { TaskStatus::Done } else { TaskStatus::Todo },
            source: "github".into(), url: issue.get("html_url").and_then(|v| v.as_str()).map(Into::into),
        })
    }).collect()
}

impl TaskSource for GithubSource {
    fn name(&self) -> &str { "github" }
    fn fetch(&self, project: &str) -> Result<Vec<Task>, String> {
        validate_repository(&self.repository)?;
        let mut tasks = Vec::new();
        for number in 1..=100 {
            let label = self.label.as_ref().map(|l| format!("&labels={}", encode(l))).unwrap_or_default();
            let endpoint = format!("repos/{}/issues?state=open&per_page=100&page={number}{label}", self.repository);
            let issues = page(&endpoint)?;
            tasks.extend(normalize(&issues, project, &self.repository));
            if issues.len() < 100 { return Ok(tasks); }
        }
        Err("GitHub issue pagination exceeded 100 pages; narrow the label filter".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_pull_requests_and_namespaces_ids() {
        let issues = serde_json::from_str::<Vec<serde_json::Value>>(r#"[
          {"number":1,"title":"Fix","state":"open","html_url":"https://github.com/a/b/issues/1"},
          {"number":2,"title":"PR","state":"open","pull_request":{}},
          {"number":3,"title":"Done","state":"closed"}]
        "#).unwrap();
        let tasks = normalize(&issues, "/project", "a/b");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "github:a/b#1");
        assert_eq!(tasks[1].status, TaskStatus::Done);
    }
    #[test]
    fn repository_and_query_are_safe() {
        assert!(validate_repository("a/b").is_ok());
        assert!(validate_repository("../issues?q=x").is_err());
        assert!(validate_repository("https://evil.test/a/b").is_err());
        assert_eq!(encode("bug & fix"), "bug%20%26%20fix");
    }
}
