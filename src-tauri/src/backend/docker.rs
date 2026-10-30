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
    if text.trim().is_empty() { return Ok(Vec::new()); }
    if text.trim_start().starts_with('[') {
        serde_json::from_str(text).map_err(|e| e.to_string())
    } else {
        text.lines().filter(|s| !s.trim().is_empty()).map(|s| serde_json::from_str(s).map_err(|e| e.to_string())).collect()
    }
}

pub fn command(svc: &Service, action: &str) -> Result<String, String> {
    let file = svc.file.as_ref().ok_or("docker-compose file required")?;
    let service = svc.service.as_ref().ok_or("docker-compose service required")?;
    let mut cmd = std::process::Command::new("docker");
    cmd.arg("compose").arg("-f").arg(file).current_dir(&svc.path);
    match action {
        "up" => { cmd.args(["up", "-d", service]); }
        "down" => { cmd.args(["stop", service]); }
        "status" => { cmd.args(["ps", "--all", "--format", "json", service]); }
        _ => return Err("unknown Compose action".into()),
    }
    let output = cmd.output().map_err(|e| format!("Docker is unavailable. Install/start Docker Desktop: {e}"))?;
    if !output.status.success() {
        return Err(format!("Docker Compose failed; check that Docker is running: {}", String::from_utf8_lossy(&output.stderr)));
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
}
