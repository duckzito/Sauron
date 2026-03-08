use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS screenshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                captured_at TEXT NOT NULL,
                file_path TEXT NOT NULL,
                summary TEXT,
                model_used TEXT,
                processing_method TEXT,
                processed_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS daily_summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                summary_date TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                file_path TEXT NOT NULL,
                screenshot_count INTEGER,
                email_sent_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        )?;
        Ok(())
    }

    pub fn insert_screenshot(&self, captured_at: &str, file_path: &str) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO screenshots (captured_at, file_path) VALUES (?1, ?2)",
            [captured_at, file_path],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_screenshot_summary(
        &self,
        id: i64,
        summary: &str,
        model_used: &str,
        processing_method: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE screenshots SET summary = ?1, model_used = ?2, processing_method = ?3, processed_at = datetime('now') WHERE id = ?4",
            rusqlite::params![summary, model_used, processing_method, id],
        )?;
        Ok(())
    }

    pub fn get_day_summaries(&self, date: &str) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT captured_at, summary FROM screenshots WHERE captured_at LIKE ?1 AND summary IS NOT NULL ORDER BY captured_at"
        )?;
        let pattern = format!("{}%", date);
        let rows = stmt.query_map([pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_day_screenshot_count(&self, date: &str) -> anyhow::Result<i64> {
        let pattern = format!("{}%", date);
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM screenshots WHERE captured_at LIKE ?1",
            [pattern],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn insert_daily_summary(
        &self,
        summary_date: &str,
        content: &str,
        file_path: &str,
        screenshot_count: i64,
    ) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO daily_summaries (summary_date, content, file_path, screenshot_count) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(summary_date) DO UPDATE SET \
             content = excluded.content, \
             file_path = excluded.file_path, \
             screenshot_count = excluded.screenshot_count",
            rusqlite::params![summary_date, content, file_path, screenshot_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_email_sent(&self, summary_date: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE daily_summaries SET email_sent_at = datetime('now') WHERE summary_date = ?1",
            [summary_date],
        )?;
        Ok(())
    }

    pub fn get_last_screenshot(&self) -> anyhow::Result<Option<(String, String)>> {
        let result = self.conn.query_row(
            "SELECT captured_at, file_path FROM screenshots ORDER BY captured_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
