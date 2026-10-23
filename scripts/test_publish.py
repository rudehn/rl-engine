"""Tests for scripts/publish.py, none of which touch the network.

    python3 -m unittest scripts/test_publish.py -v
"""

import importlib.util
import pathlib
import unittest

_SCRIPT = pathlib.Path(__file__).with_name("publish.py")
_SPEC = importlib.util.spec_from_file_location("publish", _SCRIPT)
publish = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(publish)

publish_order = publish.publish_order
plan_publish = publish.plan_publish
semver_targets = publish.semver_targets
retry_wait_seconds = publish.retry_wait_seconds


def package(name, deps=(), publishable=True):
    return {
        "name": name,
        "publish": None if publishable else [],
        "dependencies": [{"name": d} for d in deps],
    }


METADATA = {
    "packages": [
        package("atglyph", ["atglyph-grid", "atglyph-core"]),
        package("atglyph-grid", ["atglyph-core"]),
        package("atglyph-core", ["serde"]),
        package("game", ["atglyph"], publishable=False),
    ]
}


class Publish(unittest.TestCase):
    def test_the_order_puts_a_crate_after_everything_it_depends_on(self):
        self.assertEqual(publish_order(METADATA), ["atglyph-core", "atglyph-grid", "atglyph"])

    def test_the_order_breaks_ties_by_name(self):
        meta = {"packages": [package("b"), package("a"), package("c", ["b"])]}
        self.assertEqual(publish_order(meta), ["a", "b", "c"])

    def test_a_dependency_cycle_is_refused(self):
        meta = {"packages": [package("a", ["b"]), package("b", ["a"])]}
        with self.assertRaises(ValueError):
            publish_order(meta)

    def test_a_rerun_skips_what_is_already_published(self):
        order = ["atglyph-core", "atglyph-grid", "atglyph"]
        published = {"atglyph-core": {"0.4.0"}, "atglyph-grid": {"0.3.9"}}
        self.assertEqual(plan_publish(order, "0.4.0", published), ["atglyph-grid", "atglyph"])

    def test_the_first_release_has_nothing_to_check_semver_against(self):
        self.assertEqual(semver_targets(["atglyph-core", "atglyph"], {}), ([], ["atglyph-core", "atglyph"]))
        self.assertEqual(
            semver_targets(["atglyph-core", "atglyph"], {"atglyph-core": {"0.4.0"}}),
            (["atglyph-core"], ["atglyph"]),
        )

    def test_a_failure_that_is_not_the_rate_limit_is_not_retried(self):
        self.assertIsNone(retry_wait_seconds("error: failed to verify package tarball"))

    def test_the_rate_limit_waits_the_time_it_names(self):
        stderr = (
            "error: failed to publish: the remote server responded with an error (status 429 Too Many Requests): "
            "You have published too many new crates in a short period of time. "
            "Please try again after Fri, 23 Oct 2026 10:00:00 GMT"
        )
        # 2026-10-23 09:00:00 GMT
        self.assertEqual(retry_wait_seconds(stderr, now=1792746000), 3605)

    def test_a_rate_limit_that_names_no_time_waits_ten_minutes(self):
        self.assertEqual(retry_wait_seconds("429 Too Many Requests"), 600)


if __name__ == "__main__":
    unittest.main()
