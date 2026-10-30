import AppIntents

/// App Shortcuts: фразы **обязаны** содержать `\(.applicationName)`.
@available(macOS 15.0, *)
public struct DemerzelAppShortcuts: AppShortcutsProvider {
    public static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: AskDemerzelStatusIntent(),
            phrases: [
                "Спроси \(.applicationName) про \(\.$project)",
                "Спроси \(.applicationName) о \(\.$project)",
                "Статус \(.applicationName) \(\.$project)",
                "Ask \(.applicationName) about \(\.$project)",
            ],
            shortTitle: "Спроси Demerzel",
            systemImageName: "waveform"
        )
        AppShortcut(
            intent: RestartProcessIntent(),
            phrases: [
                "Перезапусти \(.applicationName) \(\.$project)",
                "Restart \(.applicationName) \(\.$project)",
            ],
            shortTitle: "Перезапуск Demerzel",
            systemImageName: "arrow.clockwise"
        )
    }
}
