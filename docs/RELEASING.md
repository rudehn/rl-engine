# Releasing

A release is the steps below, in order.
A step marked **Nate** needs his account, his credentials or his judgment; an agent stops there and says so.

1. The documentation pass, as `AGENTS.md` describes it, over everything since the last tag.
2. Bump the workspace version, the internal dependency versions, the template's pin, the readme's dependency lines and the `--tag` in the readme, the guide and the facade's front page together; `scripts/check-template.sh` names any one left behind.
3. Run `python3 scripts/publish.py --semver` and `scripts/check-publish.sh` by hand.
4. **Nate**, the first release only: create the crates.io account, make an API token scoped to publishing new crates, and store it as the `CARGO_REGISTRY_TOKEN` repository secret.
5. **Nate**, at `0.4.0`, before the tag: rename the repository to `rudehn/atglyph` in GitHub's settings, then run `python3 scripts/rename-to-atglyph.py --urls` and commit.
   GitHub does not redirect the guide's address, so a tag cut before this step would put the old `rudehn.github.io/rl-engine` into `atglyph`'s docs.rs page, its crates.io readme and every game generated from `--tag v0.4.0`.
   Let Pages republish.
6. **Nate**, the first release only: add the GitHub workflow (`release.yml` in `github.com/rudehn/atglyph`) as each crate's trusted publisher on crates.io, then delete the secret.
7. **Nate**: push the tag.
   The workflow checks everything, runs the semver check, publishes every crate in dependency order and then writes the release page.
   A run that stops partway, on crates.io's new-crate rate limit or otherwise, is re-run from the Actions page and publishes what is left.
8. Check every crate's docs.rs build.
9. **Nate**: the listings and announcements.
   Bevy Assets, arewegameyet.rs, RogueBasin, r/roguelikedev Sharing Saturday, r/rust and This Week in Rust.
