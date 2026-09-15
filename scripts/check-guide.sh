#!/usr/bin/env bash
# The guide's links into the code, checked without building the book.
#
#   scripts/check-guide.sh   every {{#include}} names a file that exists and
#                            an anchor that is in it, every image resolves,
#                            every hand-written snippet line is in an
#                            example, and every chapter is in SUMMARY.md
#
# The guide quotes the tutorial crate rather than restating it, so a renamed
# anchor or a moved file would leave a chapter silently showing an error
# where its code should be. mdBook reports that and still exits zero, so it
# cannot be the gate; this is.
set -euo pipefail
cd "$(dirname "$0")/.."

src=docs/guide/src
fail=0

note() {
  echo "  $1" >&2
  fail=1
}

# Every {{#include path}} and {{#include path:anchor}} in every chapter.
while IFS= read -r line; do
  page=${line%%:*}
  spec=${line#*:}
  target=${spec%%:*}
  anchor=""
  [[ $spec == *:* ]] && anchor=${spec#*:}
  path="$(dirname "$page")/$target"
  if [[ ! -f $path ]]; then
    note "$page: include names no such file: $target"
    continue
  fi
  if [[ -n $anchor ]] && ! grep -q "ANCHOR: $anchor\$" "$path"; then
    note "$page: $target has no anchor '$anchor'"
  fi
done < <(grep -rnoE '\{\{#include [^}]+\}\}' "$src" --include='*.md' |
  sed -E 's/:[0-9]+:\{\{#include /:/; s/\}\}$//')

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
