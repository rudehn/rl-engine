#!/usr/bin/env python3
"""The facade's prelude stays the size of a first game, and names each thing once.

    scripts/check-prelude.py    reads crates/atglyph/src/prelude.rs and fails
                                when its core lists more than 80 names, when
                                a name is in two of the core and the topic
                                modules, or when anything but `all` globs

The core is what tutorial steps 1 to 3 name, and the topic modules hold the
rest, so a game globs the core and the modules its chapters introduce. Two
modules that both export a name make it ambiguous at every use in a game
that globs both, which the compiler reports only where a game writes it;
this reports it here, naming each one and its modules. Only `all` may glob,
since it is every module at once and a glob anywhere else would hide which
names a module holds from this count.
"""

import re
import sys
from pathlib import Path

CORE_LIMIT = 80
PRELUDE = Path(__file__).resolve().parent.parent / "crates/atglyph/src/prelude.rs"


def strip_comments(text):
    """The source with line comments, doc comments included, removed."""
    return "\n".join(line.split("//", 1)[0] for line in text.split("\n"))


def split_top(inner):
    """Split a use tree's brace contents at its top-level commas."""
    parts, depth, cur = [], 0, ""
    for ch in inner:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    return [p.strip() for p in parts if p.strip()]


def names(tree):
    """Every name a `pub use` tree brings in, `*` for a glob."""
    tree = " ".join(tree.split())
    brace = tree.find("{")
    if brace < 0:
        if tree.endswith("*"):
            return ["*"]
        if " as " in tree:
            return [tree.rsplit(" as ", 1)[1].strip()]
        last = tree.rsplit("::", 1)[-1]
        return [last]
    prefix = tree[:brace].rstrip(":")
    out = []
    for part in split_top(tree[brace + 1 : tree.rfind("}")]):
        if part == "self":
            out.append(prefix.rsplit("::", 1)[-1])
        else:
            out.extend(names(part))
    return out


def modules(text):
    """Each module's names: the core's under "", each topic's under its own."""
    text = strip_comments(text)
    found = {"": []}
    # Pull every `pub mod name { ... }` out, by brace depth.
    rest, i = "", 0
    for m in re.finditer(r"pub mod (\w+) \{", text):
        if m.start() < i:
            continue
        depth, j = 0, m.end() - 1
        while True:
            if text[j] == "{":
                depth += 1
            elif text[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        rest += text[i : m.start()]
        body = text[m.end() : j]
        found[m.group(1)] = [n for use in re.findall(r"pub use ([^;]+);", body) for n in names(use)]
        i = j + 1
    rest += text[i:]
    found[""] = [n for use in re.findall(r"pub use ([^;]+);", rest) for n in names(use)]
    return found


def main():
    if not PRELUDE.is_file():
        sys.exit(f"check-prelude: no {PRELUDE}; the prelude is its own file, listing its names")
    found = modules(PRELUDE.read_text())
    label = lambda m: "prelude" if m == "" else f"prelude::{m}"
    problems = []
    core = found[""]
    if len(core) > CORE_LIMIT:
        problems.append(f"the core lists {len(core)} names, more than {CORE_LIMIT}")
    for module, listed in found.items():
        if "*" in listed and module != "all":
            problems.append(f"{label(module)} globs; only prelude::all may")
    where = {}
    for module, listed in found.items():
        if module == "all":
            continue
        for name in listed:
            where.setdefault(name, []).append(label(module))
    for name, places in sorted(where.items()):
        if name != "*" and len(places) > 1:
            problems.append(f"{name} is exported by {', '.join(places)}")
    if problems:
        print("check-prelude: " + str(PRELUDE.relative_to(PRELUDE.parents[3])), file=sys.stderr)
        for p in problems:
            print("  " + p, file=sys.stderr)
        sys.exit(1)
    sizes = ", ".join(f"{label(m)} {len(v)}" for m, v in found.items() if m != "all")
    print(f"check-prelude: {sizes}")


if __name__ == "__main__":
    main()
