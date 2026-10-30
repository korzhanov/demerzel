# Русские фразы App Shortcut (спайк #100)

Правило Apple: каждая phrase **должна** включать placeholder `\(.applicationName)`.
Имя приложения берётся из бандла (`productName` / display name) → **Demerzel**.

| Phrase (Swift) | Как слышит пользователь (ru) |
|---|---|
| `Спроси \(.applicationName) про \(\.$project)` | «Спроси Demerzel про parser» |
| `Спроси \(.applicationName) о \(\.$project)` | «Спроси Demerzel о parser» |
| `Статус \(.applicationName) \(\.$project)` | «Статус Demerzel parser» |

Один параметр на фразу (`$project` → `ProjectEntity`). Без имени приложения Siri App Shortcut не регистрируется.

Локализация: при `CFBundleLocalizations` / `ru.lproj` строки `LocalizedStringResource` подхватываются системой; для явных `.strings` / String Catalog см. WWDC22 «Implement App Shortcuts with App Intents».
