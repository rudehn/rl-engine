#!/usr/bin/env bash
# The gate a commit runs before it is made, scoped to what it can break.
#
#   scripts/gate.sh          the packages the branch touched and every package
#                            that depends on one, their fast tests only, and
#                            every public type name declared once, and the
#                            prelude holding each name once
#   scripts/gate.sh --all    the code half of CI: the whole workspace with its
#                            slow tests, the tier boundary and the wasm build,
#                            for a branch's end and before it merges
#
# The fast tier is nextest's default profile, everything but the slow tier
# that .config/nextest.toml names: the play-throughs and whole-run sweeps,
# which a commit of a feature does not need and a branch's end runs once.
#
# What the branch touched is every file changed since it left local `main`,
# committed or not, and untracked files beside them. A file in a package's
# directory touches that package; one of the workspace's own build files
# (Cargo.toml, Cargo.lock, .cargo/, .config/, rustfmt.toml, the build scripts
# under scripts/) touches all of them; anything else, a doc page say, touches
# none.
#
# The build is always the whole workspace's, with only the tests narrowed. A
# build narrowed with `-p` may unify a dependency's features differently, which
# is a second copy of it in target/, compiled cold; the whole workspace's build
# is the one every checkout and every clone of one already holds, and cargo
# rebuilds only what changed.
# Clippy is the whole workspace's for the same reason, and costs nothing for a
# crate that has not changed. Doc-tests are compiled afresh on every run, so
# they are run only for the touched packages and their dependents.
#
# After the tests, scripts/test-times.py names the slowest and holds the run
# to its budget: the whole workspace's with both tiers for `--all`, the fast
# tier's for a commit that touched every package, Undermere's fast tier's for
# one that touched only Undermere, and the per-test limits for any other set
# (AGENTS.md, "Writing fast tests").
#
# ATGLYPH_FULL_SEEDS=1 is passed through, for the generation tests' full range.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() {
    echo "gate: $*"
    "$@"
}

if [ "${1:-}" = "--all" ]; then
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets -- -D warnings
    step cargo nextest run --workspace --profile full
    step scripts/test-times.py --profile full --check full --top 10
    step cargo test --doc --workspace
    step scripts/check-tiers.sh
    step scripts/check-tiers.sh --wasm
    step scripts/check-names.sh
    step scripts/check-prelude.py
    echo "gate: the whole workspace passed"
    exit 0
fi

base=$(git merge-base main HEAD)
packages=$({
    git diff --name-only "$base"
    git ls-files --others --exclude-standard
} | sort -u | python3 -c '
import json, subprocess, sys

meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--format-version", "1"],
    capture_output=True, text=True, check=True).stdout)
root = meta["workspace_root"] + "/"
members = set(meta["workspace_members"])
dirs = {}
names = {}
for package in meta["packages"]:
    if package["id"] in members:
        directory = package["manifest_path"][len(root):].rsplit("/", 1)[0] + "/"
        dirs[directory] = package["name"]
        names[package["id"]] = package["name"]
build = ("Cargo.toml", "Cargo.lock", ".cargo/", ".config/", "rustfmt.toml",
         "clippy.toml", "rust-toolchain", "scripts/rustc-wrapper", "scripts/slot")

touched = set()
for path in sys.stdin.read().split():
    if path.startswith(build):
        touched = set(names.values())
        break
    owners = [d for d in dirs if path.startswith(d)]
    if owners:
        touched.add(dirs[max(owners, key=len)])

# Every package that depends on a touched one, through any kind of dependency.
dependents = {}
for node in meta["resolve"]["nodes"]:
    if node["id"] in members:
        for dep in node["deps"]:
            if dep["pkg"] in members:
                dependents.setdefault(names[dep["pkg"]], set()).add(names[node["id"]])
affected = set(touched)
frontier = list(touched)
while frontier:
    for dependent in dependents.get(frontier.pop(), ()):
        if dependent not in affected:
            affected.add(dependent)
            frontier.append(dependent)
print(" ".join(sorted(affected)))
')

step cargo fmt --all --check
step scripts/check-names.sh
step scripts/check-prelude.py
step cargo clippy --workspace --all-targets -- -D warnings
if [ -z "$packages" ]; then
    echo "gate: the branch touches no package, so no tests run"
    exit 0
fi
echo "gate: testing $packages"
filter=$(printf ' | package(%s)' $packages)
step cargo nextest run --workspace -E "${filter# | }"
scope=tests
[ "$packages" = undermere ] && scope=undermere
[ "$packages" = "$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json, sys; print(" ".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"])))')" ] && scope=fast
step scripts/test-times.py --check "$scope" --top 10
# Only a library has doc-tests, and `cargo test --doc` refuses a package
# with none, such as the tutorial, whose steps are all binaries.
libraries=$(cargo metadata --format-version 1 --no-deps | python3 -c '
import json, sys
want = set(sys.argv[1:])
kinds = {"lib", "rlib", "proc-macro"}
print(" ".join(sorted(p["name"] for p in json.load(sys.stdin)["packages"]
                      if p["name"] in want and any(kinds & set(t["kind"]) for t in p["targets"]))))
' $packages)
if [ -n "$libraries" ]; then
    step cargo test --doc $(printf -- ' -p %s' $libraries)
else
    echo "gate: none of $packages is a library, so there are no doc tests"
fi
echo "gate: passed"
