# Агенты: git worktree, Claude CLI, Ollama, реестр CLI, лимиты, ревью и автофикс

Phase-6/7, issues #27 (worktree), #28 (headless Claude Code CLI), #29 (concurrency + память), #30 (Ollama, опционально), #31 (тесты + diff), #32 (UI ревью), #33 (запуск из задачи), #95 (реестр CLI на машине), #96 (автофикс по алерту). Правки агента живут в отдельном worktree и ветке, чтобы основная рабочая копия проекта оставалась чистой.

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
3. **Ollama (память)** — если процесс `ollama` уже запущен (`pgrep -x`), к порогу добавляется `agents.ollama_reserve_mb` (по умолчанию **4096** МиБ).

Слот освобождается `release(run_id)` после завершения / fail / discard. Повторный `try_acquire` того же `run_id` идемпотентен.

Конфиг в `settings.json` → `agents`:

```json
{
  "agents": {
    "max_concurrent": 1,
    "min_free_memory_mb": 2048,
    "ollama_reserve_mb": 4096,
    "runner": "claude",
    "ollama_model": "qwen2.5-coder:7b",
    "autofix": "off"
  }
}
```

Отсутствующая секция → значения по умолчанию. `runner: "claude"` — основной путь; Ollama не обязателен. `autofix` по умолчанию `off` (#96).

Раннер и автофикс на проект (#95 / #96) — в `orchestrator.json`:

```json
{
  "name": "api",
  "type": "service",
  "agents": { "runner": "ollama", "autofix": "suggest" }
}
```

Приоритет раннера: явный выбор в UI → `orchestrator.json` → `agents.runner` → `claude`.  
Приоритет автофикса: `orchestrator.json` → `agents.autofix` → `off`.

## Реестр CLI (#95)

Известные агенты: `claude`, `codex`, `gemini`, `aider`, `opencode`, `cursor_agent` (бинарник `agent`, алиас `cursor-agent`), `hermes`, `grok` (install URL не сверен с docs.x.ai), `ollama`.

1. **Детект** — поиск в `PATH` (или `DEMERZEL_*_BIN`), затем лёгкий `--version` (не headless-сессия).
2. **Подсказка / установка (#140)** — таблица `config/agent-install.json`; кнопка «Установить» открывает **Terminal.app** (`osascript do script`) после подтверждения команды. Терминал по умолчанию — Terminal.app (выбор позже в настройках).
3. **Приоритет** — `agents.catalog` в settings (порядок + enabled); выключенный не вызывается; для задач/алертов — `resolve_preferred_agent` (первый enabled+установленный).
4. **Headless** — сейчас только Claude и Ollama; остальные видны в реестре/селекте как «только детект».
5. **API** — `list_available_agents`, `open_agent_install_terminal`, `resolve_preferred_agent`, `resolve_project_agent_runner`, `detect_agent` / `runner_readiness_error`.

Gemini: оставляем `gemini` CLI; free tier у Google с 2026-06-18 указывает на Antigravity CLI — отдельный runner позже.  
Grok: в таблице с `verified: false` до сверки docs.x.ai.

Тесты: фейковые исполняемые скрипты в изолированном `PATH`; `agent-install` merge/reorder/pick.

## Ollama (#30)

Опциональный headless-раннер для простых задач (не блокирует Claude CLI):

1. **Выбор** — `agents.runner` в settings или селект «Раннер» на доске задач перед «Исправить».
2. **Запуск** — `ollama run <model> "<prompt>"` (`cwd` = worktree), модель по умолчанию `qwen2.5-coder:7b`.
3. **Сбор** — stdout/stderr/код/длительность; таймаут и отмена как у Claude (SIGTERM process group).
4. **Дальше** — тот же `collect_review` (тесты + diff → `needs_review` / `failed`).
5. Override: `DEMERZEL_OLLAMA_BIN`, `DEMERZEL_OLLAMA_MODEL`.

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
- `AgentRunner` / `OllamaHandle` / `run_ollama` / `run_agent_cli` (#30 / #95)
- `ollama_bin` / `resolve_ollama_model` / `DEFAULT_OLLAMA_MODEL` / `DEFAULT_OLLAMA_TIMEOUT`
- `agent_catalog::{list_available_agents,detect_agent,install_hint,runner_readiness_error}` (#95)
- `resolve_runner_for_project` (#95)
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
3. **Complete** (фон в UI / sync в тестах) — `complete_agent_run`: выбранный раннер (Claude / Ollama) → `collect_review` → `needs_review` / `failed`, `release` слота.
4. **Очередь** — при нехватке слота/памяти run остаётся `queued` без worktree; задача уже `in_progress`.
5. **Закрытие задачи** — после accept/discard только вручную (accept не трогает TODO/GitHub).

Tauri: `start_agent_from_task(task)` → `StartAgentRunResult { run, task, queued, queue_reason? }`.

## API (дополнение #33)

- `mark_task_in_progress` / `set_todo_checkbox` / `set_tasks_json_status` (`backend/tasks`)
- `project_slug` / `new_run_id` / `task_prompt_text` / `queue_reason_message`
- `prepare_agent_from_task` / `complete_agent_run` / `start_agent_from_task`
- `StartAgentRunResult` / `StartAgentParams`

## Автофикс по алерту (#96)

Режимы `AutofixMode`: `off` / `suggest` / `auto` (`AgentsConfig.autofix`, override в `orchestrator.json`).

1. Алерт (`process_exited` ≠ 0 / `port_conflict` / `tests_failed`) → контекст: команда + хвост логов + diff.
2. `suggest` — событие `alerts:autofix-offer` + баннер «Исправить».
3. `auto` или клик — `prepare_agent_from_alert` → worktree + limiter → CLI → `collect_review` → `needs_review` / `failed`.
4. Accept по-прежнему только вручную (#32); merge в main не выполняется.

Модуль `backend/autofix`: `decide_autofix`, `build_alert_autofix_prompt`, `start_agent_from_alert`.  
Tauri: `start_agent_from_alert(project_path, prompt, runner?)` → `StartAgentFromAlertResult`.

## ACP — Agent Client Protocol (#177)

Любой агент, который говорит по [Agent Client Protocol](https://zed.dev/acp), запускается как раннер `acp`. Demerzel выступает клиентом: поднимает агента как подпроцесс в worktree задачи, отправляет промпт и собирает текст ответа.

Настройка в `settings.json` приложения:

```json
{
  "agents": {
    "runner": "acp",
    "acp_command": "gemini --acp"
  }
}
```

- `acp_command` выполняется через `sh -c` в каталоге worktree. Любой агент с ACP-режимом подходит: команда задаётся целиком.
- Протокол построчный (JSON-RPC 2.0 на stdio): `initialize` → `session/new` → `session/prompt`. Ответ собирается из `session/update` (`agent_message_chunk`).
- Разрешения: Demerzel принимает только `allow_once`. Остальные варианты отклоняются, ход отменяется.
- Запросы агента к файловой системе и терминалу клиент не поддерживает и отклоняет. Агент работает в worktree своими средствами.
- Таймаут и отмена останавливают процесс агента целиком (process group).
- Раннер `acp` не требует установки из реестра; если `acp_command` пуст, запуск завершается ошибкой с подсказкой.

Ограничения: текст ответа показывается по завершении хода, а не потоком; `session/load` и MCP-серверы для агента пока не используются (`mcpServers` передаётся пустым).

## Связь со схемой

В SQLite `agent_runs.worktree` хранит путь к каталогу run (см. миграцию `0001_init.sql`); `status` — snake_case из `RunStatus`.
