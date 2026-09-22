# The system reference implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a reference page for every engine system in the existing mdBook, and add the checks that keep those pages true as the code moves.

**Architecture:** Twenty-three pages under `docs/guide/src/systems/`, entered as seven parts in the existing `SUMMARY.md`, ordered so a reader front-to-back never meets a term before the page that defines it. Three enforcement layers: coverage, so every `impl Plugin for X` is claimed by a page; includes, so a page quotes real code through `expand-guide.py` and a moved anchor fails the build; and a fingerprint tripwire, so a page declares the files it documents and CI asks for a re-read when one of them changes. Two existing scripts assume a flat `src/` and must be taught to descend first, because until they are, a nested page passes unchecked, which is worse than failing.

**Tech Stack:** mdBook 0.5.4, Python 3 for `scripts/expand-guide.py` and the new `scripts/check-systems.py`, bash for `scripts/check-guide.sh`, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-22-system-reference-design.md`

## Global Constraints

Copied from the spec and from `AGENTS.md`. Every task's requirements include this section.

- Plain dash, never an em dash.
- One sentence per line in markdown source. Preserve normal markdown structure; do not wrap two sentences onto one physical line.
- American spelling in identifiers.
- No `TODO` comments in source. Outstanding work goes in `docs/TODO.md`.
- Banned in every page, enforced in review: powerful, simply, just, robust, seamless, leverage, delve, comprehensive, elegant, straightforward.
- No sentence beginning "In this section we will". No paragraph that restates its own heading.
- A page is capped at about 120 lines. Past that, the system wants splitting into two pages.
- A page carries no invented code. Every code block is an `expand-guide.py` include naming a real anchor in `examples/`.
- Commit messages carry no co-author trailer.
- `scripts/check-guide.sh` indents with two spaces. `scripts/check-overview.sh` indents with tabs. Match the file being edited.
- After every task, the full documentation suite passes: `scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py`.

## The two halves

Tasks 1 to 8 are machinery: script changes, a new check, the book's skeleton, and one pilot page that the template is judged on. They are ordinary TDD work with a red and a green.

Tasks 9 to 12 are prose. A page has no unit test, so its gate is `check-systems.py` passing plus the acceptance checklist in Task 8. Each page is its own commit.

---

### Task 1: `expand-guide.py` descends into subdirectories

**Files:**
- Modify: `scripts/expand-guide.py:78`
- Fixture, created and deleted inside this task: `docs/guide/src/systems/_probe.md`

**Interfaces:**
- Consumes: nothing.
- Produces: `expand-guide.py` expands and checks every `*.md` under `docs/guide/src/` at any depth. Every later task that writes a page with an include depends on this.

- [ ] **Step 1: Write the failing case**

The probe names an anchor that does not exist. A script that reads the page must reject it.

````bash
mkdir -p docs/guide/src/systems
cat > docs/guide/src/systems/_probe.md <<'EOF'
# Probe

```rust
{{#include ../../../../examples/tutorial/src/bin/step02_light.rs:no_such_anchor}}
```
EOF
````

- [ ] **Step 2: Run the check and watch it pass, which is the bug**

Run: `python3 scripts/expand-guide.py --check; echo "exit $?"`

Expected: `exit 0`. The probe is never read. `GUIDE.glob("*.md")` on line 78 does not descend, so a page in `systems/` is invisible to both the expander and the checker.

- [ ] **Step 3: Make it descend**

In `scripts/expand-guide.py`, line 78:

```python
    for page in sorted(GUIDE.rglob("*.md")):
```

`snippet()` resolves a spec against `page.parent`, so a page one directory deeper reaches the crates with one more `../` and needs no other change.

- [ ] **Step 4: Run the check and watch it fail**

Run: `python3 scripts/expand-guide.py --check; echo "exit $?"`

Expected: `exit 1`, with this on stderr:

```
  docs/guide/src/systems/_probe.md: examples/tutorial/src/bin/step02_light.rs has no anchor 'no_such_anchor'
guide: the references above name nothing
```

- [ ] **Step 5: Point the probe at a real anchor and confirm it expands**

```bash
sed -i '' 's/no_such_anchor/lantern/' docs/guide/src/systems/_probe.md
python3 scripts/expand-guide.py
grep -q 'const LANTERN' docs/guide/src/systems/_probe.md && echo "expanded in place"
```

Expected: `expanded in place`. The `lantern` anchor is at `examples/tutorial/src/bin/step02_light.rs:161` and holds `const LANTERN: LightSource`.

- [ ] **Step 6: Delete the probe and confirm the tree is clean**

```bash
rm docs/guide/src/systems/_probe.md
rmdir docs/guide/src/systems
python3 scripts/expand-guide.py --check; echo "exit $?"
```

Expected: `exit 0`.

- [ ] **Step 7: Commit**

```bash
git add scripts/expand-guide.py
git commit -m "guide: expand and check pages at any depth under src

A page in a subdirectory was never read, so an include in one naming a
missing anchor passed the check. The system reference puts its pages in
src/systems, so the flat glob had to go before the first of them landed."
```

---

### Task 2: `check-guide.sh` descends

**Files:**
- Modify: `scripts/check-guide.sh`, four places named in the steps below
- Fixture, created and deleted inside this task: `docs/guide/src/systems/_probe.md`

**Interfaces:**
- Consumes: Task 1's `rglob`, so the expander and this checker agree on which files are pages.
- Produces: `check-guide.sh` sees nested pages. Its table-of-contents check, its cross-chapter link check and its "a source file names a chapter" check all resolve paths with a directory component.

- [ ] **Step 1: Write the failing case**

A page that exists and is not in `SUMMARY.md` must be reported.

````bash
mkdir -p docs/guide/src/systems
cat > docs/guide/src/systems/_probe.md <<'EOF'
# Probe

A page nothing lists.
EOF
````

- [ ] **Step 2: Run the check and watch it pass, which is the bug**

Run: `scripts/check-guide.sh; echo "exit $?"`

Expected: `exit 0` and `ok: the guide's includes, images, snippets and table of contents all resolve`. The page is unlisted and unreported, because `present` is built with `ls ./*.md`.

- [ ] **Step 3: Teach the table-of-contents check to descend**

Replace the two lines that build `listed` and `present`:

```bash
listed=$(grep -oE '\(([0-9a-z/-]+\.md)\)' "$src/SUMMARY.md" | tr -d '()' | sort -u)
present=$(cd "$src" && find . -name '*.md' | sed 's|^\./||' | grep -v '^SUMMARY.md$' | sort -u)
```

The regex gains `/` so a `SUMMARY.md` entry of `systems/sight.md` is seen as listed rather than skipped.

- [ ] **Step 4: Run the check and watch it fail**

Run: `scripts/check-guide.sh; echo "exit $?"`

Expected: `exit 1`, with `SUMMARY.md does not list systems/_probe.md`.

- [ ] **Step 5: Teach the cross-chapter link check to resolve relative to the page**

A link from `systems/sight.md` to `../02-sight-and-light.md` resolves against the page's own directory, not against `$src`. Replace the cross-chapter link block:

```bash
# Every link from one chapter to another, resolved from the page that
# holds it: a page in systems/ reaches a chapter with ../ and a sibling
# with neither.
while IFS= read -r hit; do
  page=${hit%%:*}
  target=${hit#*:}
  [[ -f "$(dirname "$page")/$target" ]] || note "$page: links to a missing chapter: $target"
done < <(grep -rnoE '\]\(([0-9a-z/.-]+\.md)\)' "$src" --include='*.md' | sed -E 's/:[0-9]+:\]\(/:/; s/\)$//')
```

`page` comes from `grep -r` and is a path from the repository root, so `dirname` is the page's directory.

- [ ] **Step 6: Teach the "a source file names a chapter" check to keep the subdirectory**

That block takes a basename, which throws away `systems/`. Replace it:

```bash
# Every chapter a source file, a script or a doc points at by path. A
# renamed chapter used to leave a module comment aimed at nothing, which
# nothing else here would catch: the guide's own checks only look inside
# the guide.
while IFS= read -r hit; do
  file=${hit%%:*}
  rest=${hit#*:}
  page=${rest#*docs/guide/src/}
  [[ -f "$src/$page" ]] || note "$file: names no such chapter: $page"
done < <(grep -rnoE 'docs/guide/src/[0-9a-z/-]+\.md' crates examples templates scripts docs/*.md README.md AGENTS.md --include='*.rs' --include='*.md' --include='*.sh' 2>/dev/null)
```

- [ ] **Step 7: List the probe, and confirm the whole script passes**

```bash
printf -- '- [Probe](systems/_probe.md)\n' >> docs/guide/src/SUMMARY.md
scripts/check-guide.sh; echo "exit $?"
```

Expected: `exit 0`. This proves Step 3's regex accepts a path with a directory in it, which the failing case alone did not.

- [ ] **Step 8: Remove the probe and its entry, confirm clean**

```bash
rm docs/guide/src/systems/_probe.md
rmdir docs/guide/src/systems
sed -i '' '/systems\/_probe.md/d' docs/guide/src/SUMMARY.md
scripts/check-guide.sh; echo "exit $?"
```

Expected: `exit 0`, and `git diff --stat docs/guide/src/SUMMARY.md` shows nothing.

- [ ] **Step 9: Commit**

```bash
git add scripts/check-guide.sh
git commit -m "guide: check pages in subdirectories

Three of the script's checks assumed every page sat directly in src: the
contents check listed only *.md at the top, the link check resolved a
target against src rather than against the page, and the check that a
source file names a real chapter took a basename. A nested page passed
all three without being looked at, which is worse than failing."
```

---

### Task 3: `check-systems.py`, the manifest and the fingerprint

**Files:**
- Create: `scripts/check-systems.py`
- Create: `docs/guide/src/systems/` (holds the pages from Task 7 onward)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `scripts/check-systems.py` with exit 0 clean, exit 1 dirty.
  - `--bless NAME` rewrites the fingerprint in `docs/guide/src/systems/NAME.md`.
  - The manifest format every page from Task 7 onward must carry, parsed by `manifest(page) -> dict[str, list[str]]` with keys `plugins`, `files`, `fingerprint`.
  - `fingerprint(paths) -> str`, eight hex characters.
  - Module constant `COVERAGE_IS_FATAL: bool`, consumed by Task 4 and flipped by Task 12.

Python rather than bash, unlike `check-overview.sh`: this one hashes files and parses a multi-line block, and both are fragile in shell. `expand-guide.py` is the precedent and the style to match.

- [ ] **Step 1: Write the failing case**

There is no script, and a page to check.

```bash
mkdir -p docs/guide/src/systems
cat > docs/guide/src/systems/probe.md <<'EOF'
<!-- documents:
     plugins: LightingPlugin
     files: crates/rl-bevy/src/lighting.rs
     fingerprint: 00000000 -->

# Probe
EOF
```

- [ ] **Step 2: Run it and watch it not exist**

Run: `python3 scripts/check-systems.py; echo "exit $?"`

Expected: `exit 2`, `can't open file ... check-systems.py`.

- [ ] **Step 3: Write the script**

```python
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
```

- [ ] **Step 4: Run it and watch the fingerprint fail**

```bash
chmod +x scripts/check-systems.py
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: `exit 1`, naming `probe.md`, `crates/rl-bevy/src/lighting.rs`, and the bless command. The probe's fingerprint is `00000000` and the real hash is not.

`pages()` skips a name starting with `_`, which is why the probe here is `probe.md` and not `_probe.md` as in Tasks 1 and 2.

- [ ] **Step 5: Bless it and watch it pass**

```bash
python3 scripts/check-systems.py --bless probe
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: the bless prints `docs/guide/src/systems/probe.md: confirmed against crates/rl-bevy/src/lighting.rs`, then `exit 0`.

- [ ] **Step 6: Prove the tripwire trips on a real edit**

```bash
printf '\n// a line that changes the file\n' >> crates/rl-bevy/src/lighting.rs
python3 scripts/check-systems.py; echo "exit $?"
git checkout crates/rl-bevy/src/lighting.rs
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: `exit 1` then `exit 0`. This is the layer's whole purpose and it is the one step not to skip.

- [ ] **Step 7: Prove `--bless` with no name refuses**

Run: `python3 scripts/check-systems.py --bless; echo "exit $?"`

Expected: `exit 1` and the sentence about confirming being something somebody did to a page.

- [ ] **Step 8: Remove the probe, commit**

```bash
rm docs/guide/src/systems/probe.md
git add scripts/check-systems.py
git commit -m "docs: a tripwire for the system reference

A page declares the files it documents and a hash of them. When one of
those files changes the check fails, names the page and asks for a
re-read, because a reference whose prose has quietly stopped being true
is worse than no reference. Coverage lands next and stays advisory until
every system has a page."
```

---

### Task 4: the coverage layer, advisory

**Files:**
- Modify: `scripts/check-systems.py`, adding `plugins_in_code()` and the coverage block in `main()`

**Interfaces:**
- Consumes: `manifest()` and `COVERAGE_IS_FATAL` from Task 3.
- Produces: `plugins_in_code() -> set[str]`. Coverage complaints are printed on stderr and counted toward the exit code only when `COVERAGE_IS_FATAL` is true.

- [ ] **Step 1: Write the failing case**

With no pages at all, every one of the 54 plugins is unclaimed, and the script currently says nothing about it.

Run: `python3 scripts/check-systems.py; echo "exit $?"`

Expected: `exit 0` and `the system reference is current across 0 page(s)`. Nothing mentions that no plugin is documented.

- [ ] **Step 2: Add the plugin scan**

After `pages()` in `scripts/check-systems.py`:

```python
PLUGIN = re.compile(r"impl Plugin for ([A-Za-z0-9_]+)")


def plugins_in_code() -> set[str]:
    """Every plugin the workspace defines.

    A plugin is the engine's unit of opt-in, so a plugin no page claims is
    a subsystem a game can switch on and cannot read about.
    """
    found: set[str] = set()
    for source in CRATES.rglob("*.rs"):
        found.update(PLUGIN.findall(source.read_text()))
    return found
```

- [ ] **Step 3: Add the coverage block**

In `main()`, after the per-page loop and before the `for problem in problems:` loop:

```python
    claimed: dict[str, list[pathlib.Path]] = {}
    for page in pages():
        try:
            fields = manifest(page)
        except Broken:
            continue  # already reported above
        for plugin in fields["plugins"]:
            claimed.setdefault(plugin, []).append(page)

    coverage = []
    for plugin in sorted(plugins_in_code() - set(claimed)):
        coverage.append(f"no page documents `{plugin}`. A plugin a game can switch on is one it can read about.")
    for plugin, holders in sorted(claimed.items()):
        if len(holders) > 1:
            coverage.append(f"`{plugin}` is claimed by {' and '.join(p.name for p in holders)}. One page owns a plugin.")
        if plugin not in plugins_in_code():
            coverage.append(f"{holders[0]}: claims `{plugin}`, which no crate defines.")

    if coverage and COVERAGE_IS_FATAL:
        problems.extend(coverage)
    elif coverage:
        print(f"  {len(coverage)} system(s) not yet documented, which is expected until the reference is finished.", file=sys.stderr)
```

A plugin claimed by a page and defined by no crate is fatal even while coverage is advisory, because it means a page names something that has been deleted. Keep it in `coverage` for now; Task 12 makes the whole block fatal and the distinction goes away.

- [ ] **Step 4: Run it and watch the advisory appear**

Run: `python3 scripts/check-systems.py; echo "exit $?"`

Expected: `exit 0`, with `54 system(s) not yet documented, which is expected until the reference is finished.` on stderr.

If the number is not 54, the workspace has gained or lost a plugin since this plan was written. Confirm with:

```bash
grep -rho 'impl Plugin for [A-Za-z0-9_]*' --include='*.rs' crates/ | sort -u | wc -l
```

- [ ] **Step 5: Prove it goes fatal when asked**

```bash
sed -i '' 's/^COVERAGE_IS_FATAL = False/COVERAGE_IS_FATAL = True/' scripts/check-systems.py
python3 scripts/check-systems.py; echo "exit $?"
sed -i '' 's/^COVERAGE_IS_FATAL = True/COVERAGE_IS_FATAL = False/' scripts/check-systems.py
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: `exit 1` listing every unclaimed plugin, then `exit 0`. Leave the flag `False`.

- [ ] **Step 6: Commit**

```bash
git add scripts/check-systems.py
git commit -m "docs: the reference must claim every plugin

Advisory for now. A check that fails on every build is a check somebody
comments out, so this counts what is undocumented and says so without
failing, until the last page lands and the flag flips."
```

---

### Task 5: the book's skeleton

**Files:**
- Modify: `docs/guide/src/SUMMARY.md`

**Interfaces:**
- Consumes: Task 2's descending contents check, which is what makes a nested `SUMMARY.md` entry legal.
- Produces: the twenty-three page paths every later task writes to, and the seven part headings. A page filename is fixed here and is not renamed later, because Task 3's `--bless NAME` and the manifests both key on it.

- [ ] **Step 1: Append the parts and the draft entries**

An entry with an empty link is an mdBook draft: it is numbered and visible in the sidebar and rendered as a `<span>` rather than an `<a>`, so the structure shows while nothing ships blank. Append to `docs/guide/src/SUMMARY.md`:

```markdown
---

# Systems

- [Foundations]()
  - [The turn loop]()
  - [Registries and content]()
  - [Seeds and determinism]()
- [The world]()
  - [Grids and tiles]()
  - [Map generation]()
  - [Places and streaming]()
  - [Props]()
- [What acts]()
  - [Abilities]()
  - [Combat and loadout]()
  - [Items and equipment]()
  - [Minds]()
- [What it perceives]()
  - [Sight and lighting]()
  - [Noise]()
  - [Stealth]()
- [What it leaves behind]()
  - [Fire and gas]()
  - [Remains]()
  - [Statuses]()
- [What you see]()
  - [Rendering]()
  - [Panels]()
  - [Controls, modals and cursors]()
  - [Narration]()
- [What persists]()
  - [Saving and the morgue]()
  - [The overworld]()
```

The seven part names are groupings rather than pages, so they are drafts permanently. Each child gains its link as its page is written.

- [ ] **Step 2: The filenames each draft becomes**

Record these now; a later task fills one in and must not invent a name.

| Draft | File |
| --- | --- |
| The turn loop | `systems/turn-loop.md` |
| Registries and content | `systems/registries.md` |
| Seeds and determinism | `systems/seeds.md` |
| Grids and tiles | `systems/grids.md` |
| Map generation | `systems/mapgen.md` |
| Places and streaming | `systems/places.md` |
| Props | `systems/props.md` |
| Abilities | `systems/abilities.md` |
| Combat and loadout | `systems/combat.md` |
| Items and equipment | `systems/items.md` |
| Minds | `systems/minds.md` |
| Sight and lighting | `systems/sight.md` |
| Noise | `systems/noise.md` |
| Stealth | `systems/stealth.md` |
| Fire and gas | `systems/fields.md` |
| Remains | `systems/remains.md` |
| Statuses | `systems/statuses.md` |
| Rendering | `systems/rendering.md` |
| Panels | `systems/panels.md` |
| Controls, modals and cursors | `systems/controls.md` |
| Narration | `systems/narration.md` |
| Saving and the morgue | `systems/saving.md` |
| The overworld | `systems/overworld.md` |

- [ ] **Step 3: Build the book and confirm the drafts render**

```bash
mdbook build docs/guide
for title in Foundations "The turn loop" "What persists" "The overworld"; do
  grep -q "$title" docs/guide/book/toc.html && echo "present: $title"
done
```

Expected: all four `present:` lines. A count of `chapter-item` is not asserted here because it depends on how the version in use renders part titles, and a brittle number is a check somebody deletes. If `toc.html` is absent, find the sidebar file this version writes under `docs/guide/book/`; the assertion is that the new titles appear in it.

Confirm a draft is unlinked:

```bash
grep -o '<span><strong aria-hidden="true">[0-9.]*</strong> The turn loop</span>' docs/guide/book/toc.html
```

Expected: one match. A `<span>` and not an `<a>` is what makes it a draft.

- [ ] **Step 4: Confirm the checks still pass**

```bash
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py
```

Expected: all three exit 0. `check-guide.sh` must not complain about the draft entries, because a draft has no path to resolve.

- [ ] **Step 5: Commit**

```bash
git add docs/guide/src/SUMMARY.md
git commit -m "guide: the shape of the system reference

Seven parts and twenty-three drafts, in the order a turn runs through
the engine, so a reader front to back never meets a term before the page
that defines it. Each draft gains its link as its page is written."
```

---

### Task 6: wire the check in, and pay the documentation the repository asks for

**Files:**
- Modify: `.github/workflows/ci.yml:43` region, adding a step after `check-overview.sh`
- Modify: `AGENTS.md`, the documentation clause at line 14
- Modify: `docs/README.md`, the "Start here" section

**Interfaces:**
- Consumes: `scripts/check-systems.py` from Tasks 3 and 4.
- Produces: CI runs the check on every push. `AGENTS.md` names the system page as the first document a slice owes.

- [ ] **Step 1: Add the CI step**

In `.github/workflows/ci.yml`, directly after the `the inventory keeps up with the code` step:

```yaml
      - name: the system reference keeps up with the code
        run: python3 scripts/check-systems.py
```

- [ ] **Step 2: Update the documentation clause in `AGENTS.md`**

Line 14 reads `Four files, and which of them depends on what the slice is:`. It becomes `Five files, and which of them depends on what the slice is:`, and a new first bullet goes above the `docs/OVERVIEW.md` one:

```markdown
  - `docs/guide/src/systems/<system>.md`, always. It is the page a game author reads to use the system, and it carries a `documents:` manifest naming the plugins and the source files it covers. `scripts/check-systems.py` fails when one of those files changes; re-read the page, then `scripts/check-systems.py --bless <system>`.
```

Then add `scripts/check-systems.py  the reference keeps up with the code` to the layout block lower in the file, beside the other scripts.

- [ ] **Step 3: Add the reference to `docs/README.md`**

In the "Start here" section, after the guide bullet:

```markdown
- **[The system reference](guide/src/systems/)**, one page per system: what it is, how to turn it on, what the engine decides and what your game decides.
  It is part of the same book as the guide and is read at [rudehn.github.io/rl-engine](https://rudehn.github.io/rl-engine/) or as markdown from the link above.
  The guide teaches by building Warren; the reference is what you read once you know what you are looking for.
```

- [ ] **Step 4: Run every check**

```bash
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py
```

Expected: all three exit 0.

- [ ] **Step 5: Confirm the workflow parses**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('ci.yml parses')"
```

Expected: `ci.yml parses`. If PyYAML is absent, `pip install pyyaml` into a throwaway environment or skip this step and rely on the push.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml AGENTS.md docs/README.md
git commit -m "docs: the system reference is a document a slice owes

The clause named four files and now names five, with the system page
first, because it is the one a game author reads. CI runs the check."
```

---

### Task 7: the pilot page, `systems/sight.md`

**Files:**
- Create: `docs/guide/src/systems/sight.md`
- Modify: `docs/guide/src/SUMMARY.md`, giving the `Sight and lighting` draft its link

**Interfaces:**
- Consumes: every task above.
- Produces: the page every later page is modelled on. Its section set, its manifest and its length are the template.

Facts established by reading the source, so the page does not have to rediscover them:

- `LightingPlugin` is at `crates/rl-bevy/src/lighting.rs:306`. Its `build` inserts `Lighting::dark()`, registers `LightEvent`, runs `update_lighting` in `EngineSet::Light` and `tick_fuel` in `ResolveSet::Effects`, and resets `Lighting` on a new run with `Lighting::dark`. Its `finish` calls `depends_on::<CorePlugin>`.
- The public surface is `LightSource { intensity: u8, radius: i32, color: Rgb, flicker: u8 }`, `DarkSight(pub i32)`, `Fuel(pub u32)`, `LightEvent`, `DEFAULT_THRESHOLD: u8 = 16`, the `Lighting` resource, `gate()` and `perceives()`.
- `flicker` is drawn and never read by gameplay. That belongs in `The line`.
- Whether a thing glows is the game's call, made by inserting or removing `LightSource`. Ambient is a plain field the game writes; the engine has no clock hook and no notion of a day. Both belong in `The line`.
- `FovPlugin` is at `crates/rl-bevy/src/fov.rs`.
- The example to quote is `examples/tutorial/src/bin/step02_light.rs`, anchors `lantern` at line 161 and `start` at line 98. `start` shows `LANTERN` being spawned onto the player; `lantern` shows the component being added and removed to open and shade it.

- [ ] **Step 1: Write the page**

Create `docs/guide/src/systems/sight.md` with exactly these sections, in this order, and nothing else:

````markdown
<!-- documents:
     plugins: FovPlugin, LightingPlugin
     files: crates/rl-bevy/src/fov.rs
            crates/rl-bevy/src/lighting.rs
            crates/rl-grid/src/fov.rs
            crates/rl-grid/src/light.rs
     fingerprint: 00000000 -->

# Sight and lighting

<!-- one paragraph: what the system is. Sight is a viewshed per actor.
     Lighting is a field over the loaded window that cuts that viewshed
     down to what is lit, within an actor's dark sight, or adjacent. -->

## Turning it on

<!-- FovPlugin alone gives sight with no dark. LightingPlugin adds the
     gate. What depends_on::<CorePlugin> means for the order they are
     added in. What a game sees if it adds neither. -->

## The model

<!-- LightSource, DarkSight, Fuel, LightEvent, the Lighting resource,
     DEFAULT_THRESHOLD. What each field is in. -->

## Using it

```rust
{{#include ../../../../examples/tutorial/src/bin/step02_light.rs:lantern}}
```

## The line

<!-- Whether a thing glows is the game's, made by inserting or removing
     a component. Ambient is a field the game writes; there is no clock
     hook and no notion of a day. flicker is drawn and never read by
     gameplay. -->

## Where it lives

<!-- rl-grid (tier 1) holds the field and the shadowcast; rl-bevy
     (tier 2) holds the plugins and the systems. -->
````

The HTML comments are instructions to whoever writes the prose and none of them survives into the committed page. A page that still contains one is not finished.

- [ ] **Step 2: Expand the include**

```bash
python3 scripts/expand-guide.py
grep -q 'const LANTERN' docs/guide/src/systems/sight.md && echo "quoted"
```

Expected: `quoted`. The fence now holds the real code with an `<!-- include: -->` marker above it, so the markdown reads correctly in an editor and on GitHub without building the book.

- [ ] **Step 3: Bless the page**

```bash
python3 scripts/check-systems.py --bless sight
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: the confirmation line naming all four files, then `exit 0`.

- [ ] **Step 4: Give the draft its link**

In `docs/guide/src/SUMMARY.md`, `  - [Sight and lighting]()` becomes `  - [Sight and lighting](systems/sight.md)`.

- [ ] **Step 5: Run the acceptance checklist**

Every item is a hard gate. A `no` sends the page back to Step 1.

```bash
# length
[ "$(wc -l < docs/guide/src/systems/sight.md)" -le 125 ] && echo "length ok"
# no em dash
! grep -q '—' docs/guide/src/systems/sight.md && echo "dashes ok"
# no banned word
! grep -qiE '\b(powerful|simply|just|robust|seamless|leverage|delve|comprehensive|elegant|straightforward)\b' docs/guide/src/systems/sight.md && echo "words ok"
# no instruction comment left behind
! grep -q '<!-- one paragraph\|<!-- FovPlugin\|<!-- LightSource\|<!-- Whether a thing\|<!-- rl-grid' docs/guide/src/systems/sight.md && echo "comments ok"
# the six sections, in order
grep -c '^## ' docs/guide/src/systems/sight.md
```

Expected: `length ok`, `dashes ok`, `words ok`, `comments ok`, and `5`.

By eye, and not automatable:

- Does the opening paragraph say what the system is without saying it is good?
- Does `Turning it on` name the plugin and say what absence means?
- Does `The line` name something the engine refuses to decide? A page whose `The line` is vague has not found the boundary yet.
- Does any paragraph restate its own heading?
- Is every sentence on its own line in the source?

- [ ] **Step 6: Build the book and read the page**

```bash
mdbook build docs/guide && echo "open docs/guide/book/systems/sight.html"
```

Read it rendered. The sidebar should show `Sight and lighting` as a link while its six siblings in that part are still greyed.

- [ ] **Step 7: Run every check and commit**

```bash
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py
git add docs/guide/src/systems/sight.md docs/guide/src/SUMMARY.md
git commit -m "docs: sight and lighting, the first page of the reference

The pilot the template is judged on: what the system is, how to turn it
on, the model, a worked use quoted from the tutorial, the line between
what the engine decides and what a game does, and where it lives."
```

---

### Task 8: judge the template, then freeze it

**Files:**
- Create: `scripts/check-systems-style.sh`
- Modify: `AGENTS.md`, adding a "Writing a system page" section after "Building UI"

Do not put a `README.md` in `docs/guide/src/systems/`. Anything under `src` that `SUMMARY.md` lists is published, and anything it does not list trips the contents check from Task 2. The template lives in `AGENTS.md`.

**Interfaces:**
- Consumes: Task 7's page.
- Produces: the written template every page from Task 9 onward follows, and the acceptance checklist as a runnable script.

This task exists because a template judged after twenty-three pages exist is a template nobody changes.

- [ ] **Step 1: Read the pilot as a stranger**

Answer these in the commit message of this task, in one sentence each:

- Did `The line` carry its weight on this page, or was it padding?
- Was 120 lines the right cap, or did the page want more or less?
- Did `Using it` want a second anchor, the `start` one, showing where the component is first attached?
- Is `Where it lives` worth a heading, or is it one sentence that belongs at the end of `Turning it on`?

Revise the template now if any answer says so. Revising it later costs twenty-two pages.

- [ ] **Step 2: Write the acceptance checklist as a script**

Create `scripts/check-systems-style.sh`, which the writer of each page runs before committing:

```bash
#!/usr/bin/env bash
# The mechanical half of a system page's review.
#
#   scripts/check-systems-style.sh docs/guide/src/systems/noise.md
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
! grep -q '<!-- [a-z]' "$page" || note "$page: an instruction comment survived into the page."
[ "$(grep -c '^## ' "$page")" -eq 5 ] \
	|| note "$page: $(grep -c '^## ' "$page") sections, not 5. The template is fixed: Turning it on, The model, Using it, The line, Where it lives."

if [ "$failed" -ne 0 ]; then
	exit 1
fi
echo "$page: style ok. The prose is still a review matter."
```

`chmod +x scripts/check-systems-style.sh`.

- [ ] **Step 3: Run it against the pilot**

Run: `scripts/check-systems-style.sh docs/guide/src/systems/sight.md`

Expected: `docs/guide/src/systems/sight.md: style ok. The prose is still a review matter.`

- [ ] **Step 4: Prove it fails**

```bash
printf '\n## An extra heading\n' >> docs/guide/src/systems/sight.md
scripts/check-systems-style.sh docs/guide/src/systems/sight.md; echo "exit $?"
git checkout docs/guide/src/systems/sight.md
```

Expected: `exit 1` naming 6 sections rather than 5.

- [ ] **Step 5: Write the template into `AGENTS.md`**

Add a section after "Building UI":

```markdown
## Writing a system page

`docs/guide/src/systems/<system>.md` is what a game author reads to use a system, and `docs/design/<system>.md` is why it is shaped that way.
The reference is published; the design notes are not.

Six parts, fixed, in this order: one paragraph saying what the system is, then `Turning it on`, `The model`, `Using it`, `The line`, `Where it lives`.
`The line` says what the engine decides and what the game decides, and it is the section a reader arrives for.
No `Cost`, no `Risks`, no `Phases`; those stay in the design notes.

Every code block is an `expand-guide.py` include of a real anchor in `examples/`.
A page carries no invented code.
Run `scripts/check-systems-style.sh <page>` before committing, and `scripts/check-systems.py --bless <system>` after confirming the page against code that moved.
```

- [ ] **Step 6: Run every check and commit**

```bash
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py
git add scripts/check-systems-style.sh AGENTS.md
git commit -m "docs: the system page template, judged on the pilot and written down

<one sentence per question from step 1>"
```

---

### Task 9: the spine, three pages from nothing

**Files:**
- Create: `docs/guide/src/systems/turn-loop.md`, `systems/registries.md`, `systems/seeds.md`
- Modify: `docs/guide/src/SUMMARY.md`, three links

**Interfaces:**
- Consumes: the template frozen in Task 8, the scripts from Tasks 3, 4 and 8.
- Produces: the three pages every later page refers back to. Nothing later may define the turn loop, a registry or a seed; they link here.

These three have no design doc, so they prove the template on the hard case before the nine easy ones are spent. The turn loop is first because every other page refers to it.

For each of the three, in this order, one commit each:

- [ ] **Step 1: `systems/turn-loop.md`**

Source to read first: `crates/rl-core/src/turn.rs`, which `AGENTS.md` names as the density of doc comment to match, and `crates/rl-bevy/src/turn.rs`, `crates/rl-bevy/src/cue.rs`.

Manifest: `plugins: CorePlugin`, files `crates/rl-core/src/turn.rs`, `crates/rl-bevy/src/turn.rs`, `crates/rl-bevy/src/cue.rs`, `crates/rl-bevy/src/plugin.rs`.

`The line` must say that costs and clocks are integers in hundredths of a step, and that anything reacting to what a turn caused goes in `TurnSet::React` rather than in drawing.

- [ ] **Step 2: `systems/registries.md`**

Source: `crates/rl-bevy/src/registries.rs`. Manifest plugins: whichever plugin that module defines, read from the file rather than assumed.

`The line` must say that content is an id in a registry, that there are no closed taxonomy enums, and that `#[non_exhaustive]` with a `Custom { id }` variant is not how the engine extends.

An example anchor: `examples/tutorial/src/bin/step08_content.rs` has 15 anchors and is the chapter about content in files.

- [ ] **Step 3: `systems/seeds.md`**

Source: `crates/rl-bevy/src/seed.rs`.

`The line` must say that a subsystem's stream is a `Stream` registered with `add_stream` and derived from the run's `Seed` through `RunSeed::derive(domain, index)`, that a game's own draws come from `Seed::stream`, and that engine code never builds a generator from a constant or from entropy.

- [ ] **Step 4: for each page, before its commit**

```bash
python3 scripts/expand-guide.py
scripts/check-systems-style.sh docs/guide/src/systems/<name>.md
python3 scripts/check-systems.py --bless <name>
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py
```

All must pass, then give the draft its link in `SUMMARY.md` and commit the page, the link and nothing else.

---

### Task 10: the eight with design notes to mine

**Files:**
- Create: `systems/abilities.md`, `systems/fields.md`, `systems/minds.md`, `systems/noise.md`, `systems/props.md`, `systems/remains.md`, `systems/stealth.md`, `systems/panels.md`
- Modify: `docs/guide/src/SUMMARY.md`, eight links

**Interfaces:**
- Consumes: Task 9's three pages, which these link back to rather than redefine.
- Produces: eight pages. After this task, `docs/design` holds nothing a user needs, which is what the spec means by making it deletable.

Each page has a design note to mine, at `docs/design/<same name>.md`, except `panels.md`, whose note is `docs/design/ui.md`.

Mining means: take the model and the usage, leave the `Cost`, `Risks`, `Phases` and `What is not here` sections behind, and drop every citation of the plan. Where the note carries a piece of reasoning a user needs to use the system correctly, it becomes one sentence in `The line` or in `The model`, not a section.

- [ ] **Step 1: one commit per page, in this order**

`fields.md`, `remains.md`, `minds.md`, `noise.md`, `stealth.md`, `props.md`, `abilities.md`, `panels.md`.

Shortest note first. `fields.md` mines a 5KB note and `abilities.md` mines a 29KB one, so the order spends the easy ones while the template is newest and hits the hardest after seven pages of practice.

- [ ] **Step 2: for each page**

Read `docs/design/<name>.md` in full, then the crate source its manifest will list, then write the page to the template.

Manifest plugins, read from the source rather than assumed:

| Page | Plugins |
| --- | --- |
| `fields.md` | `FirePlugin`, `GasPlugin` |
| `remains.md` | `RemainsPlugin` |
| `minds.md` | `MindsPlugin` |
| `noise.md` | `NoisePlugin` |
| `stealth.md` | `StealthPlugin` |
| `props.md` | `PropsPlugin` |
| `abilities.md` | `AbilitiesPlugin`, `ThrowingPlugin` |
| `panels.md` | `UiPlugin` and every `*Panel` and `*ViewPlugin` except `MapViewPlugin` and `NarrationViewPlugin` |

Confirm each list against the code before writing it into a manifest:

```bash
grep -rho 'impl Plugin for [A-Za-z0-9_]*' --include='*.rs' crates/ | sed 's/impl Plugin for //' | sort -u
```

- [ ] **Step 3: for each page, before its commit**

The same four commands as Task 9 Step 4.

- [ ] **Step 4: after the last of the eight**

```bash
python3 scripts/check-systems.py
```

The advisory count should have fallen from 54 to roughly 15. The exact figure depends on how `panels.md` divides the panel plugins, which is why Step 2 says to confirm against the code.

---

### Task 11: the remaining eleven

**Files:**
- Create: `systems/grids.md`, `systems/mapgen.md`, `systems/places.md`, `systems/combat.md`, `systems/items.md`, `systems/statuses.md`, `systems/rendering.md`, `systems/controls.md`, `systems/narration.md`, `systems/saving.md`, `systems/overworld.md`
- Modify: `docs/guide/src/SUMMARY.md`, eleven links

**Interfaces:**
- Consumes: every page before it.
- Produces: coverage of every remaining plugin, which is what Task 12 needs before it can flip the flag.

None of these has a design note. `AGENTS.md` says each is documented in its crate's module docs and in the overview, so the module docs are the source to read.

- [ ] **Step 1: one commit per page, in this order**

`grids.md`, `mapgen.md`, `places.md`, `combat.md`, `items.md`, `statuses.md`, `rendering.md`, `controls.md`, `narration.md`, `saving.md`, `overworld.md`.

The order runs the spec's parts front to back, so a page never refers forward.

- [ ] **Step 2: the plugins each page claims**

| Page | Plugins |
| --- | --- |
| `grids.md` | none; `rl-grid` is tier 1 and defines no plugin |
| `mapgen.md` | none; `rl-mapgen` is tier 1 and defines no plugin |
| `places.md` | `StreamingPlugin` |
| `combat.md` | `CombatPlugin` |
| `items.md` | `ItemsPlugin`, `ContainerPanel`, `ContainerViewPlugin`, `InventoryPanel`, `InventoryViewPlugin` |
| `statuses.md` | `StatusPlugin`, `FactsPlugin` |
| `rendering.md` | `TerminalPlugin`, `ParticlesPlugin`, `MapViewPlugin`, `CapturePlugin` |
| `controls.md` | `ControlsPanel`, `GameMenuPanel`, `InteractKey`, `KeyScriptPlugin`, `ReplayPlugin` |
| `narration.md` | `NarratorPlugin`, `NarrationViewPlugin` |
| `saving.md` | `SavePlugin`, `UnloadPlugin` |
| `overworld.md` | `OverworldPlugin` |

A page with no plugin still carries a manifest with a `plugins:` field. Write `plugins: none` and make `manifest()` accept it: in Task 3's parser the field is a list, so `none` is one entry, and Task 4's coverage block must skip the literal. Add to `plugins_in_code()`'s consumer in the coverage block:

```python
        for plugin in fields["plugins"]:
            if plugin == "none":
                continue
            claimed.setdefault(plugin, []).append(page)
```

Make that edit in the commit that writes `grids.md`, which is the first page to need it.

- [ ] **Step 3: for each page, before its commit**

The same four commands as Task 9 Step 4.

- [ ] **Step 4: confirm the table above is exhaustive**

```bash
python3 scripts/check-systems.py
```

Expected: no advisory line at all. Every one of the 54 plugins is claimed. If the advisory persists, the table above missed a plugin; the script names which.

---

### Task 12: make coverage fatal

**Files:**
- Modify: `scripts/check-systems.py`, one line
- Modify: `docs/OVERVIEW.md`, the "Not built yet" list, removing the system reference if it is named there
- Modify: `CHANGELOG.md`, under `Unreleased`

**Interfaces:**
- Consumes: Task 11's complete coverage.
- Produces: a build that fails when a new plugin lands with no page. This is the commit that makes the rule real.

- [ ] **Step 1: Confirm coverage is complete**

Run: `python3 scripts/check-systems.py`

Expected: `the system reference is current across 23 page(s)` and no advisory. Do not proceed otherwise.

- [ ] **Step 2: Flip the flag**

```python
COVERAGE_IS_FATAL = True
```

and change the comment above it to read:

```python
# Every system has a page, so a plugin that lands without one fails the
# build. This went True in the commit that wrote the last page.
```

- [ ] **Step 3: Prove it bites**

```bash
cat > crates/rl-core/src/_probe.rs <<'EOF'
//! A probe.
use bevy::prelude::*;
/// A plugin no page claims.
pub struct ProbePlugin;
impl Plugin for ProbePlugin {
	fn build(&self, _app: &mut App) {}
}
EOF
python3 scripts/check-systems.py; echo "exit $?"
rm crates/rl-core/src/_probe.rs
python3 scripts/check-systems.py; echo "exit $?"
```

Expected: `exit 1` with ``no page documents `ProbePlugin`. A plugin a game can switch on is one it can read about.``, then `exit 0`.

`rl-core` is tier 0 and must not depend on Bevy, so the probe file would fail `check-tiers.sh`. That does not matter here because it is never committed and `check-systems.py` only greps. If this feels wrong, put the probe in `crates/rl-bevy/src/_probe.rs` instead.

- [ ] **Step 4: Pay the documentation**

`CHANGELOG.md`, under `Unreleased`, in the section a game author reads:

```markdown
- The engine's systems each have a reference page in the published book, at `docs/guide/src/systems/`.
  The guide still teaches by building Warren; the reference is what to read once you know what you are looking for.
  `docs/design/` is unchanged and stays the record of why each system is shaped as it is.
```

`README.md`'s feature list gains a line only if a reader shopping for an engine would look for it. A published reference is such a thing, so add it beside the guide.

- [ ] **Step 5: Run everything**

```bash
scripts/check-guide.sh && scripts/check-overview.sh && python3 scripts/check-systems.py && mdbook build docs/guide
```

Expected: all pass, and `docs/guide/book/systems/` holds 23 HTML files.

```bash
ls docs/guide/book/systems/*.html | wc -l
```

Expected: `23`.

- [ ] **Step 6: Commit**

```bash
git add scripts/check-systems.py CHANGELOG.md README.md docs/OVERVIEW.md
git commit -m "docs: a plugin without a page now fails the build

Every system has a reference page, so the coverage check stops being
advisory. A subsystem that lands from here on pays its page in the same
commit, the way it already pays the overview."
```

---

## What this plan does not do

Merging `check-overview.sh` and `check-systems.py`, which will then both require every plugin to be named somewhere. The spec's section 6.1 says this is worth doing and is out of scope here.

Deleting `docs/design`. Task 10 makes it possible by absorbing what a user needs. Whether to delete it is a decision to take after the reference has been read by someone who is not its author.

Publishing a markdown rendering of the book. `expand-guide.py` commits expanded code into the source, so an agent with the repository reads the real thing, and that is the case that matters.
