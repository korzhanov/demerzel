import WidgetKit
import SwiftUI
import TauriWidgets

// ─── Entry Point ─────────────────────────────────────────────────────────────
// Adjust `appGroup`, `kind`, and `widgetId` to match your app.
// Placeholders: group.app.demerzel.orchestrator, MyTauriWidget, default

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
        .description("Spike: failing count from Rust tick")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
        .contentMarginsDisabled()
    }
}
