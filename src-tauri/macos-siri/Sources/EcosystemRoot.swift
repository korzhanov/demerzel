import Foundation

/// Корень экосистемы для dem.sock (`pulse` / `restart`).
/// Источники (по приоритету): env → App Group siri-config.json → UserDefaults → пример.
enum EcosystemRoot {
    static let appGroup = "group.app.demerzel.orchestrator"
    static let configFile = "siri-config.json"
    static let defaultsKey = "demerzel.ecosystemRoot"

    static func resolve() -> String {
        if let env = ProcessInfo.processInfo.environment["DEMERZEL_ROOT"], !env.isEmpty {
            return env
        }
        if let fromGroup = loadFromAppGroup(), !fromGroup.isEmpty {
            return fromGroup
        }
        if let suite = UserDefaults(suiteName: appGroup),
           let stored = suite.string(forKey: defaultsKey), !stored.isEmpty
        {
            return stored
        }
        if let stored = UserDefaults.standard.string(forKey: defaultsKey), !stored.isEmpty {
            return stored
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("code/korzhanov/demerzel/examples/parser").path
    }

    private static func loadFromAppGroup() -> String? {
        guard let root = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: appGroup
        ) else { return nil }
        let url = root.appendingPathComponent(configFile)
        guard let data = try? Data(contentsOf: url),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let ecosystemRoot = json["ecosystemRoot"] as? String,
              !ecosystemRoot.isEmpty
        else { return nil }
        return ecosystemRoot
    }
}
