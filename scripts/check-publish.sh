#!/usr/bin/env bash
# Every published crate packages for crates.io.
#
#   scripts/check-publish.sh   checks the manifest fields crates.io shows, that
#                              each internal dependency carries the workspace
#                              version, then runs `cargo package` over them,
#                              which builds every crate from its packaged
#                              files alone (needs cargo 1.90 or later)
#
# A crate with `publish = false` is not checked. A crate that fails to package
# is fixed in the crate, never left out of this check.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo metadata --format-version 1 --no-deps | python3 -I -c '
import json, os, sys

meta = json.load(sys.stdin)
members = {p["name"]: p for p in meta["packages"]}
problems = []
for name, p in sorted(members.items()):
    if p["publish"] == []:
        continue
    for field in ("description", "license", "repository", "readme"):
        if not p.get(field):
            problems.append(f"{name}: `{field}` is not set")
    for field in ("keywords", "categories"):
        if not p.get(field):
            problems.append(f"{name}: `{field}` is empty")
    readme = p.get("readme")
    if readme and not os.path.exists(os.path.join(os.path.dirname(p["manifest_path"]), readme)):
        problems.append(f"{name}: readme `{readme}` does not exist")
    for d in p["dependencies"]:
        if d["name"] in members and d["req"] != "^" + p["version"]:
            dep, req, want = d["name"], d["req"], p["version"]
            problems.append(f"{name}: dependency on {dep} has version requirement `{req}`, not the workspace version {want}")
for line in problems:
    print("error: " + line, file=sys.stderr)
sys.exit(1 if problems else 0)
'

# The games are `publish = false`, so `cargo publish` skips them and so does this:
# only the members the checks above cover are packaged (undermere alone takes minutes).
args=()
while read -r name; do args+=(-p "$name"); done < <(
  cargo metadata --format-version 1 --no-deps | python3 -I -c '
import json, sys
for p in json.load(sys.stdin)["packages"]:
    if p["publish"] != []:
        print(p["name"])
')
cargo package "${args[@]}" --locked
