//! The message log.

use std::collections::VecDeque;

use bevy::prelude::*;

/// What kind of thing a message reports. Colour and filtering key on this,
/// never on the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogCategory {
    /// Plain narration.
    Info,
    /// Low-importance detail.
    Muted,
    /// Good news for the player.
    Good,
    /// Bad news for the player.
    Bad,
    /// Something worth noticing.
    Notice,
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// The text.
    pub text: String,
    /// What kind of message.
    pub category: LogCategory,
    /// The whole turn it was logged on.
    pub turn: u32,
}

/// A bounded ring of messages.
#[derive(Resource, Debug)]
pub struct MessageLog {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl Default for MessageLog {
    fn default() -> Self {
        Self::with_capacity(500)
    }
}

impl MessageLog {
    /// A log that keeps the last `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { entries: VecDeque::with_capacity(capacity), capacity }
    }

    /// Appends a message.
    pub fn push(&mut self, text: impl Into<String>, category: LogCategory, turn: u32) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry { text: text.into(), category, turn });
    }

    /// Appends plain narration.
    pub fn info(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, LogCategory::Info, turn);
    }

    /// The last `n` entries, newest first.
    pub fn recent(&self, n: usize) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().rev().take(n)
    }

    /// Every entry, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Number of entries kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_log_is_bounded_and_recent_is_newest_first() {
        let mut log = MessageLog::with_capacity(3);
        for i in 0..5 {
            log.info(format!("m{i}"), i);
        }
        assert_eq!(log.len(), 3);
        let recent: Vec<&str> = log.recent(2).map(|e| e.text.as_str()).collect();
        assert_eq!(recent, vec!["m4", "m3"]);
        assert_eq!(log.iter().next().unwrap().text, "m2");
    }
}
