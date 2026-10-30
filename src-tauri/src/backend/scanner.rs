//! Поиск проектов и подпроектов (#6, #85).
//!
//! Проект — папка с маркером (`package.json`, `.git`, `orchestrator.json`).
//! Подпроекты — вложенные папки с маркерами (не глубже одного уровня nesting).
//! Игнорируются `node_modules`, `target`, скрытые папки и другие служебные каталоги.

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
/// `orchestrator.json` — явная регистрация (нужен для подпроектов без package.json, напр. parser).
pub const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    "orchestrator.json",
    "Cargo.toml",
    "go.mod",
    "pyproject.toml",
    "requirements.txt",
    ".git",
];

/// Папки, которые никогда не сканируются (#85: junk / tooling / vendor trees).
pub const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    "vendor",
    "venv",
    "coverage",
    "__pycache__",
    "Pods",
    "DerivedData",
    "bower_components",
    "jspm_packages",
];

pub fn is_ignored(dir_name: &str) -> bool {
    IGNORED_DIRS.contains(&dir_name) || dir_name.starts_with('.')
}

pub fn is_project_dir(entries: &[&str]) -> bool {
    entries.iter().any(|e| PROJECT_MARKERS.contains(e))
}

/// Подсказка для пользователя, если сканирование упёрлось в права доступа.
/// На macOS папки в `Documents`, `Desktop`, `Downloads` закрыты до разрешения.
pub fn access_hint(err: &io::Error, path: &Path) -> Option<String> {
    if err.kind() != io::ErrorKind::PermissionDenied {
        return None;
    }
    Some(format!(
        "нет доступа к {}. На macOS: Системные настройки → Конфиденциальность и безопасность → Файлы и папки (или Полный доступ к диску), разрешите Demerzel. См. docs/permissions.md",
        path.display()
    ))
}

/// Ограничение глубины walk по файловой системе (#85: было 6 — слишком глубоко для «Обновить задачи»).
pub const MAX_DEPTH: usize = 3;

/// Сколько уровней подпроектов внутри уже найденного проекта (#85).
/// `1` = top-level проект + один уровень детей (dashboard / parsers/news), без глубоких nest package.json.
pub const MAX_NESTING: usize = 1;

/// Сканирует корневую папку и возвращает найденные проекты с подпроектами.
pub fn scan(root: &Path) -> io::Result<Vec<Project>> {
    scan_dir(root, 0, 0)
}

fn scan_dir(dir: &Path, depth: usize, nesting: usize) -> io::Result<Vec<Project>> {
    // depth — уровень относительно корня scan; на MAX_DEPTH детей уже не читаем.
    if depth >= MAX_DEPTH {
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
        if is_project_dir(&refs) {
            // Внутри найденного проекта ищем детей только пока nesting < MAX_NESTING.
            let children = if nesting < MAX_NESTING {
                scan_dir(&path, depth + 1, nesting + 1)?
            } else {
                Vec::new()
            };
            found.push(Project { name, path, children });
        } else {
            // Не-проект: продолжаем walk с тем же nesting (parsers/ → news).
            found.extend(scan_dir(&path, depth + 1, nesting)?);
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

    #[test]
    fn permission_error_gets_hint() {
        let err = io::Error::from(io::ErrorKind::PermissionDenied);
        let hint = access_hint(&err, Path::new("/Users/x/Documents/app")).unwrap();
        assert!(hint.contains("/Users/x/Documents/app"));
        assert!(hint.contains("docs/permissions.md"));
    }

    #[test]
    fn other_errors_get_no_hint() {
        let err = io::Error::from(io::ErrorKind::NotFound);
        assert!(access_hint(&err, Path::new("/nope")).is_none());
    }

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
        assert!(is_ignored("vendor"));
        assert!(is_ignored("coverage"));
        assert!(is_ignored("__pycache__"));
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

    #[test]
    fn skips_vendor_and_coverage_junk() {
        let root = tmp("junk");
        fs::create_dir_all(root.join("app/vendor/pkg")).unwrap();
        fs::write(root.join("app/vendor/pkg/package.json"), "{}").unwrap();
        fs::create_dir_all(root.join("app/coverage/tmp")).unwrap();
        fs::write(root.join("app/coverage/tmp/package.json"), "{}").unwrap();
        fs::write(root.join("app/package.json"), "{}").unwrap();

        let projects = scan(&root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "app");
        assert!(projects[0].children.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    /// Глубокие nested package.json (apps → packages → tools) не тянутся в дерево / refresh задач (#85).
    #[test]
    fn does_not_pull_deep_nested_package_json() {
        let root = tmp("deep");
        let app = root.join("app");
        fs::create_dir_all(app.join("apps/web/packages/ui/tools")).unwrap();
        fs::write(app.join("package.json"), "{}").unwrap();
        fs::write(app.join("apps/web/package.json"), "{}").unwrap();
        fs::write(app.join("apps/web/packages/ui/package.json"), "{}").unwrap();
        fs::write(app.join("apps/web/packages/ui/tools/package.json"), "{}").unwrap();

        let projects = scan(&root).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "app");
        // Один уровень nesting: web — да; ui/tools — нет.
        let child_names: Vec<&str> = projects[0].children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(child_names, vec!["web"]);
        assert!(projects[0].children[0].children.is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn respects_max_filesystem_depth() {
        let root = tmp("fsdepth");
        // root/a/b/c/d/package.json — d на 4-м уровне; при MAX_DEPTH=3 детей c не читаем.
        let deep = root.join("a/b/c/d");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("package.json"), "{}").unwrap();
        // Контроль: проект на 2-м уровне находится.
        fs::create_dir_all(root.join("x/y")).unwrap();
        fs::write(root.join("x/y/package.json"), "{}").unwrap();

        let projects = scan(&root).unwrap();
        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"y"), "expected shallow project, got {names:?}");
        assert!(!names.contains(&"d"), "deep project must be skipped, got {names:?}");
        fs::remove_dir_all(&root).unwrap();
    }
}
