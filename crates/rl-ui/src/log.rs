//! The message log.
//!
//! A bounded ring of lines, each carrying the [`ToneId`] it should be read
//! in, so colouring and filtering are a lookup and never a search through
//! English. A repeated line folds into the one above it with a count
//! rather than filling the panel, which is what turns "you are bitten"
//! five times into one readable line.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::tone::{ToneId, Tones};

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// The text.
    pub text: String,
    /// How it reads.
    pub tone: ToneId,
    /// The whole turn it was logged on.
    pub turn: u32,
    /// How many times in a row it was logged. Always at least one.
    pub count: u32,
}

impl LogEntry {
    /// The line as it should be shown: with its count when it repeated.
    pub fn display(&self) -> String {
        if self.count > 1 { format!("{} (x{})", self.text, self.count) } else { self.text.clone() }
    }
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

    /// Appends a message, folding it into the line above if that line says
    /// the same thing in the same tone.
    ///
    /// Folded rather than repeated because a four-line log filled by one
    /// event is a log that has stopped reporting; the count keeps the
    /// information and the room.
    pub fn push(&mut self, text: impl Into<String>, tone: ToneId, turn: u32) {
        let text = text.into();
        if let Some(last) = self.entries.back_mut()
            && last.text == text
            && last.tone == tone
        {
            last.count += 1;
            last.turn = turn;
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(LogEntry { text, tone, turn, count: 1 });
    }

    /// Appends plain narration.
    pub fn info(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, Tones::TEXT, turn);
    }

    /// Appends something that matters less than the rest.
    pub fn muted(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, Tones::MUTED, turn);
    }

    /// Appends good news.
    pub fn good(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, Tones::GOOD, turn);
    }

    /// Appends bad news.
    pub fn bad(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, Tones::BAD, turn);
    }

    /// Appends something worth noticing.
    pub fn notice(&mut self, text: impl Into<String>, turn: u32) {
        self.push(text, Tones::NOTICE, turn);
    }

    /// The last `n` entries, newest first.
    pub fn recent(&self, n: usize) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().rev().take(n)
    }

    /// Every entry, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter()
    }

    /// Every entry in `tone`, oldest first. What a filtered history screen
    /// reads.
    pub fn in_tone(&self, tone: ToneId) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().filter(move |e| e.tone == tone)
    }

    /// Number of entries kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Forgets everything. What starting a new run does.
    pub fn clear(&mut self) {
        self.entries.clear();
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

    #[test]
    fn the_same_line_twice_folds_into_a_count_and_keeps_the_later_turn() {
        let mut log = MessageLog::default();
        log.bad("the crab nips you", 4);
        log.bad("the crab nips you", 5);
        log.bad("the crab nips you", 6);
        assert_eq!(log.len(), 1);
        let entry = log.recent(1).next().unwrap();
        assert_eq!(entry.count, 3);
        assert_eq!(entry.turn, 6);
        assert_eq!(entry.display(), "the crab nips you (x3)");
    }

    #[test]
    fn the_same_words_in_a_different_tone_are_a_different_line() {
        let mut log = MessageLog::default();
        log.info("it moves", 1);
        log.bad("it moves", 1);
        assert_eq!(log.len(), 2);
        assert_eq!(log.in_tone(Tones::BAD).count(), 1);
    }
}
