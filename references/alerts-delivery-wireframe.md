# Wireframe: политика доставки алертов (#21)

Поверх `AlertCandidate` из #20. Без реальных push (#22) и `say` (#23).

```
┌─ AlertCandidate ──────────────────┐
│  rule_id, severity, event, msg    │
└────────────────┬──────────────────┘
                 │ DeliveryGate::decide(now)
                 ▼
┌─ дедуп (10 мин) ──────────────────┐
│  key = kind + project|port|run_id │
│  повтор в окне → SUPPRESSED       │
└────────────────┬──────────────────┘
                 │ emit
                 ▼
┌─ тихие часы (23:00–08:00 Berlin) ─┐
│  quiet → speak=false              │
│  quiet → notify только critical   │
└────────────────┬──────────────────┘
                 ▼
┌─ троттлинг голоса (1/мин) ────────┐
│  слишком рано → speak=false       │
└────────────────┬──────────────────┘
                 ▼
┌─ DeliveryDecision ────────────────┐
│  emit / speak / notify            │
│  (#22 читает notify, #23 — speak) │
└───────────────────────────────────┘
```
