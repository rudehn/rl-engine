# Releasing

A release is the steps below, in order.
A step marked **Nate** needs his account, his credentials or his judgment; an agent stops there and says so.

A release is on GitHub only: a tag, its release page and the guide.
A game depends on the engine from git, pinned to the tag, as `atglyph = { git = "https://github.com/rudehn/rl-engine", tag = "v0.4.0" }`, and the starter template does the same.
The crates go to crates.io only once the repository variable `PUBLISH_CRATES` is `true`; until then `release.yml` skips its `semver` and `publish` jobs, and the steps marked **crates.io** below wait for the release that turns it on.

1. The documentation pass, as `AGENTS.md` describes it, over everything since the last tag.
2. Bump the workspace version, the internal dependency versions, the template's tag pin, the dependency lines' tag in the readme and in the Bevy-free crates' readmes and front pages, and the `--tag` in the readme, the guide and the facade's front page together; `scripts/check-template.sh` names any one left behind.
3. **crates.io**: run `python3 scripts/publish.py --semver` and `scripts/check-publish.sh` by hand.
4. **Nate**, **crates.io**, the first publishing release only: create the crates.io account, make an API token scoped to publishing new crates, store it as the `CARGO_REGISTRY_TOKEN` repository secret, and set the repository variable `PUBLISH_CRATES` to `true`.
   At the same release, point the template's pin, the dependency lines of step 2 and `scripts/check-template.sh` back at a version, `atglyph = "<version>"`, and the crates' readmes at crates.io and docs.rs.
5. **Nate**, optional and later: rename the repository to `rudehn/atglyph` in GitHub's settings, then rewrite the addresses and commit, before the next tag:
   `git grep -lE 'rudehn/rl-engine|rudehn\.github\.io/rl-engine|cd rl-engine' | xargs perl -pi -e 's{github\.com/rudehn/rl-engine}{github.com/rudehn/atglyph}g; s{rudehn\.github\.io/rl-engine}{rudehn.github.io/atglyph}g; s{\bcd rl-engine\b}{cd atglyph}g'`.
   GitHub does not redirect the guide's address, so a tag cut before the commit would put the old `rudehn.github.io/rl-engine` into every game generated from that tag and, with crates.io on, into the crates' pages.
   Let Pages republish.
6. **Nate**, **crates.io**, the first publishing release only: add the GitHub workflow (`release.yml` in the repository) as each crate's trusted publisher on crates.io, then delete the secret.
7. **Nate**: push the tag.
   The workflow checks everything, writes the release page from the changelog and publishes the guide.
   With `PUBLISH_CRATES` on it first runs the semver check and publishes every crate in dependency order, and writes the page only once they are up.
   A run that stops partway, on crates.io's new-crate rate limit or otherwise, is re-run from the Actions page and publishes what is left.
8. **crates.io**: check every crate's docs.rs build.
9. **Nate**: the listings and announcements.
   Bevy Assets, arewegameyet.rs, RogueBasin, r/roguelikedev Sharing Saturday, r/rust and This Week in Rust.
