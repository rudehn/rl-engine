#!/usr/bin/env python3
"""Publish every crate of the workspace to crates.io, in dependency order.

    python3 scripts/publish.py --plan      print the order and what would publish
    python3 scripts/publish.py --semver    cargo semver-checks each crate that
                                           has a published version to compare to
    python3 scripts/publish.py --publish   cargo publish each crate not yet up

A run cut short, by the new-crate rate limit or a failure, is finished by
running it again: a crate whose version is already on crates.io is skipped.
A crate with `publish = false` is never touched.
"""

import argparse
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

USER_AGENT = "atglyph-release (https://github.com/rudehn/rl-engine)"
DEFAULT_WAIT_SECONDS = 600
# crates.io's new-crate limit lets a burst through and then one crate every
# ten minutes or so, so ten tries a crate is far past any real wait; a misread
# failure stops there instead of retrying until the job's timeout.
MAX_ATTEMPTS = 10


def publish_order(metadata):
    """Every publishable member, each after everything it depends on, ties by name."""
    members = {p["name"]: p for p in metadata["packages"] if p["publish"] != []}
    deps = {
        name: {d["name"] for d in p["dependencies"] if d["name"] in members}
        for name, p in members.items()
    }
    order = []
    done = set()
    while len(order) < len(members):
        ready = sorted(n for n in members if n not in done and deps[n] <= done)
        if not ready:
            left = sorted(set(members) - done)
            raise ValueError(f"a dependency cycle among {', '.join(left)}")
        order.append(ready[0])
        done.add(ready[0])
    return order


def plan_publish(order, version, published):
    """The crates of `order` whose `version` is not already published."""
    return [c for c in order if version not in published.get(c, set())]


def semver_targets(order, published):
    """The crates with a published version to check against, and those without."""
    checked = [c for c in order if published.get(c)]
    skipped = [c for c in order if not published.get(c)]
    return checked, skipped


def retry_wait_seconds(stderr, now=None):
    """Seconds to wait when cargo's stderr is crates.io's rate limit, else None.

    crates.io names the time to retry after, as in "try again after Fri, 23 Oct
    2026 10:00:00 GMT"; when that cannot be read the wait is ten minutes.
    """
    lower = stderr.lower()
    # Both halves, since a bare "429" is also a line number in a compile error.
    status = "status 429" in lower or "too many requests" in lower
    if not (status and "published too many" in lower):
        return None
    match = re.search(r"try again (?:after|at) ([A-Za-z]{3}, \d+ [A-Za-z]{3} \d{4} [\d:]{8}) GMT", stderr)
    if match:
        import calendar

        parsed = time.strptime(match.group(1), "%a, %d %b %Y %H:%M:%S")
        wait = calendar.timegm(parsed) - (time.time() if now is None else now)
        return max(int(wait) + 5, 5)
    return DEFAULT_WAIT_SECONDS


def fetch_published(crate):
    """The versions of `crate` on crates.io; a 404 is a crate never published."""
    request = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{crate}", headers={"User-Agent": USER_AGENT}
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            body = json.load(response)
    except urllib.error.HTTPError as err:
        if err.code == 404:
            return set()
        raise
    return {v["num"] for v in body["versions"] if not v.get("yanked")}


def workspace():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        check=True, capture_output=True, text=True,
    ).stdout
    metadata = json.loads(out)
    order = publish_order(metadata)
    versions = {p["version"] for p in metadata["packages"] if p["name"] in order}
    if len(versions) != 1:
        sys.exit(f"error: the published crates are at {sorted(versions)}, not one version")
    return order, versions.pop()


def run_publish(crate, run=subprocess.run, sleep=time.sleep):
    for attempt in range(1, MAX_ATTEMPTS + 1):
        result = run(["cargo", "publish", "-p", crate], capture_output=True, text=True)
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        if result.returncode == 0:
            return
        wait = retry_wait_seconds(result.stderr)
        if wait is None:
            sys.exit(f"error: cargo publish -p {crate} failed")
        if attempt == MAX_ATTEMPTS:
            break
        print(f"{crate}: crates.io's rate limit, retrying in {wait} s", flush=True)
        sleep(wait)
    sys.exit(f"error: {crate} is still rate limited after {MAX_ATTEMPTS} attempts; re-run the workflow")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--plan", action="store_true")
    mode.add_argument("--semver", action="store_true")
    mode.add_argument("--publish", action="store_true")
    args = parser.parse_args()

    order, version = workspace()
    published = {c: fetch_published(c) for c in order}

    if args.plan:
        print(f"version {version}, in order:")
        for c in order:
            state = "published" if version in published[c] else "would publish"
            print(f"  {c}: {state}")
        todo = plan_publish(order, version, published)
        print(f"{len(todo)} of {len(order)} would publish")
    elif args.semver:
        checked, skipped = semver_targets(order, published)
        for c in skipped:
            print(f"notice: {c} has no published version, so there is nothing to check it against")
        for c in checked:
            if subprocess.run(["cargo", "semver-checks", "-p", c]).returncode != 0:
                sys.exit(f"error: cargo semver-checks failed for {c}")
    else:
        todo = plan_publish(order, version, published)
        if not todo:
            print(f"every crate is already published at {version}")
        for c in todo:
            run_publish(c)


if __name__ == "__main__":
    main()
