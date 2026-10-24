#!/usr/bin/env bash
# The notes for a release page, from the changelog.
#
#   scripts/release-notes.sh 0.1.0   prints CHANGELOG.md's section for 0.1.0
#                                    and how to start a game on that release
#
# The release workflow publishes what this prints when a tag is pushed, so
# the page and the changelog are one text and cannot disagree.
set -euo pipefail
cd "$(dirname "$0")/.."
version=${1:?usage: scripts/release-notes.sh VERSION}

section=$(awk -v heading="## $version" '$0 == heading { found = 1; next } /^## / { found = 0 } found' CHANGELOG.md | sed '/./,$!d')
if [ -z "$section" ]; then
  echo "error: CHANGELOG.md has no section headed \"## $version\"" >&2
  exit 1
fi

# The Bevy the engine is built on, which a game lists beside it at the
# same version, read from the workspace as the version is.
bevy=$(sed -n '/^\[workspace.dependencies\]/,/^\[/{s/^bevy = "\([^"]*\)".*/\1/p;s/^bevy = {.*version = "\([^"]*\)".*/\1/p;}' Cargo.toml)
if [ -z "$bevy" ]; then
  echo "error: Cargo.toml's [workspace.dependencies] names no bevy version" >&2
  exit 1
fi

# The repository a game depends on the engine from by tag, since the engine
# is not on crates.io.
repository=$(sed -n '/^\[workspace.package\]/,/^\[/s/^repository = "\(.*\)"/\1/p' Cargo.toml)
if [ -z "$repository" ]; then
  echo "error: Cargo.toml's [workspace.package] names no repository" >&2
  exit 1
fi

# GitHub refuses a release body past 125,000 characters, and a release
# with hundreds of entries passes it (0.4.0's was 280,000). Such a page
# keeps the section's summary, everything above "Every change, newest
# first:", and links the full list in the changelog at the tag, which
# stays the one text.
limit=100000
if [ "${#section}" -gt "$limit" ]; then
  summary=$(awk '/^Every change, newest first:/ { exit } { print }' <<<"$section")
  if [ "${#summary}" -eq "${#section}" ] || [ "${#summary}" -gt "$limit" ]; then
    echo "error: the $version section is past $limit characters and has no short summary above \"Every change, newest first:\"" >&2
    exit 1
  fi
  anchor=$(printf '%s' "$version" | tr -d '.')
  section="$summary

Every change, with what a game on the last release changes for it, is in [CHANGELOG.md]($repository/blob/v$version/CHANGELOG.md#$anchor)."
fi

printf '%s\n' "$section"
cat <<EOF

## Start a game on this release

\`\`\`sh
cargo install cargo-generate
cargo generate --git $repository --tag v$version templates/starter --name my-game
\`\`\`

Or depend on it directly:

\`\`\`toml
atglyph = { git = "$repository", tag = "v$version" }
bevy = "$bevy"
\`\`\`
EOF
