#!/usr/bin/env python3
"""Put the tutorial's code into the guide's chapters.

    scripts/expand-guide.py           rewrite every chapter from the sources
    scripts/expand-guide.py --check   fail if a chapter is out of date

The guide quotes the tutorial rather than restating it, so the code in a
chapter is code that compiles. mdBook can do that quoting itself, with a
`{{#include}}` directive it expands while it builds the book. The trouble
is that a book nobody builds is the only place the quoting works: GitHub,
an editor and anything else reading the markdown show the directive where
the code should be, and the chapter about a plugin then contains no plugin.

So the expansion happens here instead, and the result is committed. A
chapter holds real code in a real fence, with an HTML comment above it
naming where the code came from, which every markdown renderer hides. The
sources stay the single source of truth, and CI runs this with `--check`
so a chapter cannot drift from the code it quotes, the same way
`cargo fmt --check` keeps formatting honest.
"""

import pathlib
import re
import sys

GUIDE = pathlib.Path("docs/guide/src")

# A block this script wrote: the marker naming the source, then the fence.
EXPANDED = re.compile(r"<!-- include: (?P<spec>[^\n>]+?) -->\n```(?P<lang>[^\n]*)\n(?P<body>.*?)```", re.S)
# A block mdBook would have expanded, left over from before this existed.
DIRECTIVE = re.compile(r"```(?P<lang>[^\n]*)\n\{\{#include (?P<spec>[^}\n]+)\}\}\n```")


class Missing(Exception):
    """A chapter names a file or an anchor that is not there."""


def snippet(page: pathlib.Path, spec: str) -> str:
    """The lines `spec` names, as they are in the source.

    `spec` is a path, optionally followed by `:anchor`. An anchor is the
    lines between `ANCHOR: name` and `ANCHOR_END: name`. Every anchor
    comment is dropped, including any nested inside, so the reader sees
    code rather than the scaffolding that marked it.
    """
    target, _, anchor = spec.partition(":")
    path = page.parent / target
    if not path.is_file():
        raise Missing(f"{page}: names no such file: {target}")
    lines = path.read_text().splitlines()
    if anchor:
        try:
            first = next(i for i, l in enumerate(lines) if l.rstrip().endswith(f"ANCHOR: {anchor}"))
            last = next(i for i, l in enumerate(lines) if l.rstrip().endswith(f"ANCHOR_END: {anchor}"))
        except StopIteration:
            raise Missing(f"{page}: {target} has no anchor '{anchor}'") from None
        lines = lines[first + 1 : last]
    kept = [l for l in lines if "ANCHOR" not in l]
    return "\n".join(kept).rstrip() + "\n"


def expanded(page: pathlib.Path) -> str:
    """`page` with every quoted block filled in from its source."""
    text = page.read_text()

    def rewrite(match: re.Match[str]) -> str:
        spec = match.group("spec").strip()
        lang = match.group("lang")
        return f"<!-- include: {spec} -->\n```{lang}\n{snippet(page, spec)}```"

    text = DIRECTIVE.sub(rewrite, text)
    return EXPANDED.sub(rewrite, text)


def main() -> int:
    check = "--check" in sys.argv[1:]
    stale, broken = [], []
    for page in sorted(GUIDE.glob("*.md")):
        try:
            want = expanded(page)
        except Missing as e:
            broken.append(str(e))
            continue
        if want == page.read_text():
            continue
        stale.append(page)
        if not check:
            page.write_text(want)
    for e in broken:
        print(f"  {e}", file=sys.stderr)
    if broken:
        print("guide: the references above name nothing", file=sys.stderr)
        return 1
    if check and stale:
        for page in stale:
            print(f"  {page}: the code it quotes has moved on", file=sys.stderr)
        print("guide: run scripts/expand-guide.py and commit the result", file=sys.stderr)
        return 1
    if not check and stale:
        print(f"expanded {len(stale)} chapter(s) from the examples")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
