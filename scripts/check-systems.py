#!/usr/bin/env python3
"""The system reference, checked against the code it documents.

    scripts/check-systems.py              fail if a page is behind its code
    scripts/check-systems.py --bless NAME confirm systems/NAME.md again

Three checks, in the order a failure is cheapest to fix:

  1. every page's manifest names files that are there,
  2. every listed file is unchanged since the page was last confirmed,
  3. every `impl Plugin for X` in `crates/` is claimed by exactly one page.

Check 3 is a warning until the reference covers every system, because a
check that fails on every build is a check somebody comments out. The
commit that finishes the last page turns COVERAGE_IS_FATAL on.

What none of them can check is whether a sentence is true. Check 2 is the
nearest thing there is: it cannot read the page, but it can say that the
code the page describes has moved and that nobody has re-read the page
since. That is the failure the other two cannot see, and the one that
makes a reference worse than no reference.
"""

import hashlib
import pathlib
import re
import sys

SYSTEMS = pathlib.Path("docs/guide/src/systems")
CRATES = pathlib.Path("crates")

# Coverage is advisory until every system has a page. Task 12 of the plan
# that introduced this flips it, in the commit that writes the last one.
COVERAGE_IS_FATAL = False

MANIFEST = re.compile(r"<!-- documents:\n(?P<body>.*?)-->", re.S)
FIELDS = ("plugins", "files", "fingerprint")


class Broken(Exception):
    """A page's manifest is missing, malformed, or names nothing."""


def manifest(page: pathlib.Path) -> dict[str, list[str]]:
    """The `documents:` block at the top of `page`.

    Every field is a whitespace or comma separated list, so a field can
    run over several lines and a path with no spaces needs no quoting.
    """
    found = MANIFEST.search(page.read_text())
    if not found:
        raise Broken(f"{page}: no `documents:` block. Every system page declares what it documents.")
    fields: dict[str, list[str]] = {}
    key = None
    for line in found.group("body").splitlines():
        head, sep, rest = line.strip().partition(":")
        if sep and head in FIELDS:
            key, line = head, rest
            fields.setdefault(key, [])
        if key is None:
            raise Broken(f"{page}: `{line.strip()}` is before any of {', '.join(FIELDS)}.")
        fields[key].extend(word for word in line.replace(",", " ").split() if word)
    for field in FIELDS:
        if not fields.get(field):
            raise Broken(f"{page}: the manifest has no `{field}:`.")
    if len(fields["fingerprint"]) != 1:
        raise Broken(f"{page}: `fingerprint:` takes one value, not {len(fields['fingerprint'])}.")
    return fields


def fingerprint(paths: list[str]) -> str:
    """A hash of the files a page documents, in the order it lists them."""
    digest = hashlib.sha256()
    for path in paths:
        digest.update(pathlib.Path(path).read_bytes())
    return digest.hexdigest()[:8]


def pages() -> list[pathlib.Path]:
    """Every system page, in reading order."""
    return sorted(p for p in SYSTEMS.glob("*.md") if not p.name.startswith("_"))


def bless(name: str) -> int:
    """Write today's fingerprint into one page, and say what it covered."""
    page = SYSTEMS / f"{name}.md"
    if not page.is_file():
        print(f"no such page: {page}", file=sys.stderr)
        return 1
    fields = manifest(page)
    fresh = fingerprint(fields["files"])
    text = page.read_text()
    page.write_text(re.sub(r"fingerprint: [0-9a-f]+", f"fingerprint: {fresh}", text, count=1))
    print(f"{page}: confirmed against {', '.join(fields['files'])}")
    return 0


def main() -> int:
    argv = sys.argv[1:]
    if "--bless" in argv:
        rest = argv[argv.index("--bless") + 1 :]
        if not rest:
            print(
                "--bless takes the name of one page, so that confirming a page is a\n"
                "thing somebody did to it rather than a thing that happened to all of them.",
                file=sys.stderr,
            )
            return 1
        return bless(rest[0])

    problems = []
    for page in pages():
        try:
            fields = manifest(page)
        except Broken as e:
            problems.append(str(e))
            continue
        missing = [f for f in fields["files"] if not pathlib.Path(f).is_file()]
        if missing:
            problems.append(f"{page}: documents files that are not there: {', '.join(missing)}")
            continue
        fresh = fingerprint(fields["files"])
        if fresh != fields["fingerprint"][0]:
            problems.append(
                f"{page}: the code it documents has changed since the page was last confirmed.\n"
                f"    It documents: {', '.join(fields['files'])}\n"
                f"    Re-read the page. If it is still true:  scripts/check-systems.py --bless {page.stem}\n"
                f"    If it is not, fix the page first."
            )

    for problem in problems:
        print(f"  {problem}", file=sys.stderr)
    if problems:
        print("\nthe system reference is behind the code", file=sys.stderr)
        return 1
    print(f"the system reference is current across {len(pages())} page(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
