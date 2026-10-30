import AppIntents

/// App Shortcuts: фразы **обязаны** содержать `\(.applicationName)`.
/// Русская локаль — через LocalizedStringResource (системный язык ru).
@available(macOS 13.0, iOS 16.0, *)
public struct DemerzelAppShortcuts: AppShortcutsProvider {
    public static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: AskDemerzelIntent(),
            phrases: [
                "Спроси \(.applicationName) про \(\.$project)",
                "Спроси \(.applicationName) о \(\.$project)",
                "Статус \(.applicationName) \(\.$project)",
                "Ask \(.applicationName) about \(\.$project)",
            ],
            shortTitle: "Спроси Demerzel",
            systemImageName: "waveform"
        )
    }
}
