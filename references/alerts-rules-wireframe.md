# Wireframe: движок правил алертов (#20)

До кода delivery (#21–#23). Только контракт правил и оценка событий.

```
┌─ settings.json (приложение) ──────────────────────────┐
│  scan_roots: [...]                                    │
│  alerts.rules: [                                      │
│    { id, kind, when, severity, enabled }              │
│  ]                                                    │
└────────────────────────┬──────────────────────────────┘
                         │
                         ▼
┌─ runtime event ────────┴──────────┐
│  process_exited / port_conflict   │
│  agent_failed / tests_failed      │
└────────────────┬──────────────────┘
                 │ evaluate(rules, event)
                 ▼
┌─ AlertCandidate ──────────────────┐
│  rule_id, severity, message       │
│  (без push / say / dedupe)        │
└───────────────────────────────────┘
```

Severity → подсказки доставки (реализация позже):

- critical → UI + notify + speak
- important → UI + notify
- info → UI
