# Wireframe: системные уведомления (#22)

Поверх `DeliveryDecision.notify` из #21. Без `say` (#23).

```
┌─ AlertEvent ──────────────────────┐
│  process_exited / port / agent…   │
└────────────────┬──────────────────┘
                 │ evaluate(rules)
                 ▼
┌─ AlertCandidate ──────────────────┐
│  severity, message, event         │
└────────────────┬──────────────────┘
                 │ DeliveryGate::decide
                 ▼
┌─ DeliveryDecision ────────────────┐
│  notify == false → stop           │
│  (quiet: только critical)         │
└────────────────┬──────────────────┘
                 │ maybe_notify
                 ▼
┌─ NotifyPayload ───────────────────┐
│  title (Demerzel · …)             │
│  body = короткий message           │
│  extra.project? (process_exited)  │
└────────────────┬──────────────────┘
                 │ tauri-plugin-notification
                 ▼
┌─ OS banner / Notification Center ─┐
│  macOS Focus фильтрует сам (ADR)  │
└────────────────┬──────────────────┘
                 │ click (actionId=tap)
                 ▼
┌─ focus main window ───────────────┐
│  emit alerts:open-project         │
│  UI: select process / highlight   │
└───────────────────────────────────┘
```

Команда: `ingest_alert_event` (Rust). ADR Focus: `docs/decisions/002-focus-mode.md`.
