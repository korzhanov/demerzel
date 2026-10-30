import AppIntents

/// RestartProcess(project) — перезапуск сервиса через dem.sock с подтверждением (#101).
@available(macOS 15.0, *)
public struct RestartProcessIntent: AppIntent {
    public static var title: LocalizedStringResource = "Перезапустить в Demerzel"
    public static var description = IntentDescription(
        "Перезапустить процесс/сервис проекта Demerzel."
    )

    @Parameter(title: "Проект")
    public var project: ProjectEntity

    public init() {}

    public init(project: ProjectEntity) {
        self.project = project
    }

    public static var parameterSummary: some ParameterSummary {
        Summary("Перезапустить \(\.$project) в Demerzel")
    }

    public func perform() async throws -> some IntentResult & ProvidesDialog {
        // Подтверждение перед деструктивным restart (ADR-003 / #101).
        try await requestConfirmation(
            actionName: .custom(
                acceptLabel: "Перезапустить",
                acceptAlternatives: ["Да", "Restart"],
                denyLabel: "Отмена",
                denyAlternatives: ["Нет", "Cancel"],
                destructive: true
            ),
            dialog: IntentDialog(stringLiteral: "Перезапустить \(project.displayName) в Demerzel?")
        )

        let root = EcosystemRoot.resolve()
        do {
            let (dialog, _) = try DemSockClient.restartDialog(root: root, project: project.id)
            return .result(dialog: IntentDialog(stringLiteral: dialog))
        } catch {
            let msg = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            return .result(dialog: IntentDialog(stringLiteral: "Demerzel: \(msg)"))
        }
    }
}
