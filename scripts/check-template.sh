#!/usr/bin/env bash
# The starter template, generated and built the way a new game would be.
#
#   scripts/check-template.sh   renders templates/starter into a scratch
#                               directory with its placeholders filled and
#                               the engine taken from this checkout rather
#                               than from crates.io, then checks formatting,
#                               clippy and the template's own tests
#
# cargo-generate is not needed: the template's only placeholders are
# {{project-name}} and {{crate_name}}, and they are filled here, so CI
# proves the template builds against the engine it ships beside.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)

# A game that depends on the engine from git makes Cargo read every
# Cargo.toml in this repository, and one holding a placeholder is an error
# printed in every build of every game. The template's manifest is
# Cargo.toml.liquid, which cargo-generate renames as it renders.
if find templates -name Cargo.toml | grep .; then
  echo "error: a template manifest must be named Cargo.toml.liquid, or Cargo reads it from every game's git dependency" >&2
  exit 1
fi

# The template pins the engine to the release it was written against, the
# workspace version. Bumping the version without the template, or the
# template without the version, is a template that generates games against
# an engine it was not written for.
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
if ! grep -qx "atglyph = \"$version\"" templates/starter/Cargo.toml.liquid; then
  echo "error: templates/starter pins an atglyph other than $version, the workspace version" >&2
  exit 1
fi

# And the Bevy the engine is built on. A game on another Bevy builds two,
# whose types do not match, and every engine call fails to compile with a
# type mismatch far from its cause.
bevy_in() {
  sed -n "/^\[$1\]/,/^\[/s/^bevy = .*version = \"\([^\"]*\)\".*/\1/p" "$2"
}
engine_bevy=$(bevy_in workspace.dependencies Cargo.toml)
template_bevy=$(bevy_in dependencies templates/starter/Cargo.toml.liquid)
if [[ -z "$engine_bevy" || "$template_bevy" != "$engine_bevy" ]]; then
  echo "error: templates/starter pins bevy ${template_bevy:-nothing}, but the engine is on bevy ${engine_bevy:-nothing}" >&2
  exit 1
fi

# The template builds its dependencies as the workspace does. The workspace
# dropped their debug info on 2026-09-28 and the template went on keeping
# it for a week, so every generated game built Bevy with debug info, the
# bulk of a build on disk, and this check, which builds into the
# workspace's own target directory, built that heavier Bevy beside the
# workspace's.
dependency_profile() {
  sed -n '/^\[profile\.dev\.package\."\*"\]/,/^\[/p' "$1" | grep -E '^(opt-level|debug) ='
}
if [[ "$(dependency_profile Cargo.toml)" != "$(dependency_profile templates/starter/Cargo.toml.liquid)" ]]; then
  echo "error: templates/starter builds its dependencies under another [profile.dev.package.\"*\"] than the workspace's" >&2
  exit 1
fi

# Every instruction to generate a game names that release too. Without
# --tag, cargo-generate takes the template from main, which may already use
# what the release the template pins does not have.
if grep -n "cargo generate --git" README.md docs/guide/src/*.md | grep -v -- "--tag v$version"; then
  echo "error: the lines above generate a game without --tag v$version" >&2
  exit 1
fi

# And every dependency line the readme offers a project of its own names it.
if grep -nE '^atglyph(-[a-z]+)? = ' README.md | grep -v "= \"$version\"\$"; then
  echo "error: the readme lines above depend on an atglyph other than $version" >&2
  exit 1
fi

out="${TMPDIR:-/tmp}/atglyph-starter-check/my-game"
rm -rf "$out"
mkdir -p "$(dirname "$out")"
cp -R templates/starter "$out"
rm "$out/cargo-generate.toml"
mv "$out/Cargo.toml.liquid" "$out/Cargo.toml"
# The workspace's lockfile, so the generated game resolves the same Bevy
# the engine was tested with; cargo prunes what it does not use.
cp Cargo.lock "$out/"

cd "$out"
find . -type f \( -name '*.rs' -o -name '*.toml' -o -name '*.md' \) | while read -r f; do
  sed -e 's/{{project-name}}/my-game/g' -e 's/{{crate_name}}/my_game/g' "$f" >"$f.filled"
  mv "$f.filled" "$f"
done
sed -e "s|^atglyph = .*|atglyph = { path = \"$root/crates/atglyph\" }|" Cargo.toml >Cargo.toml.filled
mv Cargo.toml.filled Cargo.toml
if grep -rn '{{' --include='*.rs' --include='*.toml' --include='*.md' .; then
  echo "error: a placeholder was left unfilled" >&2
  exit 1
fi

# The workspace's own target directory, so a run after the first reuses what
# the first built, where the game's own, inside the scratch copy deleted
# above, would build everything again every run. It shares less with the
# workspace's own build than the lockfile suggests: the template keeps
# Bevy's default features, which the workspace turns off, so the game
# builds a Bevy of its own beside the workspace's, and the profile check
# above keeps that one at least without debug info.
export CARGO_TARGET_DIR="$root/target"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
echo "ok: the starter template generates, builds and passes its tests"
