#!/usr/bin/env bash
# The starter template, generated and built the way a new game would be.
#
#   scripts/check-template.sh   renders templates/starter into a scratch
#                               directory with its placeholders filled and
#                               the engine taken from this checkout rather
#                               than from git, then checks formatting,
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

# The template pins the engine to a release tag, and a tag is the workspace
# version with a v in front. Bumping the version without the template, or
# the template without the version, is a template that generates games
# against an engine it was not written for.
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
if ! grep -q "tag = \"v$version\"" templates/starter/Cargo.toml.liquid; then
  echo "error: templates/starter pins a tag other than v$version, the workspace version" >&2
  exit 1
fi

out="${TMPDIR:-/tmp}/rl-engine-starter-check/my-game"
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
sed -e "s|^rl-engine = .*|rl-engine = { path = \"$root/crates/rl-engine\" }|" Cargo.toml >Cargo.toml.filled
mv Cargo.toml.filled Cargo.toml
if grep -rn '{{' --include='*.rs' --include='*.toml' --include='*.md' .; then
  echo "error: a placeholder was left unfilled" >&2
  exit 1
fi

# The workspace's own target directory: the lockfile and the profile match,
# so the game reuses every dependency the workspace already built rather
# than compiling a second Bevy beside it, which was fifteen gigabytes. The
# engine's own crates build at the game's optimisation level, so the next
# workspace build recompiles those few, and nothing else.
export CARGO_TARGET_DIR="$root/target"
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
echo "ok: the starter template generates, builds and passes its tests"
