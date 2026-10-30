//! Корневой манифест экосистемы `demerzel.yaml` (#41).
//!
//! Описывает сервисы монорепо (`path`, `cmd`, `port`, `healthcheck`,
//! опционально `type: docker-compose`). Параметры проекта остаются в
//! `orchestrator.json`. Режим shadow и desktop-уведомления — в настройках
//! приложения, не в манифесте (`deny_unknown_fields`).
//!
//! Документация: `docs/demerzel-yaml.md`, схема: `docs/demerzel.schema.json`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub path: PathBuf,
    #[serde(default)]
    pub cmd: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub healthcheck: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub service: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub services: BTreeMap<String, Service>,
}

pub fn parse(text: &str) -> Result<Manifest, String> {
    let manifest: Manifest = serde_yaml::from_str(text).map_err(|e| format!("demerzel.yaml: {e}"))?;
    if manifest.services.is_empty() {
        return Err("demerzel.yaml: services must not be empty".into());
    }
    let mut ports = BTreeSet::new();
    for (name, svc) in &manifest.services {
        if name.trim().is_empty() || svc.path.as_os_str().is_empty() {
            return Err("service name/path must not be empty".into());
        }
        if let Some(port) = svc.port {
            if port == 0 || !ports.insert(port) {
                return Err(format!("{name}: zero or duplicate port {port}"));
            }
        }
        match svc.kind.as_deref() {
            None | Some("process") => {
                if svc.file.is_some() || svc.service.is_some() {
                    return Err(format!("{name}: file/service require docker-compose type"));
                }
            }
            Some("docker-compose") => {
                if svc.file.as_ref().is_none_or(|p| p.as_os_str().is_empty())
                    || svc
                        .service
                        .as_ref()
                        .is_none_or(|s| s.is_empty() || s.starts_with('-'))
                {
                    return Err(format!("{name}: docker-compose requires file and service"));
                }
            }
            Some(kind) => return Err(format!("{name}: unknown type {kind}")),
        }
        if svc.cmd.as_ref().is_some_and(|s| s.trim().is_empty())
            || svc.healthcheck.as_ref().is_some_and(|s| s.trim().is_empty())
        {
            return Err(format!("{name}: commands must not be empty"));
        }
    }
    Ok(manifest)
}

pub fn load(root: &Path) -> Result<Manifest, String> {
    let file = root.join("demerzel.yaml");
    let manifest = match std::fs::read_to_string(&file) {
        Ok(text) => parse(&text)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let cfg = super::config::load_checked(root)?;
            let name = cfg
                .as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| {
                    root.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into()
                });
            Manifest {
                services: BTreeMap::from([(
                    name,
                    Service {
                        path: root.into(),
                        cmd: super::config::resolve_dev_command(root),
                        port: None,
                        healthcheck: None,
                        kind: None,
                        file: None,
                        service: None,
                    },
                )]),
            }
        }
        Err(e) => return Err(format!("{}: {e}", file.display())),
    };
    let mut resolved = manifest;
    let mut reservations = super::ports::Allocator::default();
    for (name, svc) in &mut resolved.services {
        svc.path = root
            .join(&svc.path)
            .canonicalize()
            .map_err(|e| format!("{name}: {e}"))?;
        if !svc.path.is_dir() {
            return Err(format!("{name}: path is not a directory"));
        }
        if let Some(file) = &svc.file {
            svc.file = Some(svc.path.join(file));
        }
        if let Some(cfg) = super::config::load_checked(&svc.path)? {
            if let Some(ports) = cfg.ports {
                let range = super::ports::PortRange::new(ports.range[0], ports.range[1])
                    .ok_or("invalid port range")?;
                reservations.reserve(name, range)?;
                if svc.port.is_some_and(|p| !range.contains(p)) {
                    return Err(format!("{name}: manifest port is outside project range"));
                }
            }
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-eco-{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn validates_manifest() {
        assert!(parse(
            "services:\n  api:\n    path: ./api\n    port: 5100\n  db:\n    path: .\n    type: docker-compose\n    file: compose.yaml\n    service: postgres\n"
        )
        .is_ok());
        assert!(parse("services:\n  api:\n    path: .\n    type: process\n    cmd: bun run dev\n").is_ok());
        assert!(parse("services: {}\n").is_err());
        assert!(parse("services:\n  api:\n    path: .\n    type: docker-compose\n").is_err());
        assert!(parse("mode: shadow\nservices:\n  api:\n    path: .\n").is_err());
        assert!(parse("notifications: desktop\nservices:\n  api:\n    path: .\n").is_err());
        assert!(parse(
            "services:\n  a:\n    path: .\n    port: 50\n  b:\n    path: .\n    port: 50\n"
        )
        .is_err());
        assert!(parse("services:\n  api:\n    path: .\n    cmd: '  '\n").is_err());
        assert!(parse("services:\n  api:\n    path: .\n    type: k8s\n").is_err());
        assert!(parse(
            "services:\n  api:\n    path: .\n    type: process\n    file: compose.yaml\n"
        )
        .is_err());
    }

    #[test]
    fn load_falls_back_to_orchestrator_json() {
        let root = tmp_root("fallback");
        fs::write(
            root.join("orchestrator.json"),
            r#"{"name":"solo","type":"app","commands":{"dev":"bun run dev"}}"#,
        )
        .unwrap();
        let m = load(&root).unwrap();
        assert_eq!(m.services.len(), 1);
        let svc = m.services.get("solo").unwrap();
        assert_eq!(svc.cmd.as_deref(), Some("bun run dev"));
        assert!(svc.path.is_dir());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_reads_yaml_and_resolves_paths() {
        let root = tmp_root("yaml");
        let api = root.join("api");
        fs::create_dir_all(&api).unwrap();
        fs::write(
            api.join("orchestrator.json"),
            r#"{"name":"api","type":"service","ports":{"range":[5100,5199]}}"#,
        )
        .unwrap();
        fs::write(
            root.join("demerzel.yaml"),
            "services:\n  api:\n    path: ./api\n    port: 5100\n    healthcheck: curl -sf http://127.0.0.1:5100/\n",
        )
        .unwrap();
        let m = load(&root).unwrap();
        let svc = m.services.get("api").unwrap();
        assert_eq!(svc.port, Some(5100));
        assert_eq!(svc.path, api.canonicalize().unwrap());
        assert!(svc.healthcheck.as_ref().unwrap().contains("curl"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_rejects_port_outside_project_range() {
        let root = tmp_root("portrange");
        let api = root.join("api");
        fs::create_dir_all(&api).unwrap();
        fs::write(
            api.join("orchestrator.json"),
            r#"{"name":"api","type":"service","ports":{"range":[5100,5199]}}"#,
        )
        .unwrap();
        fs::write(
            root.join("demerzel.yaml"),
            "services:\n  api:\n    path: ./api\n    port: 6000\n",
        )
        .unwrap();
        let err = load(&root).unwrap_err();
        assert!(err.contains("outside project range"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }
}
