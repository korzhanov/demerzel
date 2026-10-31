# App Intents / Siri (macOS)

Спайк #100 → ADR [003-siri-app-intents.md](decisions/003-siri-app-intents.md) (**go**).  
Production-интенты — #101: `AskDemerzelStatus` + `RestartProcess` в `DemerzelAppIntents.appex`.

## Зачем

Голосовой / Spotlight-доступ к пульсу и перезапуску проекта без открытия окна Demerzel. Zero-setup App Shortcuts: фразы доступны сразу после установки приложения.

## Архитектура

```
Siri / Shortcuts / Spotlight
        │  App Intents (Swift, PlugIns/DemerzelAppIntents.appex)
        ▼
 AskDemerzelStatus / RestartProcess
        │  Unix socket (JSON line)
        ▼
 ~/.orchestrator/runtime/dem.sock   ← тот же API, что CLI (#102)
        │
        ▼
 PulseStore / process restart
```

Корень экосистемы: App Group `siri-config.json` (`ecosystemRoot` ← первый `scan_roots` при старте/сохранении настроек), иначе `DEMERZEL_ROOT` / UserDefaults.

Опционально: `EntityQuery` читает имена проектов из App Group `summary.json` (`group.app.demerzel.orchestrator`).

## Фразы

Каждая phrase **должна** содержать `\(.applicationName)` (имя бандла → Demerzel).

Примеры (ru):

- «Спроси Demerzel про parser»
- «Спроси Demerzel о web»
- «Статус Demerzel api»
- «Перезапусти Demerzel parser» (с подтверждением)

Контракт фраз и JSON запроса — `src/lib/siri-bridge.ts` (тесты `bun test src/lib/siri-bridge.test.ts`).

## Транспорт

Протокол сокета — [cli.md](cli.md#supervisor--локальный-api):

```json
{ "action": "pulse", "root": "/path/to/ecosystem", "project": "parser", "json": true }
```

```json
{ "action": "restart", "root": "/path/to/ecosystem", "project": "parser", "json": true }
```

Ответ supervisor: `{ "output": "<json или текст>", "error": null }`.  
Для диалога intent парсит `output` как pulse/restart JSON.

## Сборка

```sh
# вместе с tauri bundle (beforeBundleCommand):
./src-tauri/macos-siri/build-siri.sh
# → src-tauri/macos-siri/build/Build/Products/Release/DemerzelAppIntents.appex
# tauri.conf.json → PlugIns/DemerzelAppIntents.appex
```

Требуется: Xcode, `xcodegen`, macOS 15+ (App Intents Extension + `requestConfirmation`).

## Sandbox

- Локальные сборки Demerzel без App Sandbox: сокет работает из Swift в том же user session.
- App Store / sandbox: заложить entitlement на Unix domain socket или helper вне sandbox; App Group не заменяет live `restart`.

## Эталон спайка

Production-код: `src-tauri/macos-siri/`. Исходный спайк #100 удалён, см. историю git.
