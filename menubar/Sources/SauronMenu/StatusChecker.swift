import Foundation
import SQLite3

final class StatusChecker {
    private let configDir: URL
    private let dbPath: URL

    init() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        // Rust dirs::config_dir() returns ~/Library/Application Support on macOS
        self.configDir = home.appendingPathComponent("Library/Application Support/sauron")
        self.dbPath = home.appendingPathComponent("sauron/sauron.db")
    }

    // MARK: - Pause Control

    func isPaused() -> Bool {
        let pauseFile = configDir.appendingPathComponent("sauron.paused")
        return FileManager.default.fileExists(atPath: pauseFile.path)
    }

    func togglePause() -> Bool {
        let pauseFile = configDir.appendingPathComponent("sauron.paused")
        if FileManager.default.fileExists(atPath: pauseFile.path) {
            try? FileManager.default.removeItem(at: pauseFile)
            return false
        } else {
            FileManager.default.createFile(atPath: pauseFile.path, contents: nil)
            return true
        }
    }

    // MARK: - Daemon Status

    func isDaemonRunning() -> Bool {
        let pidFile = configDir.appendingPathComponent("sauron.pid")
        guard let contents = try? String(contentsOf: pidFile, encoding: .utf8),
              let pid = Int32(contents.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            return false
        }
        // kill with signal 0 checks if process exists without sending a signal
        return kill(pid, 0) == 0
    }

    // MARK: - SQLite Queries

    func getLastScreenshot() -> Date? {
        guard let db = openDB() else { return nil }
        defer { sqlite3_close(db) }

        var stmt: OpaquePointer?
        let sql = "SELECT captured_at FROM screenshots ORDER BY id DESC LIMIT 1"
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return nil }
        defer { sqlite3_finalize(stmt) }

        guard sqlite3_step(stmt) == SQLITE_ROW else { return nil }
        guard let cString = sqlite3_column_text(stmt, 0) else { return nil }

        let dateStr = String(cString: cString)
        return parseDate(dateStr)
    }

    func getTodayCount() -> Int {
        guard let db = openDB() else { return 0 }
        defer { sqlite3_close(db) }

        let today = Self.todayString()
        let sql = "SELECT COUNT(*) FROM screenshots WHERE captured_at LIKE '\(today)%'"

        var stmt: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return 0 }
        defer { sqlite3_finalize(stmt) }

        guard sqlite3_step(stmt) == SQLITE_ROW else { return 0 }
        return Int(sqlite3_column_int(stmt, 0))
    }

    // MARK: - Helpers

    private func openDB() -> OpaquePointer? {
        var db: OpaquePointer?
        let flags = SQLITE_OPEN_READONLY | SQLITE_OPEN_NOMUTEX
        guard sqlite3_open_v2(dbPath.path, &db, flags, nil) == SQLITE_OK else {
            if let db = db { sqlite3_close(db) }
            return nil
        }
        return db
    }

    private func parseDate(_ str: String) -> Date? {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        // Try common formats
        for format in ["yyyy-MM-dd HH:mm:ss", "yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd HH:mm:ss.SSS"] {
            formatter.dateFormat = format
            if let date = formatter.date(from: str) { return date }
        }
        return nil
    }

    static func todayString() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: Date())
    }

    static func configPath() -> URL {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent("Library/Application Support/sauron/config.toml")
    }
}
