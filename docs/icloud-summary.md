# Синхронизация summary.json через iCloud (#37)

Demerzel (macOS) может писать ту же сводку, что питает виджет, в **iCloud Drive**, чтобы файл был виден на iPhone в приложении **Файлы**. Это не Swift WidgetKit и не iOS-виджет — нативный WidgetKit fallback: [widgetkit-fallback.md](widgetkit-fallback.md) (#38).

## Зачем

На Mac виджет читает IR через `tauri-plugin-widgets` / App Group. На iPhone нет хоста Demerzel: нужен общий файл. iCloud Drive даёт путь без отдельного бэкенда.

## Путь

| | |
|---|---|
| Контейнер | `~/Library/Mobile Documents/com~apple~CloudDocs/` |
| По умолчанию | `…/Demerzel/summary.json` |
| На iPhone | Файлы → iCloud Drive → **Demerzel** → `summary.json` |

Формат файла — тот же versioned JSON, что [#24](https://github.com/korzhanov/demerzel/issues/24) (`version`, `projects`, `failing_total`, задачичётчики задач/агентов, `updated_at`). Запись атомарная (`*.json.tmp` → rename), без перезаписи при неизменном содержимом.

## Настройка (`settings.json`)

Секция приложения (не `orchestrator.json`):

```json
{
  "icloud": {
    "enabled": true,
    "folder": "Demerzel"
  }
}
```

| Поле | По умолчанию | Смысл |
|---|---|---|
| `enabled` | `false` | Писать в iCloud Drive |
| `folder` | `Demerzel` | Относительная папка внутри CloudDocs (`..` и абсолютные пути отклоняются → `Demerzel`) |

## Команды Tauri

| Команда | Поведение |
|---|---|
| `sync_summary_to_icloud(summary)` | Синхронизация по `settings.icloud`; `SkippedDisabled` / `Unchanged` / `Written` |
| `refresh_widget_from_summary` | Best-effort sync в iCloud, затем обновление macOS-виджета |

Если iCloud Drive не смонтирован (нет входа в Apple ID / каталога CloudDocs) — ошибка `NotFound` с путём; виджет на Mac при этом не обязан падать.

## Доступ на macOS

Запись в iCloud Drive может требовать разрешение «Файлы и папки» / полный доступ к диску — см. [permissions.md](permissions.md).

## iOS-цель (потребление)

1. Включить `icloud.enabled` на Mac и дождаться записи `summary.json`.
2. На iPhone: **Файлы** → iCloud Drive → папка из `folder`.
3. Дальше: Shortcuts / сторонний виджет / будущий iOS-клиент читают JSON. Нативный macOS WidgetKit (App Group, не iCloud) — [widgetkit-fallback.md](widgetkit-fallback.md).

## Модуль

Чистая логика: `src-tauri/src/backend/icloud_sync.rs` (без Tauri в unit-тестах).
