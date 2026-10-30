# Доступ к папкам на macOS

Demerzel читает папки проектов. macOS защищает некоторые места (`Documents`, `Desktop`, `Downloads`, iCloud Drive, внешние диски) и без разрешения возвращает `Operation not permitted` или `Permission denied`.

## Что делать

1. Системные настройки → Конфиденциальность и безопасность → **Файлы и папки**. Разрешите Demerzel доступ к нужным папкам.
2. Если проекты лежат в других местах, выберите **Полный доступ к диску** и добавьте Demerzel.
3. Перезапустите приложение.

## Как видно в приложении и CLI

- Приложение показывает сообщение с путём к папке и ссылкой на этот документ.
- При сканировании корней (`scanner`) подсказка попадает в UI; CLI `dem status` / `dem up` читают манифест в `ROOT` и печатают ошибку доступа к пути сервиса, если OS запретила чтение (см. [docs/cli.md](cli.md)).

## iCloud Drive и summary (#37)

Если включена синхронизация `settings.icloud.enabled`, Demerzel пишет `summary.json` в  
`~/Library/Mobile Documents/com~apple~CloudDocs/<folder>/`. Без доступа к iCloud Drive запись вернёт ошибку; macOS-виджет при этом может продолжать работать. Подробности: [icloud-summary.md](icloud-summary.md).

## App Group и WidgetKit fallback (#38)

При `settings.widgetkit.enabled` (или forced fallback после ошибки плагина) host пишет  
`~/Library/Group Containers/group.app.demerzel.orchestrator/summary.json`. Для рабочего контейнера нужны подпись и App Group в Developer portal — см. [widgetkit-fallback.md](widgetkit-fallback.md).

## Для разработчиков

- Подсказка формируется в `backend/scanner.rs::access_hint` для `io::ErrorKind::PermissionDenied`.
- Подпись приложения для локальных сборок влияет на выдачу разрешений: см. спайк `spikes/1-widget-plugin/` и `scripts/sign-macos.sh`.
