#!/usr/bin/env bash
# The gate a commit runs before it is made, scoped to what it can break.
#
#   scripts/gate.sh          the packages the branch touched and every package
#                            that depends on one
#   scripts/gate.sh --all    the code half of CI: the whole workspace, the tier
#                            boundary and the wasm build, for a branch's end
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
# RL_FULL_SEEDS=1 is passed through, for the generation tests' full range.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() {
    echo "gate: $*"
    "$@"
}

if [ "${1:-}" = "--all" ]; then
    step cargo fmt --all --check
    step cargo clippy --workspace --all-targets -- -D warnings
    step cargo nextest run --workspace
    step cargo test --doc --workspace
    step scripts/check-tiers.sh
    step scripts/check-tiers.sh --wasm
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
step cargo clippy --workspace --all-targets -- -D warnings
if [ -z "$packages" ]; then
    echo "gate: the branch touches no package, so no tests run"
    exit 0
fi
echo "gate: testing $packages"
filter=$(printf ' | package(%s)' $packages)
step cargo nextest run --workspace -E "${filter# | }"
step cargo test --doc $(printf -- ' -p %s' $packages)
echo "gate: passed"
