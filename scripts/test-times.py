#!/usr/bin/env python3
"""Reads the times of the last nextest run, and holds it to the suite's budget.

    scripts/test-times.py [--profile default|full|ci] [--top 30]
    scripts/test-times.py --check full|fast|undermere|tests [--profile ...]

nextest writes every test's wall time to target/nextest/<profile>/junit.xml
(.config/nextest.toml asks it to), and scripts/slot, which every test runs
inside, appends every test's processor time to target/nextest/cpu/<run>.tsv.
The report lists the slowest tests by both and each package's totals.

Processor time is the budget, not wall time. A test's wall time grows with
whatever else the machine is running: other agents' builds and tests, and
the suite's own tests, each of which plays its seeds on every core. Its
processor time is the work it does, near enough the same on a quiet machine
and a busy one, and the suite's wall time on a quiet machine is that work
over the cores. So:

- the run's processor time over its budget fails the check: the suite was
  made heavier, whatever the machine was doing;
- its wall time over its budget only warns, since a busy machine explains it;
- any one test past the per-test thresholds is named, and one past the
  per-test limit fails the check;
- a test of the fast tier, nextest's default profile, past two seconds is
  named as one to make cheaper or move to the slow tier.

`--check full` is for a run of the whole workspace with both tiers, which
`scripts/gate.sh --all` runs with `--profile full`; `--check fast` for the
whole workspace's fast tier; `--check undermere` for Undermere's fast tier
alone, which is what a gate on an Undermere branch runs; and `--check
tests` holds any other run to the per-test numbers only. Under RL_FULL_SEEDS every seed is played, so only
the report is given. `--profile ci` reads CI's run against CI's own
numbers: the runner is another machine and CI plays every seed, so there
the check only ever warns. AGENTS.md, "Writing fast tests", says what to do
when the check speaks.
"""

import argparse
import os
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

# Processor and wall seconds for a run, by what it ran. The processor
# budgets are what the branch that set them measured on the 8-core
# development machine on 2026-10-16, both tiers 950 s, the fast tier 413 s
# and Undermere's fast tier alone 251 s, with three tenths more for the
# processor time a heavily loaded machine adds and for the suite to grow
# into. The wall budgets warn only: both tiers took 178 s on a quiet
# machine, and the fast tiers are set in proportion. Raise one only on
# purpose, saying why in the commit, as a fingerprint is re-baselined.
BUDGETS = {
    "full": {"cpu": 1250, "wall": 200},
    "fast": {"cpu": 540, "wall": 120},
    "undermere": {"cpu": 330, "wall": 60},
}
# CI's runner, playing every seed of both tiers: about 2,000 s of processor
# time on the development machine, doubled for a slower runner. Unmeasured
# on the runner itself when set, so a warning only; the first CI runs are
# the baseline to tighten them to.
CI_BUDGETS = {
    "full": {"cpu": 4000, "wall": 1200},
}
# Any one test past either threshold is named; one past the limit fails.
# The limit is twice the most any test took when it was set, the Undermere
# fingerprint's four scripted delves at about 60 s.
TEST_CPU = 20.0
TEST_WALL = 30.0
TEST_CPU_LIMIT = 120.0
# What a test in the fast tier, nextest's default profile, may take before
# it is named as one for the slow tier; the fingerprints stay fast at any
# cost, as the early warning that a change moved play.
FAST_TIER_CPU = 2.0

ROOT = Path(__file__).resolve().parent.parent


def stem(suite):
    """The name a suite's test binary starts with: the crate's for a
    library, the test's or the binary's own for the rest."""
    if "::" in suite:
        return suite.split("::", 1)[1].split("/")[-1]
    return suite.replace("-", "_")


def load(junit):
    root = ET.parse(junit).getroot()
    tests = {}
    for suite in root.iter("testsuite"):
        name = suite.get("name")
        for case in suite.iter("testcase"):
            tests[(stem(name), case.get("name"))] = {"suite": name, "package": name.split("::")[0], "wall": float(case.get("time", 0)), "cpu": None}
    return root.get("uuid"), float(root.get("time", 0)), tests


def read_cpu(target, run, tests):
    """Fills in each test's processor time from what scripts/slot wrote for
    the run; false when it wrote nothing."""
    path = target / "nextest" / "cpu" / f"{run}.tsv"
    if not path.exists():
        return False
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if "--exact" not in fields:
            continue
        binary = Path(fields[1]).name.rsplit("-", 1)[0]
        name = fields[fields.index("--exact") + 1]
        if (binary, name) in tests:
            tests[(binary, name)]["cpu"] = float(fields[0])
    return True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", default="default")
    parser.add_argument("--top", type=int, default=30)
    parser.add_argument("--check", choices=["full", "fast", "undermere", "tests"])
    args = parser.parse_args()
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    junit = target / "nextest" / args.profile / "junit.xml"
    if not junit.exists():
        sys.exit(f"test-times: no {junit}; run cargo nextest first")
    run, wall, tests = load(junit)
    have_cpu = read_cpu(target, run, tests)
    cpu = sum(t["cpu"] or 0 for t in tests.values())

    print(f"test-times: {len(tests)} tests in {wall:.1f} s of wall time", end="")
    print(f", {cpu:.1f} s of processor time" if have_cpu else ", processor time not recorded")
    key = "cpu" if have_cpu else "wall"
    ranked = sorted(tests.items(), key=lambda kv: -(kv[1][key] or 0))
    if args.top:
        print(f"\nslowest {args.top} by {'processor' if have_cpu else 'wall'} time:   cpu     wall")
        for (_, name), t in ranked[: args.top]:
            c = f"{t['cpu']:7.1f}" if t["cpu"] is not None else "      -"
            print(f"  {c} {t['wall']:8.1f}  {t['suite']}  {name}")
        packages = {}
        for t in tests.values():
            p = packages.setdefault(t["package"], [0, 0.0, 0.0])
            p[0] += 1
            p[1] += t["cpu"] or 0
            p[2] += t["wall"]
        print("\nby package:      tests      cpu     wall")
        for package, (count, c, w) in sorted(packages.items(), key=lambda kv: -kv[1][1 if have_cpu else 2]):
            print(f"  {package:14} {count:5} {c:8.1f} {w:8.1f}")

    if not args.check:
        return
    ci = args.profile == "ci"
    full = bool(os.environ.get("RL_FULL_SEEDS")) and not ci
    if full:
        print("test-times: RL_FULL_SEEDS plays every seed, so the everyday budget is not checked")
        return
    budget = (CI_BUDGETS if ci else BUDGETS).get(args.check)
    warn = (lambda text: print(f"::warning::{text}")) if ci else (lambda text: print(f"test-times: warning: {text}"))
    failed = []
    fast_tier = args.profile == "default"
    for (_, name), t in ranked:
        c = t["cpu"] or 0
        if c > TEST_CPU_LIMIT and not ci:
            failed.append(f"{t['suite']} {name} took {c:.1f} s of processor time, past the {TEST_CPU_LIMIT:.0f} s no one test may take")
        elif fast_tier and c > FAST_TIER_CPU and "fingerprint" not in t["suite"]:
            warn(f"{t['suite']} {name} took {c:.1f} s of processor time in the fast tier, past {FAST_TIER_CPU:.0f} s: make it cheaper or name it in the slow tier in .config/nextest.toml")
        elif c > TEST_CPU or t["wall"] > TEST_WALL:
            shown = f"{c:.1f}" if t["cpu"] is not None else "?"
            warn(f"{t['suite']} {name} took {shown} s of processor time and {t['wall']:.1f} s of wall time, past {TEST_CPU:.0f} s or {TEST_WALL:.0f} s")
    if budget:
        if not have_cpu:
            warn("no processor times were recorded, so the run's budget is not checked: is scripts/slot the nextest wrapper?")
        elif cpu > budget["cpu"]:
            text = f"the {args.check} suite took {cpu:.0f} s of processor time, over its budget of {budget['cpu']} s"
            if ci:
                warn(text)
            else:
                failed.append(text)
        if wall > budget["wall"]:
            warn(f"the {args.check} suite took {wall:.0f} s of wall time, over its budget of {budget['wall']} s; on a busy machine that may be the machine")
        print(f"test-times: {cpu:.0f} s of processor time and {wall:.0f} s of wall time against the {args.check} budget of {budget['cpu']} s and {budget['wall']} s")
    for text in failed:
        print(f"test-times: error: {text}; AGENTS.md, 'Writing fast tests', says how to bring it back")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
