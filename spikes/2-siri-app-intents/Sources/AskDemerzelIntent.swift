import AppIntents
import SwiftUI

/// App Shortcut «Спроси Demerzel …» — пульс проекта через dem.sock.
/// Встраивание в Tauri host — #101; здесь эталон для спайка #100.
@available(macOS 13.0, iOS 16.0, *)
public struct AskDemerzelIntent: AppIntent {
    public static var title: LocalizedStringResource = "Спроси Demerzel"
    public static var description = IntentDescription(
        "Показать пульс проекта Demerzel (summary + ETA)."
    )

    @Parameter(title: "Проект")
    public var project: ProjectEntity

    public init() {}

    public init(project: ProjectEntity) {
        self.project = project
    }

    public static var parameterSummary: some ParameterSummary {
        Summary("Спроси Demerzel про \(\.$project)")
    }

    public func perform() async throws -> some IntentResult & ProvidesDialog & ShowsSnippetIntent {
        let root = Self.ecosystemRoot()
        do {
            let (dialog, pulse) = try DemSockClient.pulseDialog(root: root, project: project.id)
            return .result(
                dialog: IntentDialog(stringLiteral: dialog),
                view: PulseSnippetView(
                    project: pulse.project ?? project.id,
                    summary: pulse.summary,
                    etaSeconds: pulse.eta_seconds,
                    state: pulse.state
                )
            )
        } catch {
            let msg = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            return .result(
                dialog: IntentDialog(stringLiteral: "Demerzel: \(msg)"),
                view: PulseSnippetView(
                    project: project.id,
                    summary: msg,
                    etaSeconds: nil,
                    state: "error"
                )
            )
        }
    }

    /// Корень экосистемы: UserDefaults / env / разумный default для спайка.
    public static func ecosystemRoot() -> String {
        if let env = ProcessInfo.processInfo.environment["DEMERZEL_ROOT"], !env.isEmpty {
            return env
        }
        if let stored = UserDefaults.standard.string(forKey: "demerzel.ecosystemRoot"), !stored.isEmpty {
            return stored
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("code/korzhanov/demerzel/examples/parser").path
    }
}

@available(macOS 13.0, iOS 16.0, *)
public struct PulseSnippetView: View {
    public var project: String
    public var summary: String
    public var etaSeconds: Int?
    public var state: String?

    public var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(project).font(.headline)
            Text(summary).font(.body)
            if let eta = etaSeconds {
                Text(DemSockClient.formatEtaRu(eta)).font(.caption).foregroundStyle(.secondary)
            }
            if let state {
                Text(state).font(.caption2).foregroundStyle(.tertiary)
            }
        }
        .padding(8)
        .frame(maxWidth: 320, alignment: .leading)
    }
}
