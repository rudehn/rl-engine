//! The morgue: the record of a run that has ended.
//!
//! Every roguelike keeps one, and every one is the same shape: who the
//! character was, how it ended, what it had, and the last things that
//! happened, written as a text file the player can read, keep or paste into
//! a bug report. The shape is the engine's, as an [`Obituary`]; what goes
//! in it is the game's as much as it likes, through [`Morgue::section`].
//! Where it is written is a [`SaveBackend`], the same seam a save goes
//! through, so a browser game keeps its morgue in storage and a native one
//! in a `morgue` folder beside its saves.
//!
//! Nothing here decides when a run is over or reads the log: the layer that
//! knows those, the UI, composes the obituary on the frame the run ends and
//! hands it here to be filed.

use std::sync::Arc;

use bevy::prelude::Resource;
use rl_core::RunSeed;

use crate::backend::{FileBackend, SaveBackend, SaveError};

/// What is written about a run.
///
/// The header is the engine's: the title, the seed, the turn and the
/// outcome in one line, and the game's own epitaph under it. Then every
/// section in order, each a heading and a body, wherever it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obituary {
    /// The game's title, as the first line.
    pub title: String,
    /// The run's seed, so the run can be had again.
    pub seed: RunSeed,
    /// The whole turn it ended on.
    pub turn: u32,
    /// How it ended, in one line: `Killed by the goblin`, `Won`.
    pub outcome: String,
    /// The game's own words about the ending, or nothing.
    pub epitaph: String,
    /// Headed sections, in order.
    pub sections: Vec<(String, String)>,
}

impl Obituary {
    /// A header with nothing under it yet.
    pub fn new(title: impl Into<String>, seed: RunSeed, turn: u32, outcome: impl Into<String>) -> Self {
        Self { title: title.into(), seed, turn, outcome: outcome.into(), epitaph: String::new(), sections: Vec::new() }
    }

    /// Adds a section.
    pub fn section(mut self, heading: impl Into<String>, body: impl Into<String>) -> Self {
        self.sections.push((heading.into(), body.into()));
        self
    }

    /// The file's name, without an extension: the title, the seed and the
    /// turn, so two runs never share one and the player can tell them apart
    /// in a folder.
    pub fn slot(&self) -> String {
        let title: String = self.title.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        format!("{}-{}-turn{}", title.trim_matches('-'), self.seed.0, self.turn)
    }

    /// The whole file, as plain text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.title);
        out.push('\n');
        out.push_str(&"=".repeat(self.title.chars().count().max(1)));
        out.push_str(&format!("\n\nSeed {}. {} on turn {}.\n", self.seed.0, self.outcome, self.turn));
        if !self.epitaph.is_empty() {
            out.push_str(&self.epitaph);
            out.push('\n');
        }
        for (heading, body) in &self.sections {
            out.push_str(&format!("\n{heading}\n{}\n{body}\n", "-".repeat(heading.chars().count().max(1))));
        }
        out
    }
}

/// Where a run's obituary is filed, and what the game has to add to the
/// next one.
///
/// Sections a game pushes with [`section`](Self::section) go into the next
/// obituary written and are then forgotten, so a game reacting to the run's
/// end pushes what it knows and the engine files it with the rest.
#[derive(Resource)]
pub struct Morgue {
    backend: Arc<dyn SaveBackend + Send + Sync>,
    title: String,
    sections: Vec<(String, String)>,
    last: Option<String>,
}

impl Morgue {
    /// The platform's default: text files in a `morgue` folder beside the
    /// executable, or browser storage under `name` on wasm, with `title` on
    /// the first line of every file.
    pub fn platform_default(name: &str, title: impl Into<String>) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::new(crate::backend::WebBackend::new(format!("{name}:morgue")), title)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = name;
            Self::new(FileBackend::new(FileBackend::dir_beside_executable().join("morgue")).with_extension("txt"), title)
        }
    }

    /// Files through `backend`.
    pub fn new(backend: impl SaveBackend + Send + Sync + 'static, title: impl Into<String>) -> Self {
        Self { backend: Arc::new(backend), title: title.into(), sections: Vec::new(), last: None }
    }

    /// The title every file starts with.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Adds a section to the next obituary written.
    pub fn section(&mut self, heading: impl Into<String>, body: impl Into<String>) -> &mut Self {
        self.sections.push((heading.into(), body.into()));
        self
    }

    /// The sections pushed since the last file, taken for it.
    pub fn take_sections(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.sections)
    }

    /// Writes `obituary`, and answers where: the slot it went to.
    pub fn file(&mut self, obituary: &Obituary) -> Result<String, SaveError> {
        let slot = obituary.slot();
        self.backend.persist(&slot, &obituary.render())?;
        self.last = Some(slot.clone());
        Ok(slot)
    }

    /// The slot the last obituary went to, if one has.
    pub fn last(&self) -> Option<&str> {
        self.last.as_deref()
    }

    /// Reads back the file at `slot`, for a test or a screen that shows it.
    pub fn read(&self, slot: &str) -> Result<Option<String>, SaveError> {
        self.backend.load(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryBackend;

    #[test]
    fn an_obituary_renders_its_header_epitaph_and_sections_and_names_its_file_by_the_run() {
        let o = Obituary::new("The Hollow Whale", RunSeed(7), 212, "Killed by the gut eel").section("Last words", "You die.");
        let mut o = o;
        o.epitaph = "The whale keeps you.".into();
        assert_eq!(o.slot(), "the-hollow-whale-7-turn212");
        let text = o.render();
        assert!(text.starts_with("The Hollow Whale\n================\n\nSeed 7. Killed by the gut eel on turn 212.\nThe whale keeps you.\n"), "{text}");
        assert!(text.contains("\nLast words\n----------\nYou die.\n"), "{text}");
    }

    #[test]
    fn the_morgue_files_what_the_game_added_and_forgets_it_for_the_next() {
        let mut morgue = Morgue::new(MemoryBackend::default(), "Corsair");
        morgue.section("Ledger", "Retire rich: done.");
        let sections = morgue.take_sections();
        assert_eq!(sections.len(), 1);
        assert!(morgue.take_sections().is_empty(), "taken once");
        let mut o = Obituary::new(morgue.title(), RunSeed(3), 9, "Won");
        o.sections = sections;
        let slot = morgue.file(&o).unwrap();
        assert_eq!(morgue.last(), Some(slot.as_str()));
        assert!(morgue.read(&slot).unwrap().unwrap().contains("Retire rich: done."));
    }
}
