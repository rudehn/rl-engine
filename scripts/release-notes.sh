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

printf '%s\n' "$section"
cat <<EOF

## Start a game on this release

\`\`\`sh
cargo install cargo-generate
cargo generate --git https://github.com/rudehn/rl-engine --tag v$version templates/starter --name my-game
\`\`\`

Or depend on it directly:

\`\`\`toml
rl-engine = { git = "https://github.com/rudehn/rl-engine", tag = "v$version" }
\`\`\`
EOF
