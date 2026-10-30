//! Поиск проектов и подпроектов (#6).

/// Проект или подпроект, найденный в корневой папке.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub children: Vec<Project>,
}

/// Маркеры, по которым папка считается проектом.
pub const PROJECT_MARKERS: &[&str] = &["package.json", ".git", "orchestrator.json"];

/// Папки, которые никогда не сканируются.
pub const IGNORED_DIRS: &[&str] = &["node_modules", "target", ".git", ".svelte-kit", "build", "dist"];

pub fn is_ignored(dir_name: &str) -> bool {
    IGNORED_DIRS.contains(&dir_name) || dir_name.starts_with('.')
}

pub fn is_project_dir(entries: &[&str]) -> bool {
    entries.iter().any(|e| PROJECT_MARKERS.contains(e))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
