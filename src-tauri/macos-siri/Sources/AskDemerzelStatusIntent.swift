import AppIntents

/// AskDemerzelStatus(project) — пульс проекта через dem.sock (#101).
@available(macOS 15.0, *)
public struct AskDemerzelStatusIntent: AppIntent {
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

    public func perform() async throws -> some IntentResult & ProvidesDialog {
        let root = EcosystemRoot.resolve()
        do {
            let (dialog, _) = try DemSockClient.pulseDialog(root: root, project: project.id)
            return .result(dialog: IntentDialog(stringLiteral: dialog))
        } catch {
            let msg = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            return .result(dialog: IntentDialog(stringLiteral: "Demerzel: \(msg)"))
        }
    }
}
