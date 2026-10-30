# Demerzel

The Shadow Orchestrator of Your Local Stack Empire.

Локальный оркестратор для разработки на macOS: дашборд проектов и подпроектов, статусы задач, распределение портов, запуск агентов для исправления багов, виджет на рабочем столе и голосовые алерты.

## Стек

- Tauri 2 (оболочка и бэкенд на Rust)
- SvelteKit в режиме SPA (интерфейс)
- `tauri-plugin-widgets` (виджет macOS, запасной вариант: Swift WidgetKit)
- SQLite (состояние)

Решения зафиксированы в [docs/decisions](docs/decisions).

## Статус

Ранняя разработка. План работ: [issues](https://github.com/korzhanov/demerzel/issues), фазы через метки `phase-0` … `phase-7`.

## Документация

- [docs/CONVENTIONS.md](docs/CONVENTIONS.md): ветки, коммиты, версии, маркетинговый лог
- [references/](references/README.md): мокапы, вайрфреймы, маркетинговые ассеты
- [CHANGELOG.md](CHANGELOG.md): технический журнал изменений
- [MARKETING.md](MARKETING.md): лог для публикаций в соцсетях

## Лицензия

Не выбрана.
