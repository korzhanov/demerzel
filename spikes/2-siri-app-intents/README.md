# Спайк 2: App Intents / Siri Shortcuts для Demerzel

Цель: подтвердить на Mac, что App Shortcut «Спроси Demerzel …» (App Intents, macOS 13+) может принимать сущность «проект», отвечать диалогом + сниппетом и доставать данные из Demerzel. Результат решает, идём ли в #101 (интенты статуса и перезапуска).

Требуется: macOS 13+, Xcode, Bun, собранный `dem` / живой supervisor (`~/.orchestrator/runtime/dem.sock`).

## Вопросы спайка

| Вопрос | Ответ (кратко) |
|---|---|
| Русская фраза Siri | Да: `LocalizedStringResource` / `AppShortcutsProvider` с русской локалью; фраза обязана содержать `\(.applicationName)` («Demerzel»). |
| Имя приложения в фразе | Обязательно (требование App Shortcuts). Пример: «Спроси \(.applicationName) про \(\.$project)». |
| Связь intent ↔ Tauri | **Unix-сокет `dem.sock`** (тот же протокол, что CLI #102). App Group — опциональный кэш списка проектов; CLI subprocess — запасной, но хуже. |
| Sandbox | Локальный ad-hoc / без App Sandbox: сокет свободен. MAS / sandbox: нужен entitlement на Unix-сокет или out-of-sandbox helper; App Group сам по себе live-команды не даёт. |

## Шаги

1. Поднять локальный API (если сокета нет):
   ```sh
   cd src-tauri && cargo build --bin dem
   ./target/debug/dem pulse parser --json ../examples/parser
   ```

2. Проба моста Swift → `dem.sock` (без Siri, доказывает IPC + текст диалога):
   ```sh
   cd spikes/2-siri-app-intents
   swift scripts/dem-sock-probe.swift ../../examples/parser parser
   ```
   Ожидание: `DIALOG: parser: …, ещё N мин`.

3. Эталон App Intent (исходники в `Sources/`):
   - `AskDemerzelIntent` — параметр `ProjectEntity`, `perform` → сокет `pulse`, `IntentDialog` + snippet.
   - `DemerzelAppShortcuts` — русские фразы с `\(.applicationName)`.
   - Встраивание в основной `.app` — **сделано в #101**: `src-tauri/macos-siri/` → `PlugIns/DemerzelAppIntents.appex`.

4. Ручная проверка на Mac (после `tauri build` / install):
   - Shortcuts / Spotlight: «Спроси Demerzel про parser».
   - Siri: та же фраза; «Перезапусти Demerzel parser» — с подтверждением.
   - Ответ: IntentDialog с summary / ETA (или `project: restarted`).

## Критерии

- **go:** мост Swift→`dem.sock` возвращает пульс; App Intents API покрывает диалог + entity + App Shortcut с русскими фразами и `\(.applicationName)`; путь встраивания в Tauri ясен (Swift в бандле host). Sandbox-оговорки задокументированы.
- **no-go:** нет стабильного IPC без сети; нельзя локализовать фразу / имя приложения; App Intents нельзя связать с desktop Tauri-бандлом даже вне sandbox.

## Результат

- ADR: [`docs/decisions/003-siri-app-intents.md`](../../docs/decisions/003-siri-app-intents.md) — **go**
- Отчёт: [`docs/decisions/003-report.md`](../../docs/decisions/003-report.md)
- Гайд: [`docs/siri-app-intents.md`](../../docs/siri-app-intents.md)
- Контракт фраз/JSON (тесты): `src/lib/siri-bridge.ts`
