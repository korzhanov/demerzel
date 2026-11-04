//! Журнал активности и экспорт записей в iCalendar (.ics).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub uid: String,
    pub category: String,
    pub summary: String,
    pub description: String,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Default)]
pub struct ActivityJournal {
    path: Option<PathBuf>,
    records: Vec<ActivityRecord>,
}

impl ActivityJournal {
    pub fn configure(&mut self, path: PathBuf) {
        self.records = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        self.path = Some(path);
    }

    pub fn record(&mut self, record: ActivityRecord) -> Result<bool, String> {
        if let Some(index) = self.records.iter().position(|existing| existing.uid == record.uid) {
            if self.records[index] == record {
                return Ok(false);
            }
            let previous = std::mem::replace(&mut self.records[index], record);
            if let Err(error) = self.persist() {
                self.records[index] = previous;
                return Err(error);
            }
            return Ok(true);
        }
        self.records.push(record);
        if let Err(error) = self.persist() {
            self.records.pop();
            return Err(error);
        }
        Ok(true)
    }

    pub fn export(&self, path: &Path, include_processes: bool, include_agents: bool) -> Result<(), String> {
        let mut text = String::from(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Demerzel//Activity Calendar//EN\r\nCALSCALE:GREGORIAN\r\nMETHOD:PUBLISH\r\n",
        );
        for event in &self.records {
            if (event.category == "process" && !include_processes)
                || (event.category == "agent" && !include_agents)
            {
                continue;
            }
            let start = parse_time(&event.started_at)?;
            let end = parse_time(&event.finished_at)?.max(start + chrono::Duration::seconds(1));
            text.push_str("BEGIN:VEVENT\r\n");
            push_property(&mut text, "UID", &format!("{}@demerzel", event.uid));
            push_property(
                &mut text,
                "DTSTAMP",
                &Utc::now().format("%Y%m%dT%H%M%SZ").to_string(),
            );
            push_property(&mut text, "DTSTART", &start.format("%Y%m%dT%H%M%SZ").to_string());
            push_property(&mut text, "DTEND", &end.format("%Y%m%dT%H%M%SZ").to_string());
            push_property(&mut text, "SUMMARY", &escape_text(&event.summary));
            push_property(&mut text, "DESCRIPTION", &escape_text(&event.description));
            text.push_str("END:VEVENT\r\n");
        }
        text.push_str("END:VCALENDAR\r\n");
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("ics.tmp");
        std::fs::write(&temp, text).map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    }

    fn persist(&self) -> Result<(), String> {
        let Some(path) = &self.path else { return Ok(()); };
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(&self.records).map_err(|e| e.to_string())?;
        std::fs::write(&temp, json).map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    }
}

fn parse_time(value: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(time) = DateTime::parse_from_rfc3339(value) {
        return Ok(time.with_timezone(&Utc));
    }
    let seconds = value.parse::<i64>().map_err(|_| format!("invalid calendar timestamp: {value}"))?;
    DateTime::from_timestamp(seconds, 0).ok_or_else(|| format!("invalid calendar timestamp: {value}"))
}

fn escape_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n")
}

fn push_property(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push(':');
    out.push_str(value);
    out.push_str("\r\n");
}
