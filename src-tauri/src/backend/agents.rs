//! Запуски агентов: статусы run и менеджер git worktree (#27–#31).
//!
//! На каждый баг — отдельный worktree в `~/.orchestrator/worktrees/<project>/<run-id>`
//! с веткой `agent/<run-id>`, чтобы правки агента не попадали в основную ветку.
//! Override корня: `DEMERZEL_WORKTREES_DIR`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Статусы прогона агента (#27–#31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    NeedsReview,
    Failed,
    Discarded,
    Accepted,
}

/// Допустимые переходы. Accepted только после NeedsReview: слияние всегда вручную.
pub fn can_transition(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;
    matches!(
        (from, to),
        (Queued, Running)
            | (Running, NeedsReview)
            | (Running, Failed)
            | (NeedsReview, Accepted)
            | (NeedsReview, Discarded)
            | (Failed, Discarded)
            | (Queued, Discarded)
    )
}

/// Лимит параллельных агентов по умолчанию.
pub const DEFAULT_CONCURRENCY: usize = 1;

/// Созданный worktree: путь, ветка и идентификаторы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeHandle {
    pub project: String,
    pub run_id: String,
    pub path: PathBuf,
    pub branch: String,
}

/// Имя ветки агента: `agent/<run-id>`.
pub fn branch_name(run_id: &str) -> Result<String, String> {
    validate_segment("run_id", run_id)?;
    Ok(format!("agent/{run_id}"))
}

/// Корень каталога worktree (`~/.orchestrator/worktrees` или `DEMERZEL_WORKTREES_DIR`).
pub fn worktrees_root() -> Result<PathBuf, String> {
    if let Some(override_dir) = std::env::var_os("DEMERZEL_WORKTREES_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".orchestrator/worktrees"))
}

/// Путь worktree: `<root>/<project>/<run-id>`.
pub fn worktree_path(project: &str, run_id: &str) -> Result<PathBuf, String> {
    validate_segment("project", project)?;
    validate_segment("run_id", run_id)?;
    Ok(worktrees_root()?.join(project).join(run_id))
}

fn validate_segment(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return Err(format!("{label} must be a single path segment without '..'"));
    }
    if value.starts_with('.') {
        return Err(format!("{label} must not start with '.'"));
    }
    Ok(())
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().unwrap_or(&"")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "git {} failed: {}{}",
            args.join(" "),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" ({})", stdout.trim())
            }
        ));
    }
    Ok(output)
}

/// Создаёт ветку `agent/<run-id>` и worktree в `~/.orchestrator/worktrees/...`.
///
/// `base_ref` — от чего ветвиться (по умолчанию `HEAD` репозитория проекта).
pub fn create_worktree(
    repo: &Path,
    project: &str,
    run_id: &str,
    base_ref: Option<&str>,
) -> Result<WorktreeHandle, String> {
    let path = worktree_path(project, run_id)?;
    let branch = branch_name(run_id)?;
    let base = base_ref.unwrap_or("HEAD");

    if path.exists() {
        return Err(format!("worktree path already exists: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Новая ветка + каталог: правки только в worktree, основная ветка не трогается.
    run_git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            path.to_str().ok_or("worktree path is not UTF-8")?,
            base,
        ],
    )?;

    Ok(WorktreeHandle {
        project: project.to_string(),
        run_id: run_id.to_string(),
        path,
        branch,
    })
}

/// Удаляет worktree и ветку `agent/<run-id>` после завершения или отмены.
pub fn remove_worktree(repo: &Path, project: &str, run_id: &str) -> Result<(), String> {
    let path = worktree_path(project, run_id)?;
    let branch = branch_name(run_id)?;

    if path.exists() {
        run_git(
            repo,
            &[
                "worktree",
                "remove",
                "--force",
                path.to_str().ok_or("worktree path is not UTF-8")?,
            ],
        )?;
    } else {
        // Каталог уже снесён — подчистим запись worktree, если осталась.
        let _ = run_git(repo, &["worktree", "prune"]);
    }

    // Ветка агента больше не нужна; игнор, если уже удалена.
    let delete = Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git branch -D: {e}"))?;
    if !delete.status.success() {
        let stderr = String::from_utf8_lossy(&delete.stderr);
        if !stderr.contains("not found") && !stderr.contains("doesn't exist") {
            return Err(format!("git branch -D {branch} failed: {}", stderr.trim()));
        }
    }

    // Пустой каталог проекта под worktrees — подчистить, если остался.
    if let Some(project_dir) = path.parent() {
        let _ = std::fs::remove_dir(project_dir);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use RunStatus::*;
    use std::fs;
    use std::sync::Mutex;

    /// Сериализация тестов с `DEMERZEL_WORKTREES_DIR` / env.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn cannot_accept_without_review_inner() {
        assert!(!can_transition(Running, Accepted));
        assert!(can_transition(NeedsReview, Accepted));
    }

    #[test]
    fn cannot_accept_without_review() {
        cannot_accept_without_review_inner();
    }

    #[test]
    fn default_concurrency_is_one() {
        assert_eq!(DEFAULT_CONCURRENCY, 1);
    }

    #[test]
    fn branch_name_format() {
        assert_eq!(branch_name("run-42").unwrap(), "agent/run-42");
        assert!(branch_name("../x").is_err());
        assert!(branch_name("a/b").is_err());
        assert!(branch_name("").is_err());
    }

    #[test]
    fn worktree_path_under_override_root() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("demerzel-wt-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &root) };
        let path = worktree_path("demo", "r1").unwrap();
        assert_eq!(path, root.join("demo").join("r1"));
        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&root);
    }

    fn init_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        run_git(dir, &["init"]).unwrap();
        run_git(dir, &["config", "user.email", "test@demerzel.local"]).unwrap();
        run_git(dir, &["config", "user.name", "Demerzel Test"]).unwrap();
        // Без default branch name на старых git.
        let _ = run_git(dir, &["checkout", "-b", "main"]);
        fs::write(dir.join("README.md"), "main\n").unwrap();
        run_git(dir, &["add", "README.md"]).unwrap();
        run_git(dir, &["commit", "-m", "init"]).unwrap();
    }

    fn git_stdout(repo: &Path, args: &[&str]) -> String {
        let out = run_git(repo, args).unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn create_and_remove_worktree_isolates_main() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        let main_before = git_stdout(&repo, &["rev-parse", "HEAD"]);
        let status_before = git_stdout(&repo, &["status", "--porcelain"]);
        assert!(status_before.is_empty(), "repo must be clean before: {status_before}");

        let handle = create_worktree(&repo, "my-app", "run-100", None).unwrap();
        assert!(handle.path.is_dir());
        assert_eq!(handle.branch, "agent/run-100");
        assert_eq!(handle.path, wt_root.join("my-app").join("run-100"));

        // Правка только в worktree.
        fs::write(handle.path.join("agent-change.txt"), "from agent\n").unwrap();
        run_git(&handle.path, &["add", "agent-change.txt"]).unwrap();
        run_git(&handle.path, &["commit", "-m", "agent wip"]).unwrap();

        // Основной каталог и ветка main не изменились.
        let status_main = git_stdout(&repo, &["status", "--porcelain"]);
        assert!(
            status_main.is_empty(),
            "main worktree must stay clean, got: {status_main}"
        );
        assert_eq!(git_stdout(&repo, &["rev-parse", "HEAD"]), main_before);
        assert_eq!(
            git_stdout(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "main"
        );
        assert!(
            !repo.join("agent-change.txt").exists(),
            "agent file must not appear in main worktree"
        );
        // На ветке агента коммит есть.
        assert_ne!(
            git_stdout(&repo, &["rev-parse", "agent/run-100"]),
            main_before
        );

        remove_worktree(&repo, "my-app", "run-100").unwrap();
        assert!(!handle.path.exists());
        let branches = git_stdout(&repo, &["branch", "--list", "agent/run-100"]);
        assert!(branches.is_empty(), "agent branch must be deleted: {branches}");
        assert_eq!(git_stdout(&repo, &["rev-parse", "HEAD"]), main_before);

        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn remove_is_idempotent_when_path_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-rm-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-rm-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        create_worktree(&repo, "p", "r2", Some("HEAD")).unwrap();
        remove_worktree(&repo, "p", "r2").unwrap();
        // Повторная очистка после отмены не должна падать.
        remove_worktree(&repo, "p", "r2").unwrap();

        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn rejects_duplicate_worktree_path() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-dup-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-dup-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        create_worktree(&repo, "p", "same", None).unwrap();
        let err = create_worktree(&repo, "p", "same", None).unwrap_err();
        assert!(err.contains("already exists"), "{err}");

        remove_worktree(&repo, "p", "same").unwrap();
        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }
}
