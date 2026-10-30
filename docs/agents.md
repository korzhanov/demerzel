# Агенты: git worktree, Claude CLI, лимиты и ревью

Phase-6, issues #27 (worktree), #28 (headless Claude Code CLI), #29 (concurrency + память), #31 (тесты + diff), #32 (UI ревью), #33 (запуск из задачи). Правки агента живут в отдельном worktree и ветке, чтобы основная рабочая копия проекта оставалась чистой.

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

## Параллелизм и память (#29)

Перед переводом run из `Queued` → `Running` вызывается `AgentLimiter::try_acquire`:

1. **Concurrency** — одновременно не больше `agents.max_concurrent` (по умолчанию **1**). Иначе `StartDecision::Queue(ConcurrencyLimit)`.
2. **Память** — `vm_stat`: доступно ≈ `(Pages free + inactive + speculative) × page size`. Если меньше `agents.min_free_memory_mb` (по умолчанию **2048** МиБ) — очередь `InsufficientMemory`.
3. **Ollama** — если процесс `ollama` уже запущен (`pgrep -x`), к порогу добавляется `agents.ollama_reserve_mb` (по умолчанию **4096** МиБ). Сам раннер Ollama — #30.

Слот освобождается `release(run_id)` после завершения / fail / discard. Повторный `try_acquire` того же `run_id` идемпотентен.

Конфиг в `settings.json` → `agents`:

```json
{
  "agents": {
    "max_concurrent": 1,
    "min_free_memory_mb": 2048,
    "ollama_reserve_mb": 4096
  }
}
```

Отсутствующая секция → значения по умолчанию.

## Тесты и diff после агента (#31)

После headless-прогона (ещё в worktree, статус `running`):

1. **Тесты** — `commands.test` из `orchestrator.json` (`resolve_test_command`), `sh -c` в каталоге worktree. Нет поля / пусто → skip (`TEST_SKIPPED`), без spawn.
2. **Результат** — `TestRunResult`: команда, `exit_code`, объединённый лог (stdout+stderr), `skipped`, `duration_ms`.
3. **Diff** — `capture_diff(worktree, base_ref)` → `git diff --no-ext-diff <base>` (коммиты агента + незакоммиченное относительно базы).
4. **Статус** — `status_after_tests`: skip или exit 0 → `needs_review`; иначе → `failed`. Accepted только вручную из `needs_review` (#32 UI).

Сборка пакета: `collect_review(worktree, test_command, base_ref)` → `AgentReviewBundle { tests, diff, base_ref, status }`.

Статусы run (snake_case для SQLite / UI):

| Статус | Значение |
|--------|----------|
| `queued` | ждёт слот / старт |
| `running` | агент или пост-обработка |
| `needs_review` | тесты ок / skip — ждёт человека |
| `failed` | тесты упали (или агент) |
| `discarded` | отброшено |
| `accepted` | принято (merge вручную) |

Переходы: `can_transition` / `RunStatus::as_str` / `parse`.

## API (Rust)

Модуль `backend/agents`:

- `create_worktree(repo, project, run_id, base_ref?)` → `WorktreeHandle`
- `remove_worktree(repo, project, run_id)`
- `worktree_path` / `branch_name` / `worktrees_root`
- `build_prompt(task, worktree)` → `String`
- `ClaudeHandle::spawn` / `cancel` / `wait_until`
- `run_claude(worktree, task, timeout, cancel?)` → `ClaudeRunResult`
- `claude_bin` / `DEFAULT_CLAUDE_TIMEOUT` / `PROJECT_RULES_FILE`
- `AgentsConfig` / `AgentLimiter` / `try_acquire` / `release`
- `available_memory_mb` / `parse_vm_stat_available_mb` / `ollama_process_running`
- `DEFAULT_CONCURRENCY` / `DEFAULT_MIN_FREE_MEMORY_MB` / `DEFAULT_OLLAMA_RESERVE_MB`
- `RunStatus` / `can_transition` / `status_after_tests` / `as_str` / `parse`
- `run_project_tests` / `capture_diff` / `collect_review`
- `TestRunResult` / `AgentReviewBundle` / `TEST_SKIPPED`
- `config::resolve_test_command`
- `AgentRunRecord` / `AgentRunRegistry` / `AcceptAgentRunResult`
- `manual_merge_command` / `shell_single_quote`
- `upsert` / `list` / `accept` / `discard` (discard → `remove_worktree`)
- `prepare_agent_from_task` / `complete_agent_run` / `start_agent_from_task` (#33)
- `StartAgentRunResult` / `project_slug` / `task_prompt_text`

## UI ревью (#32)

Панель «Ревью агентов» в главном окне:

1. **Список** — `list_agent_runs` (in-memory `AgentRunRegistry` до полной SQLite-связки).
2. **Детали** — diff и лог тестов из записи (`get_agent_run`).
3. **Принять** — только из `needs_review`: статус → `accepted`, UI показывает `manual_merge_command` (`git -C '<repo>' merge --no-ff agent/<run-id>`). Сам merge не выполняется.
4. **Отклонить** — из `needs_review` / `failed` / `queued`: `remove_worktree` + статус `discarded`.

Tauri: `list_agent_runs`, `get_agent_run`, `upsert_agent_run`, `accept_agent_run`, `discard_agent_run`.

## Запуск из задачи (#33)

Кнопка «Исправить» на карточке доски задач:

1. **Статус задачи** — `mark_task_in_progress`: TODO.md → `- [~]`, `tasks.json` → `in_progress`, GitHub — только локально в ответе (issue не закрывается).
2. **Prepare** — `prepare_agent_from_task`: запись `queued`, `AgentLimiter::try_acquire`; при слоте — `create_worktree`, статус `running`.
3. **Complete** (фон в UI / sync в тестах) — `complete_agent_run`: Claude CLI → `collect_review` → `needs_review` / `failed`, `release` слота.
4. **Очередь** — при нехватке слота/памяти run остаётся `queued` без worktree; задача уже `in_progress`.
5. **Закрытие задачи** — после accept/discard только вручную (accept не трогает TODO/GitHub).

Tauri: `start_agent_from_task(task)` → `StartAgentRunResult { run, task, queued, queue_reason? }`.

## API (дополнение #33)

- `mark_task_in_progress` / `set_todo_checkbox` / `set_tasks_json_status` (`backend/tasks`)
- `project_slug` / `new_run_id` / `task_prompt_text` / `queue_reason_message`
- `prepare_agent_from_task` / `complete_agent_run` / `start_agent_from_task`
- `StartAgentRunResult` / `StartAgentParams`

## Связь со схемой

В SQLite `agent_runs.worktree` хранит путь к каталогу run (см. миграцию `0001_init.sql`); `status` — snake_case из `RunStatus`.
