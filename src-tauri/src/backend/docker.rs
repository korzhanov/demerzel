//! Docker Compose runtime for `type: docker-compose` services (#42).
//!
//! - `dem up` → `docker compose -f <file> up -d <service>`
//! - `dem down` → `docker compose stop <service>`
//! - status via `docker compose ps --all --format json` (array or JSON lines)
//! - контейнерный `Health` из Compose; опциональный YAML `healthcheck` — на стороне CLI

use super::ecosystem::Service;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "State")]
    pub state: String,
    #[serde(default, rename = "Health")]
    pub health: String,
}

pub fn parse_status(text: &str) -> Result<Vec<Container>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    if text.trim_start().starts_with('[') {
        serde_json::from_str(text).map_err(|e| e.to_string())
    } else {
        text.lines()
            .filter(|s| !s.trim().is_empty())
            .map(|s| serde_json::from_str(s).map_err(|e| e.to_string()))
            .collect()
    }
}

/// Compose без healthcheck отдаёт пустой `Health` или `"none"` — считаем ок при `running`.
pub fn health_ok(health: &str) -> bool {
    let h = health.trim();
    h.is_empty() || h.eq_ignore_ascii_case("healthy") || h.eq_ignore_ascii_case("none")
}

/// Сервис готов: есть контейнеры, все `running`, health не в starting/unhealthy.
pub fn containers_ready(containers: &[Container]) -> bool {
    !containers.is_empty()
        && containers
            .iter()
            .all(|c| c.state == "running" && health_ok(&c.health))
}

pub fn require_ready(containers: &[Container]) -> Result<(), String> {
    if containers_ready(containers) {
        return Ok(());
    }
    if containers.is_empty() {
        return Err("no Compose containers; is Docker running and the service defined?".into());
    }
    let detail = containers
        .iter()
        .map(|c| {
            if c.health.trim().is_empty() {
                format!("{}={}", c.service, c.state)
            } else {
                format!("{}={}({})", c.service, c.state, c.health)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!("Compose service not ready: {detail}"))
}

pub fn command(svc: &Service, action: &str) -> Result<String, String> {
    let file = svc.file.as_ref().ok_or("docker-compose file required")?;
    let service = svc.service.as_ref().ok_or("docker-compose service required")?;
    let mut cmd = std::process::Command::new("docker");
    cmd.arg("compose").arg("-f").arg(file).current_dir(&svc.path);
    match action {
        "up" => {
            cmd.args(["up", "-d", service]);
        }
        "down" => {
            cmd.args(["stop", service]);
        }
        "status" => {
            cmd.args(["ps", "--all", "--format", "json", service]);
        }
        _ => return Err("unknown Compose action".into()),
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Docker is unavailable. Install/start Docker Desktop: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Docker Compose failed; check that Docker is running: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_compose_array_and_json_lines() {
        let line = r#"{"Service":"db","State":"running","Health":"healthy"}"#;
        assert_eq!(parse_status(line).unwrap()[0].health, "healthy");
        assert_eq!(parse_status(&format!("[{line}]")).unwrap()[0].service, "db");
        assert_eq!(parse_status(&format!("{line}\n{line}\n")).unwrap().len(), 2);
        assert!(parse_status("broken").is_err());
    }

    #[test]
    fn health_treats_empty_and_none_as_ok() {
        assert!(health_ok(""));
        assert!(health_ok("none"));
        assert!(health_ok("healthy"));
        assert!(!health_ok("starting"));
        assert!(!health_ok("unhealthy"));
    }

    #[test]
    fn ready_requires_running_and_ok_health() {
        let ok = Container {
            service: "db".into(),
            state: "running".into(),
            health: "healthy".into(),
        };
        let no_hc = Container {
            service: "redis".into(),
            state: "running".into(),
            health: String::new(),
        };
        let bad = Container {
            service: "db".into(),
            state: "running".into(),
            health: "starting".into(),
        };
        assert!(containers_ready(std::slice::from_ref(&ok)));
        assert!(containers_ready(&[no_hc]));
        assert!(!containers_ready(&[]));
        assert!(!containers_ready(std::slice::from_ref(&bad)));
        assert!(require_ready(std::slice::from_ref(&ok)).is_ok());
        assert!(require_ready(std::slice::from_ref(&bad))
            .unwrap_err()
            .contains("not ready"));
    }
}
