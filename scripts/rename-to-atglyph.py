#!/usr/bin/env python3
"""The engine's rename from `rl-engine` to `atglyph`, as a script.

    scripts/rename-to-atglyph.py          rewrite every living file and move the crates
    scripts/rename-to-atglyph.py --check  list each leftover old name as path:line, exit 1 if any
    scripts/rename-to-atglyph.py --urls   rename only the repository's and the guide's addresses

The rename is a script rather than one commit's hand edits because other
branches keep landing on `main` while it is in flight: the branch is
rebased, takes `main`'s side of every conflict in a renamed file, and runs
this again, and a branch started before the rename migrates the same way.
So every rule here is idempotent, and the script is run again after every
rebase until `0.4.0` is released; after that it and its test go.

`--urls` is the release's step, run once the GitHub repository itself is
renamed. Until then the old addresses are the ones that resolve, so the
main run masks them and leaves them as they are.

History is not rewritten: the dated plans, the reviews, `.planning`, the
plan's progress log, the soak log and the changelog's released entries say
what the engine was called when they were written.
"""

import os
import pathlib
import re
import subprocess
import sys
from typing import Callable, Literal

CRATES = ("core", "grid", "mapgen", "rules", "save", "bevy", "render", "ui", "tty")

# The module each sub-crate is reached by through the facade. `core` is
# Rust's own crate and `atglyph::bevy` would read as Bevy itself, so those
# two take another word; the rest keep their suffix.
SHORT: dict[str, str] = {
    "core": "primitives",
    "grid": "grid",
    "mapgen": "mapgen",
    "rules": "rules",
    "save": "save",
    "bevy": "ecs",
    "render": "render",
    "ui": "ui",
    "tty": "tty",
}

_C = "|".join(CRATES)

# The addresses that change only when the repository is renamed, at the release.
URLS = (
    ("github.com/rudehn/rl-engine", "github.com/rudehn/atglyph"),
    ("rudehn.github.io/rl-engine", "rudehn.github.io/atglyph"),
)

# Paths that record what the engine was, and keep its name.
HISTORY_DIRS = ("docs/superpowers/", "docs/reviews/", ".planning/")
HISTORY_FILES = ("docs/PLAN.md", "examples/undermere/SOAK.md")
# This script and its test hold the old names as patterns.
SELF = ("scripts/rename-to-atglyph.py", "scripts/test_rename_to_atglyph.py")
FACADE_USERS = ("examples/foundry/", "examples/undermere/", "examples/tutorial/", "templates/", "docs/guide/")

FACADE_LIB = "crates/atglyph/src/lib.rs"
CHANGELOG = "CHANGELOG.md"
# The changelog's released entries begin at its first version heading.
RELEASED = "\n## 0."
# The rename's own changelog entry names the old names on purpose, up to the
# next blank line.
RENAME_ENTRY = re.compile(r"^- The engine is `atglyph`.*?(?=\n\n|\Z)", re.M | re.S)

_FACADE_PATH = re.compile(rf"\brl_engine::rl_({_C})\b")
_BARE_CRATE = re.compile(rf"\brl_({_C})\b")
_FACADE_CRATE = re.compile(r"\brl_engine\b")
_PACKAGE = re.compile(r"\brl-engine\b")
_SUB_PACKAGE = re.compile(rf"\brl-({_C})\b")
_ENV = re.compile(r"\bRL_(FULL_SEEDS|RECORD|REPLAY|CAPTURE(_[A-Z]+)?)\b")
_REEXPORT = re.compile(rf"^(\s*)pub use atglyph_({_C});", re.M)


def _short(match: re.Match) -> str:
    return f"atglyph::{SHORT[match.group(1)]}"


def rewrite(text: str, facade_user: bool) -> str:
    """`text` with every old name renamed, the URLs aside.

    A game reaches the sub-crates through the facade, so in a game's file a
    bare sub-crate path is the facade's short module; in the engine it is the
    sub-crate itself.
    """
    masks = []
    for i, (old, _) in enumerate(URLS):
        mask = f"\x00URL{i}\x00"
        masks.append((mask, old))
        text = text.replace(old, mask)
    text = _FACADE_PATH.sub(_short, text)
    if facade_user:
        text = _BARE_CRATE.sub(_short, text)
    text = _FACADE_CRATE.sub("atglyph", text)
    text = _BARE_CRATE.sub(r"atglyph_\1", text)
    text = _PACKAGE.sub("atglyph", text)
    text = _SUB_PACKAGE.sub(r"atglyph-\1", text)
    text = _ENV.sub(r"ATGLYPH_\1", text)
    for mask, old in masks:
        text = text.replace(mask, old)
    return text


def rename_urls(text: str) -> str:
    """`text` with the repository's and the guide's addresses renamed."""
    for old, new in URLS:
        text = text.replace(old, new)
    return text


def reexport_short_names(text: str) -> str:
    """The facade's `pub use` of each sub-crate, under its short name."""
    return _REEXPORT.sub(lambda m: f"{m.group(1)}pub use atglyph_{m.group(2)} as {SHORT[m.group(2)]};", text)


def rewrite_changelog(text: str, change: Callable[[str], str] = lambda t: rewrite(t, facade_user=False)) -> str:
    """The changelog with `change` applied to its unreleased part only.

    The released entries and the rename's own entry are left as written.
    """
    cut = text.find(RELEASED)
    head, tail = (text, "") if cut < 0 else (text[:cut], text[cut:])
    out, at = [], 0
    for entry in RENAME_ENTRY.finditer(head):
        out.append(change(head[at : entry.start()]))
        out.append(entry.group(0))
        at = entry.end()
    out.append(change(head[at:]))
    return "".join(out) + tail


def classify(path: pathlib.Path) -> Literal["skip", "facade_user", "engine"]:
    """Whether `path` is left alone, a game's file, or the engine's."""
    name = path.as_posix()
    if {".git", "target"} & set(path.parts):
        return "skip"
    if name.startswith(HISTORY_DIRS) or name in HISTORY_FILES or name in SELF:
        return "skip"
    if path.name == "Cargo.lock" or path.is_symlink():
        return "skip"
    if path.is_file():
        try:
            path.read_bytes().decode("utf-8")
        except UnicodeDecodeError:
            return "skip"
    if name.startswith(FACADE_USERS) or name == "README.md":
        return "facade_user"
    return "engine"


def destination(path: str) -> str:
    """Where `path` lives once its crate directory has moved."""
    path = re.sub(r"^crates/rl-engine/", "crates/atglyph/", path)
    return re.sub(rf"^crates/rl-({_C})/", r"crates/atglyph-\1/", path)


def rewrite_file(path: str, text: str, facade_user: bool) -> str:
    """One file's text, renamed by the rules its path calls for."""
    if path == CHANGELOG:
        text = rewrite_changelog(text)
    else:
        text = rewrite(text, facade_user)
    if destination(path) == FACADE_LIB:
        text = reexport_short_names(text)
    return text


def tracked() -> list[str]:
    """Every tracked file, so untracked build output is never touched."""
    out = subprocess.run(["git", "ls-files", "-z"], capture_output=True, check=True).stdout
    return [p for p in out.decode("utf-8").split("\0") if p]


def living() -> list[tuple[str, str, bool]]:
    """Each tracked file the rename reads, with its text and whether it is a game's."""
    found = []
    for name in tracked():
        path = pathlib.Path(name)
        if not path.is_file():
            continue
        cls = classify(path)
        if cls == "skip":
            continue
        found.append((name, path.read_bytes().decode("utf-8"), cls == "facade_user"))
    return found


def check() -> int:
    """Print each leftover old name as path:line; 1 if there was one."""
    leftovers = []
    for name, text, facade_user in living():
        if destination(name) != name:
            leftovers.append(f"{name}: in a crate directory not yet moved")
        fresh = rewrite_file(name, text, facade_user).split("\n")
        for number, (old, new) in enumerate(zip(text.split("\n"), fresh), start=1):
            if old != new:
                leftovers.append(f"{name}:{number}: {old.strip()}")
    for line in leftovers:
        print(line)
    return 1 if leftovers else 0


def urls() -> int:
    """Rename the repository's and the guide's addresses, history aside."""
    for name, text, _ in living():
        fresh = rewrite_changelog(text, rename_urls) if name == CHANGELOG else rename_urls(text)
        if fresh != text:
            pathlib.Path(name).write_bytes(fresh.encode("utf-8"))
    return 0


def run() -> int:
    """Rewrite every living file, move the crate directories, rename the lockfile's entries."""
    for name, text, facade_user in living():
        fresh = rewrite_file(name, text, facade_user)
        if fresh != text:
            pathlib.Path(name).write_bytes(fresh.encode("utf-8"))
    moves = [("crates/rl-engine", "crates/atglyph")] + [(f"crates/rl-{c}", f"crates/atglyph-{c}") for c in CRATES]
    for old, new in moves:
        if pathlib.Path(old).is_dir():
            subprocess.run(["git", "mv", old, new], check=True)
    # Renames the workspace's own entries in the lockfile without upgrading
    # any dependency.
    subprocess.run(["cargo", "update", "--workspace"], check=True)
    # `atglyph` sorts before `bevy` where `rl_engine` sorted after it, so
    # rustfmt reorders the imports. The template is no workspace member and
    # is formatted by its own `rustfmt.toml`.
    subprocess.run(["cargo", "fmt", "--all"], check=True)
    template = [name for name in tracked() if name.startswith("templates/") and name.endswith(".rs")]
    if template:
        subprocess.run(["rustfmt", "--edition", "2024", *template], check=True)
    return 0


def main() -> int:
    root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
    os.chdir(root)
    argv = sys.argv[1:]
    if argv == ["--check"]:
        return check()
    if argv == ["--urls"]:
        return urls()
    if argv:
        print(__doc__, file=sys.stderr)
        return 2
    return run()


if __name__ == "__main__":
    raise SystemExit(main())
