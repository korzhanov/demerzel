# App Intents / Siri (macOS)

Спайк #100 → ADR [003-siri-app-intents.md](decisions/003-siri-app-intents.md) (**go**). Реализация интентов — #101.

## Зачем

Голосовой / Spotlight-доступ к пульсу и (после #101) перезапуску проекта без открытия окна Demerzel. Zero-setup App Shortcuts: фразы доступны сразу после установки приложения.

## Архитектура

```
Siri / Shortcuts / Spotlight
        │  App Intents (Swift)
        ▼
 AskDemerzelIntent / RestartProcess
        │  Unix socket (JSON line)
        ▼
 ~/.orchestrator/runtime/dem.sock   ← тот же API, что CLI (#102)
        │
        ▼
 PulseStore / process restart
```

Опционально: `EntityQuery` читает имена проектов из App Group `summary.json` (`group.app.demerzel.orchestrator`), если сокет ещё не поднят.

## Фразы

Каждая phrase **должна** содержать `\(.applicationName)` (имя бандла → Demerzel).

Примеры (ru):

- «Спроси Demerzel про parser»
- «Спроси Demerzel о web»
- «Статус Demerzel api»

Контракт фраз и JSON запроса — `src/lib/siri-bridge.ts` (тесты `bun test src/lib/siri-bridge.test.ts`).

## Транспорт

Протокол сокета — [cli.md](cli.md#supervisor--локальный-api):

```json
{ "action": "pulse", "root": "/path/to/ecosystem", "project": "parser", "json": true }
```

Ответ supervisor: `{ "output": "<json или текст>", "error": null }`.  
Для диалога intent парсит `output` как pulse JSON и собирает строку `name: summary, ещё N мин`.

## Sandbox

- Локальные сборки Demerzel без App Sandbox: сокет работает из Swift в том же user session.
- App Store / sandbox: заложить entitlement на Unix domain socket или helper вне sandbox; App Group не заменяет live `restart`.

## Эталон кода

`spikes/2-siri-app-intents/` — Sources + `swift scripts/dem-sock-probe.swift`. Не подключать в production build до #101.
