# Пример: пульс parser (#97)

Минимальный проект `type: parser` с опросом прогресса.

## Команда (как в `orchestrator.json`)

```sh
cd examples/parser
bun run pulse
```

Stdout:

```json
{"summary":"1500 новых лемм, 450 проверено","metrics":{"parsed":1500,"checked":450},"eta_seconds":600,"state":"running"}
```

## Файл

Тот же снимок лежит в [`pulse.json`](pulse.json). Чтобы опрашивать файл вместо команды:

```json
"pulse": { "file": "pulse.json", "interval_secs": 15 }
```

Документация контракта: [docs/pulse.md](../../docs/pulse.md).
