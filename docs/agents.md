# Агенты: git worktree и Claude CLI

Phase-6, issues #27 (worktree) и #28 (headless Claude Code CLI). Правки агента живут в отдельном worktree и ветке, чтобы основная рабочая копия проекта оставалась чистой.

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

## Claude Code CLI (#28)

Headless-прогон внутри каталога worktree:

1. **Промпт** — `build_prompt(task, worktree)`: секция `# Task` из описания задачи; если в корне worktree есть `CLAUDE.md`, перед ней добавляется `# Project rules`.
2. **Запуск** — `claude -p "<prompt>"` (`cwd` = worktree), отдельная process group на Unix.
3. **Сбор** — stdout, stderr, код выхода, длительность (`duration_ms`).
4. **Таймаут** — по умолчанию 30 мин (`DEFAULT_CLAUDE_TIMEOUT`); по истечении SIGTERM дереву, затем SIGKILL.
5. **Отмена** — флаг `AtomicBool` (кнопка UI позже) → тот же SIGTERM/SIGKILL.

Override бинарника для тестов: `DEMERZEL_CLAUDE_BIN`.

## API (Rust)

Модуль `backend/agents`:

- `create_worktree(repo, project, run_id, base_ref?)` → `WorktreeHandle`
- `remove_worktree(repo, project, run_id)`
- `worktree_path` / `branch_name` / `worktrees_root`
- `build_prompt(task, worktree)` → `String`
- `ClaudeHandle::spawn` / `cancel` / `wait_until`
- `run_claude(worktree, task, timeout, cancel?)` → `ClaudeRunResult`
- `claude_bin` / `DEFAULT_CLAUDE_TIMEOUT` / `PROJECT_RULES_FILE`

Лимиты параллелизма (#29), тесты/diff (#31) и UI (#32–#33) поверх этого слоя.

## Связь со схемой

В SQLite `agent_runs.worktree` хранит путь к каталогу run (см. миграцию `0001_init.sql`).
