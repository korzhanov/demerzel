//! PATH для дочерних процессов (#133). Приложение из Finder не получает PATH
//! оболочки, поэтому `bun`, `node` и `claude` из ~/.bun/bin, /opt/homebrew/bin
//! не находятся и процесс падает с кодом 127.

use std::path::Path;

/// Каталоги, которые обычно содержат CLI-инструменты разработчика на macOS/Linux.
/// Относительные к HOME перечисляются через `home`.
pub fn tool_dirs(home: Option<&Path>) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    if let Some(h) = home {
        for rel in [".bun/bin", ".local/bin", ".cargo/bin", ".npm-global/bin"] {
            dirs.push(h.join(rel).to_string_lossy().into_owned());
        }
    }
    for abs in [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        dirs.push(abs.to_string());
    }
    dirs
}

/// Собирает PATH: сначала то, что уже есть, затем недостающие стандартные каталоги.
/// Порядок сохраняется, дубликаты пропускаются.
pub fn augmented_path(current: Option<&str>, home: Option<&Path>) -> String {
    let mut out: Vec<String> = Vec::new();
    if let Some(cur) = current {
        for p in cur.split(':').filter(|p| !p.is_empty()) {
            if !out.iter().any(|x| x == p) {
                out.push(p.to_string());
            }
        }
    }
    for d in tool_dirs(home) {
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn keeps_existing_entries_first() {
        let p = augmented_path(Some("/custom/bin:/usr/bin"), None);
        let parts: Vec<&str> = p.split(':').collect();
        assert_eq!(parts[0], "/custom/bin");
        assert_eq!(parts[1], "/usr/bin");
        assert_eq!(parts.iter().filter(|x| **x == "/usr/bin").count(), 1);
    }

    #[test]
    fn adds_home_tool_dirs_and_homebrew() {
        let home = PathBuf::from("/Users/oleg");
        let p = augmented_path(None, Some(&home));
        assert!(p.contains("/Users/oleg/.bun/bin"));
        assert!(p.contains("/opt/homebrew/bin"));
    }

    #[test]
    fn empty_current_path_still_gets_defaults() {
        let p = augmented_path(Some(""), None);
        assert!(p.contains("/usr/bin"));
        assert!(!p.starts_with(':'));
    }
}
