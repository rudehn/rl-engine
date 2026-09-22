#!/usr/bin/env bash
# The mechanical half of a system page's review.
#
#   scripts/check-systems-style.sh docs/guide/src/systems/sight.md
#
# What it cannot check is whether the page is any good. That is the list
# in AGENTS.md, and it is read by a person.
set -euo pipefail
page=${1:?usage: scripts/check-systems-style.sh <page>}
failed=0

note() {
	printf '  %s\n' "$1" >&2
	failed=1
}

lines=$(wc -l < "$page")
[ "$lines" -le 125 ] || note "$page: $lines lines. A page past 120 is a system that wants splitting."
! grep -q '—' "$page" || note "$page: an em dash. The house uses a plain dash."
! grep -qiE '\b(powerful|simply|just|robust|seamless|leverage|delve|comprehensive|elegant|straightforward)\b' "$page" \
	|| note "$page: a word from the ban list. Say what it does instead."
# A page carries two comments and no others: the manifest `check-systems.py`
# reads, and one `include:` marker per snippet. Anything else is a line of the
# skeleton that was written around instead of answered. Matching `<!-- [a-z]`
# instead would flag both of the legitimate ones.
! grep '<!--' "$page" | grep -vE '<!-- (documents:|include:)' > /dev/null \
	|| note "$page: a comment that is neither the manifest nor an include. The skeleton's instructions do not ship."
[ "$(grep -c '^## ' "$page")" -eq 5 ] \
	|| note "$page: $(grep -c '^## ' "$page") sections, not 5. The template is fixed: Turning it on, The model, Using it, The line, Where it lives."

if [ "$failed" -ne 0 ]; then
	exit 1
fi
echo "$page: style ok. The prose is still a review matter."
