# Wireframe: голосовые алерты `say` (#23)

Поверх `DeliveryDecision.speak` из #21. Notify — #22.

```
┌─ AlertEvent ──────────────────────┐
│  process_exited / port / agent…   │
└────────────────┬──────────────────┘
                 │ evaluate(rules)
                 ▼
┌─ AlertCandidate ──────────────────┐
│  severity, event, message         │
└────────────────┬──────────────────┘
                 │ DeliveryGate::decide
                 ▼
┌─ DeliveryDecision ────────────────┐
│  speak == false → stop            │
│  (только critical; quiet → off;   │
│   throttle ≤1/min)                │
└────────────────┬──────────────────┘
                 │ maybe_speak + VoiceConfig
                 ▼
┌─ SpeakPayload ────────────────────┐
│  voice = Milena (config)          │
│  phrase = шаблон + {project}…     │
└────────────────┬──────────────────┘
                 │ VoiceSpeaker / SaySpeaker
                 ▼
┌─ say -v Milena «…» ───────────────┐
│  spawn, без блокировки UI         │
└───────────────────────────────────┘
```

Шаблоны по умолчанию (короткие, без повторов):

| Событие | Фраза |
|---------|--------|
| `process_exited` | `Олег, внимание. {project} упал, код {code}.` |
| `port_conflict` | `Олег, внимание. Порт {port} занят.` |
| `agent_failed` | `Олег, внимание. Агент {run_id} не справился.` |
| `tests_failed` | `Олег, внимание. Тесты {run_id} упали.` |

Команда: `ingest_alert_event` → `{ spoken }`. Трейт `VoiceSpeaker` мокается в cargo-тестах.
