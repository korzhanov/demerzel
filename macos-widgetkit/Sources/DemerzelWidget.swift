import WidgetKit
import SwiftUI

// Native WidgetKit fallback (#38): reads summary.json from App Group.
// Primary path remains tauri-plugin-widgets (ADR-001). Enable via settings.widgetkit.

let appGroupId = "group.app.demerzel.orchestrator"
let summaryFileName = "summary.json"
let widgetKind = "DemerzelNativeWidget"

struct ProjectStatus: Codable, Hashable {
    var name: String
    var running: Int
    var failing: Int
}

struct SummaryPayload: Codable, Hashable {
    var version: Int
    var projects: [ProjectStatus]
    var failing_total: Int
    var port_conflicts: Int
    var tasks_todo: Int
    var tasks_in_progress: Int
    var agents_running: Int
    var updated_at: String

    static let empty = SummaryPayload(
        version: 1,
        projects: [],
        failing_total: 0,
        port_conflicts: 0,
        tasks_todo: 0,
        tasks_in_progress: 0,
        agents_running: 0,
        updated_at: "—"
    )
}

enum SummaryStore {
    static func load() -> SummaryPayload {
        guard let root = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: appGroupId
        ) else {
            return .empty
        }
        let url = root.appendingPathComponent(summaryFileName)
        guard let data = try? Data(contentsOf: url),
              let decoded = try? JSONDecoder().decode(SummaryPayload.self, from: data)
        else {
            return .empty
        }
        return decoded
    }
}

struct DemerzelEntry: TimelineEntry {
    let date: Date
    let summary: SummaryPayload
}

struct DemerzelProvider: TimelineProvider {
    func placeholder(in context: Context) -> DemerzelEntry {
        DemerzelEntry(date: Date(), summary: .empty)
    }

    func getSnapshot(in context: Context, completion: @escaping (DemerzelEntry) -> Void) {
        completion(DemerzelEntry(date: Date(), summary: SummaryStore.load()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<DemerzelEntry>) -> Void) {
        let entry = DemerzelEntry(date: Date(), summary: SummaryStore.load())
        // Budget-friendly: refresh ~every 15 minutes (WidgetKit daily budget).
        let next = Calendar.current.date(byAdding: .minute, value: 15, to: Date()) ?? Date()
        completion(Timeline(entries: [entry], policy: .after(next)))
    }
}

struct DemerzelWidgetView: View {
    var entry: DemerzelEntry
    @Environment(\.widgetFamily) var family

    var body: some View {
        let failing = entry.summary.failing_total
        let tint = failing == 0 ? Color(red: 0.30, green: 0.69, blue: 0.31) : Color(red: 0.90, green: 0.22, blue: 0.21)

        VStack(alignment: .leading, spacing: 6) {
            Text("Demerzel")
                .font(.caption)
                .foregroundStyle(Color(white: 0.55))

            Text("\(failing) failing")
                .font(family == .systemSmall ? .title2.bold() : .title.bold())
                .foregroundStyle(.white)

            if family != .systemSmall {
                Text("Tasks: \(entry.summary.tasks_todo) todo · \(entry.summary.tasks_in_progress) in progress")
                    .font(.caption2)
                    .foregroundStyle(Color(white: 0.55))
                if entry.summary.port_conflicts > 0 {
                    Text("Port conflicts: \(entry.summary.port_conflicts)")
                        .font(.caption2)
                        .foregroundStyle(tint)
                }
            } else {
                Text(entry.summary.port_conflicts == 0 ? "Ports ok" : "Port conflicts")
                    .font(.caption2)
                    .foregroundStyle(tint)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .padding(12)
        .containerBackground(for: .widget) {
            Color(red: 0.05, green: 0.05, blue: 0.07)
        }
    }
}

@main
struct DemerzelNativeWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: widgetKind, provider: DemerzelProvider()) { entry in
            DemerzelWidgetView(entry: entry)
        }
        .configurationDisplayName("Demerzel")
        .description("Статус стека из summary.json (fallback WidgetKit)")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}
