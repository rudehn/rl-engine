#!/usr/bin/env bash
# The inventory cannot quietly fall behind the code.
#
# `docs/OVERVIEW.md` is the one page that says what the engine has, and the
# rule that it moves in the same commit as the system it gains was written
# under "rules the build enforces" while nothing in the build enforced it.
# This is the enforcement. It checks only what can be checked exactly, so
# there are no false failures to learn to ignore:
#
#   1. every `impl Plugin for X` in `crates/` is named in the overview,
#   2. every `docs/design/*.md` is named in `AGENTS.md`'s layout section,
#   3. every directory in `examples/` is named in the overview,
#   4. and named in the guide, whose last chapter is the table of them.
#
# Check 4 exists because Foundry, the newest example, reached its own
# design document and a fingerprint test without ever being mentioned in
# the guide.
#
# What it cannot check is whether the prose is still true. That stays a
# review matter, and the tables the overview now carries are there so the
# untrue sentence is the shorter thing to find.
set -euo pipefail

cd "$(dirname "$0")/.."

overview=docs/OVERVIEW.md
agents=AGENTS.md
failed=0

note() {
	printf '%s\n' "$1" >&2
	failed=1
}

# 1. Plugins. A plugin is the engine's unit of opt-in, so a plugin nobody
# can find by name is a subsystem a game cannot switch on.
while read -r plugin; do
	if ! grep -q "\b${plugin}\b" "$overview"; then
		note "$overview: no mention of \`${plugin}\`. A plugin the inventory does not name is one a game cannot find."
	fi
done < <(grep -rho 'impl Plugin for [A-Za-z0-9_]*' --include='*.rs' crates/ | sed 's/impl Plugin for //' | sort -u)

# 2. Design docs. The layout section lists them by name, and that list went
# stale at four of nine before this check existed.
for doc in docs/design/*.md; do
	name=$(basename "$doc" .md)
	if ! grep -q "\b${name}\b" "$agents"; then
		note "$agents: \`docs/design/${name}.md\` exists and the layout section does not list it."
	fi
done

# 3. Examples. Each one is a worked example the overview describes, and a
# game that is not described is a game nobody reads.
for dir in examples/*/; do
	name=$(basename "$dir")
	if ! grep -qi "\b${name}\b" "$overview"; then
		note "$overview: no mention of the \`examples/${name}\` game."
	fi
	if ! grep -rqi "\b${name}\b" docs/guide/src/; then
		note "docs/guide/src: no mention of the \`examples/${name}\` game. The guide's last chapter is the table of them."
	fi
done

if [ "$failed" -ne 0 ]; then
	printf '\n%s\n' "The inventory is behind the code. See docs/OVERVIEW.md and AGENTS.md." >&2
	exit 1
fi

echo "the inventory names every plugin, design doc and example"
