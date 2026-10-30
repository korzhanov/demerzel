# Агенты: git worktree

Phase-6, issue #27 (база для #28–#33). Правки агента живут в отдельном worktree и ветке, чтобы основная рабочая копия проекта оставалась чистой.

## Расположение

| Что | Значение |
|-----|----------|
| Корень | `~/.orchestrator/worktrees` |
| Override | `DEMERZEL_WORKTREES_DIR` |
| Каталог run | `<root>/<project>/<run-id>` |
| Ветка | `agent/<run-id>` |

`project` и `run_id` — один сегмент пути (без `/`, `..`, ведущей точки).

## Жизненный цикл (#27)

1. **Create** — `git worktree add -b agent/<run-id> <path> <base>` (`base` по умолчанию `HEAD` репозитория проекта).
2. Агент работает только внутри worktree; коммиты идут в `agent/<run-id>`.
3. **Remove** (успех, fail или отмена) — `git worktree remove --force`, затем `git branch -D agent/<run-id>`.

Проверка изоляции: в основном worktree `git status` чистый, файл агента не появляется на `main`.

## API (Rust)

Модуль `backend/agents`:

- `create_worktree(repo, project, run_id, base_ref?)` → `WorktreeHandle`
- `remove_worktree(repo, project, run_id)`
- `worktree_path` / `branch_name` / `worktrees_root`

Раннер CLI (#28), лимиты (#29), тесты/diff (#31) и UI (#32–#33) поверх этого слоя.

## Связь со схемой

В SQLite `agent_runs.worktree` хранит путь к каталогу run (см. миграцию `0001_init.sql`).
