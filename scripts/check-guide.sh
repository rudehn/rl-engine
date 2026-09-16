#!/usr/bin/env bash
# The guide's links into the code, checked without building the book.
#
#   scripts/check-guide.sh   every chapter holds the code it quotes, every
#                            image resolves, every hand-written snippet line
#                            is in an example, every chapter a chapter or a
#                            source file points at is there, and every
#                            chapter is in SUMMARY.md
#
# The guide quotes the tutorial crate rather than restating it, so the code
# in a chapter is code that compiles. scripts/expand-guide.py puts it there
# and this runs it in --check mode, so a renamed anchor or a moved file
# fails the build rather than leaving a chapter quoting something that is
# no longer true.
set -euo pipefail
cd "$(dirname "$0")/.."

src=docs/guide/src
fail=0

note() {
  echo "  $1" >&2
  fail=1
}

# Every chapter holds the code it quotes, expanded from the source.
if ! python3 scripts/expand-guide.py --check; then
  fail=1
fi

# Every image a chapter points at.
while IFS= read -r hit; do
  page=${hit%%:*}
  image=${hit#*:}
  [[ -f "$(dirname "$page")/$image" ]] || note "$page: no such image: $image"
done < <(grep -rnoE '\]\(([^)]+\.png)\)' "$src" --include='*.md' | sed -E 's/:[0-9]+:\]\(/:/; s/\)$//')

# Every chapter is reachable from the table of contents, and every entry in
# the table of contents is a chapter.
listed=$(grep -oE '\(([0-9a-z-]+\.md)\)' "$src/SUMMARY.md" | tr -d '()' | sort -u)
present=$(cd "$src" && ls ./*.md | sed 's|^\./||' | grep -v '^SUMMARY.md$' | sort -u)
for page in $present; do
  grep -qx "$page" <<<"$listed" || note "SUMMARY.md does not list $page"
done
for page in $listed; do
  [[ -f "$src/$page" ]] || note "SUMMARY.md lists a missing page: $page"
done

# Every chapter a source file, a script or a doc points at by path. A
# renamed chapter used to leave a module comment aimed at nothing, which
# nothing else here would catch: the guide's own checks only look inside
# the guide.
while IFS= read -r hit; do
  file=${hit%%:*}
  rest=${hit#*:}
  page=${rest#*:}
  page=${page##*/}
  [[ -f "$src/$page" ]] || note "$file: names no such chapter: $page"
done < <(grep -rnoE 'docs/guide/src/[0-9a-z-]+\.md' crates examples templates scripts docs/*.md README.md AGENTS.md --include='*.rs' --include='*.md' --include='*.sh' 2>/dev/null)

# Every link from one chapter to another.
while IFS= read -r hit; do
  page=${hit%%:*}
  target=${hit#*:}
  [[ -f "$src/$target" ]] || note "$page: links to a missing chapter: $target"
done < <(grep -rnoE '\]\(([0-9a-z-]+\.md)\)' "$src" --include='*.md' | sed -E 's/:[0-9]+:\]\(/:/; s/\)$//')

# Every line of a Rust snippet a chapter writes out by hand rather than
# includes is a line of an example's source. A copied line is the one kind
# of reference mdBook cannot check, so it is the kind that drifted: a chapter
# kept an API for a week after the code had moved on. A line holding `...`
# elides on purpose and is exempt.
while IFS= read -r stale; do
  note "$stale"
done < <(python3 - "$src" <<'PY'
import pathlib, re, sys
lines = {l.strip() for f in pathlib.Path("examples").rglob("*.rs") for l in f.read_text().splitlines()}
for page in sorted(pathlib.Path(sys.argv[1]).glob("*.md")):
    for block in re.finditer(r"```rust[^\n]*\n(.*?)```", page.read_text(), re.S):
        if "{{#include" in block.group(1):
            continue
        for line in block.group(1).splitlines():
            t = line.strip()
            if t and "..." not in t and t not in lines:
                print(f"{page}: a snippet line no example has: {t}")
PY
)

if ((fail)); then
  echo "guide: broken references above" >&2
  exit 1
fi
echo "ok: the guide's includes, images, snippets and table of contents all resolve"
