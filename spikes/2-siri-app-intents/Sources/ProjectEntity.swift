import AppIntents
import Foundation

/// Сущность «проект» для App Shortcut (параметр Siri / Shortcuts).
/// Список берётся из live `dem pulse --json` или из кэша App Group `summary.json` (#38).
@available(macOS 13.0, iOS 16.0, *)
public struct ProjectEntity: AppEntity {
    public static var typeDisplayRepresentation = TypeDisplayRepresentation(name: "Проект")
    public static var defaultQuery = ProjectEntityQuery()

    public var id: String
    public var displayName: String

    public var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(displayName)")
    }

    public init(id: String, displayName: String? = nil) {
        self.id = id
        self.displayName = displayName ?? id
    }
}

@available(macOS 13.0, iOS 16.0, *)
public struct ProjectEntityQuery: EntityQuery {
    public init() {}

    public func entities(for identifiers: [String]) async throws -> [ProjectEntity] {
        let all = try await suggestedEntities()
        return all.filter { identifiers.contains($0.id) }
    }

    public func suggestedEntities() async throws -> [ProjectEntity] {
        // Спайк: статический пример + попытка прочитать App Group summary.
        // #101: подставить список из dem.sock / demerzel.yaml root из настроек.
        if let fromGroup = Self.loadFromAppGroup(), !fromGroup.isEmpty {
            return fromGroup
        }
        return [
            ProjectEntity(id: "parser", displayName: "parser"),
            ProjectEntity(id: "web", displayName: "web"),
            ProjectEntity(id: "api", displayName: "api"),
        ]
    }

    private static func loadFromAppGroup() -> [ProjectEntity]? {
        let group = "group.app.demerzel.orchestrator"
        guard let root = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: group
        ) else { return nil }
        let url = root.appendingPathComponent("summary.json")
        guard let data = try? Data(contentsOf: url),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let projects = json["projects"] as? [[String: Any]]
        else { return nil }
        return projects.compactMap { row in
            guard let name = row["name"] as? String, !name.isEmpty else { return nil }
            return ProjectEntity(id: name, displayName: name)
        }
    }
}
