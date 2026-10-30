# Отчёт спайка #100: App Intents / Siri

- **Ветка:** `feat/siri-app-intents-spike`
- **ADR:** [003-siri-app-intents.md](./003-siri-app-intents.md) — вердикт **go**
- **Код спайка:** `spikes/2-siri-app-intents/` (изолированные Swift Sources + probe; основной Demerzel не ломаем)
- **Контракт (тесты):** `src/lib/siri-bridge.ts`

## Вердикт

**Go.** App Intents на macOS пригодны для Demerzel: фразы с именем приложения (в т.ч. русские), entity «проект», dialog + snippet. Live-данные — через уже существующий Unix-сокет `dem.sock` (#102). Встраивание в Tauri — Swift в бандле host (как widget `.appex`). Переходим к #101.

## Доказательства

### A. IPC Swift → dem.sock

```sh
cd src-tauri && cargo build --bin dem
./target/debug/dem pulse parser --json ../examples/parser
# → JSON summary + eta_seconds

cd ../spikes/2-siri-app-intents
swift scripts/dem-sock-probe.swift ../../examples/parser parser
# → DIALOG: parser: 1500 новых лемм, 450 проверено, ещё 10 мин
```

На машине спайка (2026-10-31): оба шага OK при живом supervisor.

### B. App Intents (эталон)

| Файл | Роль |
|---|---|
| `Sources/DemSockClient.swift` | Unix-сокет клиент + formatEtaRu |
| `Sources/ProjectEntity.swift` | `AppEntity` + query (App Group / fallback) |
| `Sources/AskDemerzelIntent.swift` | Intent + dialog + snippet view |
| `Sources/DemerzelAppShortcuts.swift` | Phrases с `\(.applicationName)` |
| `Resources/phrases.ru.md` | Таблица русских фраз |

Полный Siri E2E требует embed в подписанный `.app` (#101). API и фразы проверены по документации Apple (WWDC22 App Shortcuts: имя приложения обязательно; один параметр; dialog/snippet).

### C. Sandbox

| Режим | dem.sock | App Group | Вывод |
|---|---|---|---|
| Локальный ad-hoc, без App Sandbox | OK | опционально | Primary path |
| App Sandbox / MAS | нужен entitlement / helper | OK для файлов | Не блокер для #101 локально |

## Дерево

```
spikes/2-siri-app-intents/
  README.md
  Resources/phrases.ru.md
  Sources/
    AskDemerzelIntent.swift
    DemSockClient.swift
    DemerzelAppShortcuts.swift
    ProjectEntity.swift
  scripts/dem-sock-probe.swift
docs/decisions/
  003-siri-app-intents.md
  003-report.md
docs/siri-app-intents.md
src/lib/siri-bridge.ts
src/lib/siri-bridge.test.ts
```

## Следующие шаги

1. **#101** — `AskDemerzelStatus` + `RestartProcess` в продукте, опираясь на этот эталон и `dem.sock`.
2. Не смешивать с #95 / #96 (агенты) в том же PR.
