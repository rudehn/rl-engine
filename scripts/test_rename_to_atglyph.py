"""Tests for scripts/rename-to-atglyph.py.

    python3 -m unittest scripts/test_rename_to_atglyph.py -v

The script's name has a hyphen, as every script here does, so it is loaded
from its path rather than imported by name.
"""

import importlib.util
import pathlib
import unittest

_SCRIPT = pathlib.Path(__file__).with_name("rename-to-atglyph.py")
_SPEC = importlib.util.spec_from_file_location("rename_to_atglyph", _SCRIPT)
rename = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(rename)

rewrite = rename.rewrite
classify = rename.classify
rename_urls = rename.rename_urls
rewrite_changelog = rename.rewrite_changelog


class Rewrite(unittest.TestCase):
    def test_a_facade_path_takes_its_short_name(self):
        self.assertEqual(rewrite("use rl_engine::rl_bevy::TurnSet;", facade_user=True), "use atglyph::ecs::TurnSet;")
        self.assertEqual(rewrite("rl_engine::rl_core::Point", facade_user=False), "atglyph::primitives::Point")

    def test_a_bare_crate_path_in_a_game_goes_through_the_facade(self):
        self.assertEqual(rewrite("`rl_mapgen::Dress`", facade_user=True), "`atglyph::mapgen::Dress`")

    def test_a_bare_crate_path_in_the_engine_keeps_its_crate(self):
        self.assertEqual(rewrite("use rl_rules::Registry;", facade_user=False), "use atglyph_rules::Registry;")

    def test_crate_names_paths_and_the_metadata_key_move(self):
        self.assertEqual(rewrite("[package.metadata.rl-engine]", facade_user=False), "[package.metadata.atglyph]")
        self.assertEqual(rewrite("crates/rl-bevy/src/noise.rs", facade_user=True), "crates/atglyph-bevy/src/noise.rs")
        self.assertEqual(rewrite("rl-engine.workspace = true", facade_user=True), "atglyph.workspace = true")

    def test_the_repository_and_guide_urls_stay_until_the_release(self):
        text = "https://github.com/rudehn/rl-engine/blob/main/x and https://rudehn.github.io/rl-engine/"
        self.assertEqual(rewrite(text, facade_user=True), text)
        self.assertEqual(rename_urls(text), "https://github.com/rudehn/atglyph/blob/main/x and https://rudehn.github.io/atglyph/")

    def test_seed_domains_and_other_words_are_left_alone(self):
        for text in ['b"rl.loot"', '("rl.combat", COMBAT)', "ctrl_ui", "rl_ui_layout", "curl-core", "rl-test-times.tsv"]:
            self.assertEqual(rewrite(text, facade_user=False), text)

    def test_environment_variables_move(self):
        self.assertEqual(rewrite("RL_FULL_SEEDS=1", facade_user=False), "ATGLYPH_FULL_SEEDS=1")
        self.assertEqual(rewrite("RL_CAPTURE_AT", facade_user=False), "ATGLYPH_CAPTURE_AT")

    def test_a_rewrite_is_idempotent(self):
        once = rewrite("use rl_engine::rl_ui::LogPanel; rl-core", facade_user=True)
        self.assertEqual(rewrite(once, facade_user=True), once)

    def test_history_and_links_are_skipped(self):
        for path in ["docs/reviews/a.md", "docs/superpowers/plans/x.md", ".planning/y.md", "docs/PLAN.md", "examples/undermere/SOAK.md", "Cargo.lock"]:
            self.assertEqual(classify(pathlib.Path(path)), "skip")
        self.assertEqual(classify(pathlib.Path("examples/terminal/src/main.rs")), "engine")
        self.assertEqual(classify(pathlib.Path("docs/guide/src/systems/noise.md")), "facade_user")

    def test_the_facade_reexports_each_sub_crate_under_its_short_name(self):
        text = "pub use atglyph_bevy;\n#[cfg(not(target_arch = \"wasm32\"))]\npub use atglyph_tty;\n"
        expected = "pub use atglyph_bevy as ecs;\n#[cfg(not(target_arch = \"wasm32\"))]\npub use atglyph_tty as tty;\n"
        self.assertEqual(rename.reexport_short_names(text), expected)
        self.assertEqual(rename.reexport_short_names(expected), expected)

    def test_the_changelog_keeps_its_releases_and_the_renames_own_entry(self):
        text = (
            "# Changelog\n\n## Unreleased\n\n"
            "- The engine is `atglyph`: `rl-engine` is `atglyph`.\n"
            "  To change: `rl_engine::rl_bevy::` becomes `atglyph::ecs::`.\n\n"
            "- `rl_rules::Registry` loads.\n\n"
            "## 0.3.0\n\n- `rl_rules::Registry` loaded.\n"
        )
        expected = text.replace("- `rl_rules::Registry` loads.", "- `atglyph_rules::Registry` loads.")
        self.assertEqual(rewrite_changelog(text), expected)
        self.assertEqual(rewrite_changelog(expected), expected)


if __name__ == "__main__":
    unittest.main()
