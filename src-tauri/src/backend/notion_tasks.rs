//! Notion database pages over REST (`curl`), с кэшем (#36).
//! Авторизация — `NOTION_TOKEN` (integration secret); в `orchestrator.json` не хранится.
use super::tasks::{Task, TaskSource, TaskStatus};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

struct CachedQuery {
    pages: Vec<serde_json::Value>,
    next_cursor: Option<String>,
    fetched: Instant,
}

static CACHE: OnceLock<Mutex<BTreeMap<String, CachedQuery>>> = OnceLock::new();
/// Минимальный интервал между повторными REST-запросами к одной БД (rate limit).
const MIN_REFRESH: Duration = Duration::from_secs(60);
const NOTION_VERSION: &str = "2022-06-28";
const DEFAULT_TITLE_PROP: &str = "Name";
const DEFAULT_STATUS_PROP: &str = "Status";

/// Источник задач: страницы Notion database.
pub struct NotionSource {
    pub database_id: String,
    /// Имя property типа `title` (по умолчанию `Name`).
    pub title_property: Option<String>,
    /// Имя property статуса: `status` / `select` / `checkbox` (по умолчанию `Status`).
    pub status_property: Option<String>,
}

fn validate_database_id(id: &str) -> Result<String, String> {
    let compact: String = id.chars().filter(|c| *c != '-').collect();
    if compact.len() != 32 || !compact.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Notion database_id must be a 32-char hex UUID (dashes optional)".into());
    }
    Ok(compact.to_ascii_lowercase())
}

fn token_from_env() -> Result<String, String> {
    std::env::var("NOTION_TOKEN")
        .map(|t| t.trim().to_string())
        .map_err(|_| "Notion: set NOTION_TOKEN (integration secret)".to_string())
        .and_then(|t| {
            if t.is_empty() {
                Err("Notion: NOTION_TOKEN is empty".into())
            } else {
                Ok(t)
            }
        })
}

fn http_status_from_headers(headers: &str) -> &str {
    headers
        .lines()
        .next()
        .and_then(|s| s.split_whitespace().nth(1))
        .unwrap_or("")
}

fn plain_title(prop: &serde_json::Value) -> Option<String> {
    let parts = prop.get("title")?.as_array()?;
    let text: String = parts
        .iter()
        .filter_map(|p| p.get("plain_text").and_then(|v| v.as_str()))
        .collect();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.into())
    }
}

fn find_title(
    properties: &serde_json::Map<String, serde_json::Value>,
    preferred: &str,
) -> Option<String> {
    if let Some(prop) = properties.get(preferred)
        && prop.get("type").and_then(|t| t.as_str()) == Some("title") {
            return plain_title(prop);
        }
    properties.values().find_map(|prop| {
        (prop.get("type").and_then(|t| t.as_str()) == Some("title")).then(|| plain_title(prop))?
    })
}

fn map_status_name(name: &str) -> TaskStatus {
    let lower = name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "done" | "complete" | "completed" | "closed" | "готово" | "сделано" => TaskStatus::Done,
        "in progress" | "in_progress" | "doing" | "wip" | "в работе" => TaskStatus::InProgress,
        "blocked" | "waiting" | "on hold" | "заблокировано" | "ждём" => TaskStatus::Blocked,
        _ => TaskStatus::Todo,
    }
}

fn status_from_property(prop: &serde_json::Value) -> TaskStatus {
    match prop.get("type").and_then(|t| t.as_str()) {
        Some("status") => prop
            .get("status")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .map(map_status_name)
            .unwrap_or(TaskStatus::Todo),
        Some("select") => prop
            .get("select")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .map(map_status_name)
            .unwrap_or(TaskStatus::Todo),
        Some("checkbox") => {
            if prop.get("checkbox").and_then(|v| v.as_bool()) == Some(true) {
                TaskStatus::Done
            } else {
                TaskStatus::Todo
            }
        }
        _ => TaskStatus::Todo,
    }
}

/// Нормализация JSON pages → `Task`. Archived → `done`.
pub fn normalize(
    pages: &[serde_json::Value],
    project: &str,
    database_id: &str,
    title_property: &str,
    status_property: &str,
) -> Vec<Task> {
    pages
        .iter()
        .filter_map(|page| {
            let id = page.get("id")?.as_str()?;
            let props = page.get("properties")?.as_object()?;
            let title = find_title(props, title_property)?;
            let archived = page.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
            let status = if archived {
                TaskStatus::Done
            } else if let Some(prop) = props.get(status_property) {
                status_from_property(prop)
            } else {
                TaskStatus::Todo
            };
            Some(Task {
                id: format!("notion:{database_id}:{id}"),
                project: project.into(),
                title,
                status,
                source: "notion".into(),
                url: page
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(Into::into),
            })
        })
        .collect()
}

fn query_page(
    database_id: &str,
    token: &str,
    start_cursor: Option<&str>,
) -> Result<(Vec<serde_json::Value>, Option<String>), String> {
    let cache_key = format!(
        "notion:{database_id}:{}",
        start_cursor.unwrap_or("start")
    );
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    if let Some(entry) = cache.get(&cache_key)
        && entry.fetched.elapsed() < MIN_REFRESH {
            return Ok((entry.pages.clone(), entry.next_cursor.clone()));
        }

    let mut body = serde_json::json!({ "page_size": 100 });
    if let Some(cursor) = start_cursor {
        body["start_cursor"] = serde_json::Value::String(cursor.into());
    }

    let url = format!("https://api.notion.com/v1/databases/{database_id}/query");
    let out = std::process::Command::new("curl")
        .args([
            "-sS",
            "-D",
            "-",
            "-X",
            "POST",
            &url,
            "-H",
            &format!("Authorization: Bearer {token}"),
            "-H",
            &format!("Notion-Version: {NOTION_VERSION}"),
            "-H",
            "Content-Type: application/json",
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| format!("Notion: install curl: {e}"))?;

    if !out.status.success() && out.stdout.is_empty() {
        return Err(format!(
            "Notion curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let raw = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    let (headers, response_body) = raw.split_once("\n\n").ok_or_else(|| {
        format!(
            "Notion: bad response: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    })?;
    let status = http_status_from_headers(headers);
    if status != "200" {
        return Err(format!(
            "Notion REST {status}: {}",
            response_body.chars().take(400).collect::<String>()
        ));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(response_body).map_err(|e| format!("Notion response: {e}"))?;
    let pages = parsed
        .get("results")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let next = if parsed
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        parsed
            .get("next_cursor")
            .and_then(|c| c.as_str())
            .map(str::to_string)
    } else {
        None
    };

    cache.insert(
        cache_key,
        CachedQuery {
            pages: pages.clone(),
            next_cursor: next.clone(),
            fetched: Instant::now(),
        },
    );
    Ok((pages, next))
}

impl TaskSource for NotionSource {
    fn name(&self) -> &str {
        "notion"
    }

    fn fetch(&self, project: &str) -> Result<Vec<Task>, String> {
        let database_id = validate_database_id(&self.database_id)?;
        let token = token_from_env()?;
        let title_prop = self
            .title_property
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_TITLE_PROP);
        let status_prop = self
            .status_property
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_STATUS_PROP);

        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let (pages, next) = query_page(&database_id, &token, cursor.as_deref())?;
            let page_count = pages.len();
            all.extend(normalize(
                &pages,
                project,
                &database_id,
                title_prop,
                status_prop,
            ));
            match next {
                Some(c) if page_count > 0 => cursor = Some(c),
                _ => {
                    return Ok(all);
                }
            }
        }
        Err("Notion pagination exceeded 100 pages; narrow the database filter".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_database_id_hex_uuid() {
        assert_eq!(
            validate_database_id("01234567-89ab-cdef-0123-456789abcdef").unwrap(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            validate_database_id("0123456789ABCDEF0123456789abcdef").unwrap(),
            "0123456789abcdef0123456789abcdef"
        );
        assert!(validate_database_id("short").is_err());
        assert!(validate_database_id("../evil").is_err());
        assert!(validate_database_id("https://notion.so/x").is_err());
        assert!(validate_database_id("gggggggggggggggggggggggggggggggg").is_err());
    }

    #[test]
    fn normalize_maps_status_select_checkbox_and_archived() {
        let db = "0123456789abcdef0123456789abcdef";
        let pages = serde_json::from_str::<Vec<serde_json::Value>>(
            r#"[
          {
            "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "url": "https://www.notion.so/aaa",
            "archived": false,
            "properties": {
              "Name": {"type":"title","title":[{"plain_text":"Open task"}]},
              "Status": {"type":"status","status":{"name":"Not started"}}
            }
          },
          {
            "id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            "url": "https://www.notion.so/bbb",
            "archived": false,
            "properties": {
              "Name": {"type":"title","title":[{"plain_text":"WIP"}]},
              "Status": {"type":"select","select":{"name":"In progress"}}
            }
          },
          {
            "id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
            "url": "https://www.notion.so/ccc",
            "archived": false,
            "properties": {
              "Name": {"type":"title","title":[{"plain_text":"Done box"}]},
              "Status": {"type":"checkbox","checkbox":true}
            }
          },
          {
            "id": "dddddddd-dddd-dddd-dddd-dddddddddddd",
            "url": "https://www.notion.so/ddd",
            "archived": true,
            "properties": {
              "Name": {"type":"title","title":[{"plain_text":"Archived"}]},
              "Status": {"type":"status","status":{"name":"Not started"}}
            }
          },
          {
            "id": "eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee",
            "archived": false,
            "properties": {
              "Name": {"type":"title","title":[{"plain_text":"Blocked"}]},
              "Status": {"type":"status","status":{"name":"Blocked"}}
            }
          },
          {
            "id": "ffffffff-ffff-ffff-ffff-ffffffffffff",
            "archived": false,
            "properties": {
              "Name": {"type":"title","title":[]},
              "Status": {"type":"status","status":{"name":"Not started"}}
            }
          }
        ]"#,
        )
        .unwrap();
        let tasks = normalize(&pages, "/proj", db, "Name", "Status");
        assert_eq!(tasks.len(), 5);
        assert_eq!(tasks[0].id, format!("notion:{db}:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"));
        assert_eq!(tasks[0].source, "notion");
        assert_eq!(tasks[0].status, TaskStatus::Todo);
        assert_eq!(tasks[0].url.as_deref(), Some("https://www.notion.so/aaa"));
        assert_eq!(tasks[1].status, TaskStatus::InProgress);
        assert_eq!(tasks[2].status, TaskStatus::Done);
        assert_eq!(tasks[3].status, TaskStatus::Done);
        assert_eq!(tasks[4].status, TaskStatus::Blocked);
        assert_eq!(tasks[0].project, "/proj");
    }

    #[test]
    fn finds_first_title_property_when_preferred_missing() {
        let page = serde_json::json!({
            "id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "archived": false,
            "properties": {
              "Задача": {"type":"title","title":[{"plain_text":"Русский заголовок"}]},
              "Status": {"type":"status","status":{"name":"Done"}}
            }
        });
        let tasks = normalize(
            &[page],
            "/p",
            "0123456789abcdef0123456789abcdef",
            "Name",
            "Status",
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Русский заголовок");
        assert_eq!(tasks[0].status, TaskStatus::Done);
    }

    #[test]
    fn map_status_names_cover_aliases() {
        assert_eq!(map_status_name("готово"), TaskStatus::Done);
        assert_eq!(map_status_name("в работе"), TaskStatus::InProgress);
        assert_eq!(map_status_name("ждём"), TaskStatus::Blocked);
        assert_eq!(map_status_name("Backlog"), TaskStatus::Todo);
    }

    #[test]
    fn http_status_parsed_from_headers() {
        assert_eq!(
            http_status_from_headers("HTTP/1.1 200 OK\nContent-Type: application/json\n"),
            "200"
        );
        assert_eq!(http_status_from_headers("HTTP/2 401"), "401");
    }
}
