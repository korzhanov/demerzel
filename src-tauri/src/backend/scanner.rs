//! Поиск проектов и подпроектов (#6).
//!
//! Проект — папка с маркером (`package.json`, `.git`, `orchestrator.json`).
//! Подпроекты — вложенные папки с маркерами. Игнорируются `node_modules`,
//! `target`, скрытые папки и другие служебные каталоги.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Проект или подпроект, найденный в корневой папке.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Project {
    pub name: String,
    #[serde(serialize_with = "ser_path")]
    pub path: PathBuf,
    pub children: Vec<Project>,
}

fn ser_path<S: serde::Serializer>(p: &PathBuf, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&p.to_string_lossy())
}

/// Маркеры, по которым папка считается проектом.
pub const PROJECT_MARKERS: &[&str] = &["package.json", ".git", "orchestrator.json"];

/// Папки, которые никогда не сканируются.
pub const IGNORED_DIRS: &[&str] = &["node_modules", "target", "build", "dist"];

pub fn is_ignored(dir_name: &str) -> bool {
    IGNORED_DIRS.contains(&dir_name) || dir_name.starts_with('.')
}

pub fn is_project_dir(entries: &[&str]) -> bool {
    entries.iter().any(|e| PROJECT_MARKERS.contains(e))
}

/// Ограничение глубины, чтобы не уйти в огромные деревья.
pub const MAX_DEPTH: usize = 6;

/// Сканирует корневую папку и возвращает найденные проекты с подпроектами.
pub fn scan(root: &Path) -> io::Result<Vec<Project>> {
    scan_dir(root, 0)
}

fn scan_dir(dir: &Path, depth: usize) -> io::Result<Vec<Project>> {
    if depth > MAX_DEPTH {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored(&name) {
            continue;
        }
        let path = entry.path();
        let names = child_names(&path)?;
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let children = scan_dir(&path, depth + 1)?;
        if is_project_dir(&refs) {
            found.push(Project { name, path, children });
        } else {
            found.extend(children);
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

fn child_names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        names.push(entry?.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("demerzel-scan-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detects_project_by_marker() {
        assert!(is_project_dir(&["src", "package.json"]));
        assert!(is_project_dir(&[".git"]));
        assert!(!is_project_dir(&["src", "README.md"]));
    }

    #[test]
    fn ignores_heavy_and_hidden_dirs() {
        assert!(is_ignored("node_modules"));
        assert!(is_ignored(".cache"));
        assert!(!is_ignored("parsers"));
    }

    #[test]
    fn finds_project_with_nested_subprojects() {
        let root = tmp("nested");
        let app = root.join("hrh");
        fs::create_dir_all(app.join("parsers/news")).unwrap();
        fs::create_dir_all(app.join("dashboard")).unwrap();
        fs::write(app.join("package.json"), "{}").unwrap();
        fs::write(app.join("parsers/news/orchestrator.json"), "{}").unwrap();
        fs::write(app.join("dashboard/package.json"), "{}").unwrap();

        let projects = scan(&root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "hrh");
        let child_names: Vec<&str> = projects[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(child_names, vec!["dashboard", "news"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn skips_node_modules_and_hidden() {
        let root = tmp("skip");
        fs::create_dir_all(root.join("app/node_modules/dep")).unwrap();
        fs::write(root.join("app/node_modules/dep/package.json"), "{}").unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/package.json"), "{}").unwrap();
        fs::write(root.join("app/package.json"), "{}").unwrap();

        let projects = scan(&root).unwrap();
        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["app"]);
        assert!(projects[0].children.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }
}
