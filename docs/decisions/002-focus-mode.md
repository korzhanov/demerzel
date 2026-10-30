# ADR-002: Статус «Не беспокоить» / Focus Mode на macOS

- **Status:** accepted
- **Date:** 2026-10-30
- **Issues:** #22
- **Related:** `docs/alerts.md`, DeliveryGate (#21)

## Context

Системные уведомления Demerzel (#22) должны по возможности уважать режим Focus / «Не беспокоить» пользователя. Нужно выяснить, есть ли **публичный** API macOS для чтения этого статуса из обычного desktop-приложения (Tauri / Rust), и что делать, если нет.

## Findings

| API / механизм | Доступность для Demerzel | Вывод |
|---|---|---|
| `INFocusStatusCenter` (`Intents`) + `focusStatus.isFocused` | Только для приложений с capability **Communication Notifications** (мессенджеры / communication intents). Apple прямо ограничивает Focus Status communication-приложениями. | Не подходит: Demerzel — оркестратор локального стека, не communication app. |
| `UNNotificationSettings` / UserNotifications | Настраивает *наши* категории и авторизацию; **не** отдаёт глобальный Focus/DND пользователя. | Не читает Focus. |
| Приватные / недокументированные Preference / Notification Center internals | Нет стабильного публичного контракта; App Store / notarization risk. | Не используем. |
| Системный фильтр Focus при доставке banner/alert | macOS сам решает, показывать ли уведомление приложения в текущем Focus (по правилам пользователя для приложения). | Полагаемся на OS. |

Публичного API «сейчас включён Focus/DND?» для произвольного macOS-приложения **нет**. Единственный документированный путь — Focus Status для communication apps, что к Demerzel не применимо.

## Decision

1. **Не** запрашиваем и **не** читаем статус Focus / DND в приложении.
2. Уважаем тишину через уже существующую политику **DeliveryGate** (#21): тихие часы → `notify` только для `critical`; дедуп и троттлинг голоса без изменений. Голос `say` (#23) — отдельно.
3. Показ идёт через `tauri-plugin-notification` → UserNotifications / `notify-rust`; дальнейшую фильтрацию баннеров оставляет macOS Focus.
4. Клик по уведомлению фокусирует окно и эмитит `alerts:open-project` с именем проекта (если есть).

## Consequences

- В quiet hours important-алерты не уходят в OS notify (наш gate), даже если Focus выключен.
- Если пользователь в Focus и система глушит Demerzel — уведомление может не появиться; мы это не обходим и не детектим.
- Если позже появится публичный Focus API для non-communication apps — пересмотреть этот ADR.

## Alternatives considered

- Включить Communication Notifications capability только ради `INFocusStatusCenter` — отклонён: capability для messaging UX, ложная семантика, лишние entitlements.
- Парсить `~/Library/DoNotDisturb/` / defaults — отклонён: хрупко, непублично.
