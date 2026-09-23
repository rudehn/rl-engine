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
# An unbalanced fence or an unterminated manifest would leave `prose` reading
# the rest of the page as quoted code, which is to say checking nothing, so
# the shape of the page is settled before anything is read out of it.
[ $(( $(grep -c '^```' "$page") % 2 )) -eq 0 ] \
	|| note "$page: an odd number of fences. An unclosed one hides everything after it from these checks."
# The em dash is banned everywhere, including inside a quoted anchor: what a
# page holds is what the book publishes, and a doc comment in an example
# reaches the reader the same way a sentence here does.
! grep -q '—' "$page" || note "$page: an em dash. The house uses a plain dash."
# The word ban is a rule about the page's own prose, so it is checked over
# prose: the quoted code and the two comments are not the page's words. A page
# that quotes an example whose crate is named for a banned word fails on the
# include's path otherwise, and a snippet is free to hold whatever the example
# holds.
prose() {
	awk '/^```/ { fence = !fence; next } /^<!-- documents:/ { manifest = 1 } manifest { if (/-->/) manifest = 0; next } !fence && $0 !~ /^<!-- include:/' "$page"
}
! prose | grep -qiE '\b(powerful|simply|just|robust|seamless|leverage|delve|comprehensive|elegant|straightforward)\b' \
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
