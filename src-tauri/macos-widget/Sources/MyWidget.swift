import WidgetKit
import SwiftUI
import TauriWidgets

// Primary macOS widget via tauri-plugin-widgets (ADR-001).
// appGroup / extensionBundleId must match plugins.widgets in tauri.conf.json.
// Fallback native WidgetKit (summary.json) lives in macos-widgetkit/ (#38).

@main
struct MyWidget: Widget {
    let kind = "MyTauriWidget"
    let appGroup = "group.app.demerzel.orchestrator"
    let widgetId = "default"

    var body: some WidgetConfiguration {
        StaticConfiguration(
            kind: kind,
            provider: TauriWidgetProvider(appGroup: appGroup, widgetId: widgetId, refreshMinutes: 15)
        ) { entry in
            TauriWidgetView(entry: entry)
        }
        .configurationDisplayName("Demerzel")
        .description("Статус портов и упавших сервисов")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
        .contentMarginsDisabled()
    }
}
