import SwiftUI

@main
struct SauronMenuApp: App {
    @State private var isRunning = false
    @State private var isPaused = false
    @State private var lastCapture: Date?
    @State private var todayCount = 0

    private let checker = StatusChecker()
    private let timer = Timer.publish(every: 10, on: .main, in: .common).autoconnect()

    var body: some Scene {
        MenuBarExtra {
            VStack(alignment: .leading) {
                Label(
                    isRunning ? (isPaused ? "Sauron is paused" : "Sauron is running") : "Sauron is stopped",
                    systemImage: isRunning ? (isPaused ? "pause.circle.fill" : "checkmark.circle.fill") : "xmark.circle.fill"
                )
                .foregroundStyle(isRunning ? (isPaused ? .orange : .green) : .red)

                if let lastCapture {
                    Text("Last capture: \(relativeTime(lastCapture))")
                        .font(.caption)
                }

                Text("Today: \(todayCount) screenshots")
                    .font(.caption)

                Divider()

                if isRunning {
                    Button(isPaused ? "Resume Captures" : "Pause Captures") {
                        isPaused = checker.togglePause()
                    }
                    Button("Stop Sauron") { runSauronCommand("stop") }
                } else {
                    Button("Start Sauron") { runSauronCommand("start") }
                }

                Button("Trigger Summary") { runSauronCommand("summary") }
                Button("Open Config") { openConfig() }

                Divider()

                Button("Quit") {
                    NSApplication.shared.terminate(nil)
                }
            }
            .padding(4)
            .onReceive(timer) { _ in refreshStatus() }
            .onAppear { refreshStatus() }
        } label: {
            Image(systemName: "eye.fill")
                .symbolRenderingMode(.palette)
                .foregroundStyle(isRunning ? (isPaused ? .orange : .green) : .red)
        }
    }

    private func refreshStatus() {
        isRunning = checker.isDaemonRunning()
        isPaused = checker.isPaused()
        lastCapture = checker.getLastScreenshot()
        todayCount = checker.getTodayCount()
    }

    private func relativeTime(_ date: Date) -> String {
        let seconds = Int(-date.timeIntervalSinceNow)
        if seconds < 60 { return "just now" }
        let minutes = seconds / 60
        if minutes < 60 { return "\(minutes) min ago" }
        let hours = minutes / 60
        return "\(hours)h ago"
    }

    private static let sauronCandidates = [
        "/usr/local/bin/sauron",
        "/opt/homebrew/bin/sauron",
    ]

    private func findSauronBinary() -> String? {
        Self.sauronCandidates.first { FileManager.default.fileExists(atPath: $0) }
    }

    private func runSauronCommand(_ subcommand: String) {
        guard let binary = findSauronBinary() else { return }

        DispatchQueue.global(qos: .userInitiated).async {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: binary)
            process.arguments = [subcommand]
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            process.environment = [
                "HOME": FileManager.default.homeDirectoryForCurrentUser.path,
                "PATH": "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin"
            ]
            try? process.run()

            if subcommand == "stop" {
                process.waitUntilExit()
            }

            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                self.refreshStatus()
            }
        }
    }

    private func openConfig() {
        let configURL = StatusChecker.configPath()
        // Ensure the config file exists before opening
        let dir = configURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if !FileManager.default.fileExists(atPath: configURL.path) {
            FileManager.default.createFile(atPath: configURL.path, contents: nil)
        }
        NSWorkspace.shared.open([configURL], withApplicationAt: URL(fileURLWithPath: "/System/Applications/TextEdit.app"), configuration: NSWorkspace.OpenConfiguration())
    }
}
