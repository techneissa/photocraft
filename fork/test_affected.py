#!/usr/bin/env python3
"""Tests of fork/affected.py: `python3 -I fork/test_affected.py` (no cargo needed, no network)."""

import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))

import affected  # noqa: E402


def pkg(dir_, deps=(), doc=False, feature=False):
    return {"dir": dir_ + "/", "deps": set(deps), "doc": doc, "feature": feature}


# a small workspace shaped like PhotoCraft: geom <- raster <- codecs <- io <- engine <- ui-egui <- app
PKGS = {
    "photocraft-geom": pkg("crates/geom"),
    "photocraft-raster": pkg("crates/raster", ["photocraft-geom"]),
    "photocraft-codecs": pkg("crates/codecs", ["photocraft-raster"], feature=True),
    "photocraft-io": pkg("crates/io", ["photocraft-codecs"], doc=True, feature=True),
    "photocraft-testkit": pkg("crates/testkit"),
    "photocraft-engine": pkg("crates/engine", ["photocraft-io", "photocraft-testkit"]),
    "photocraft-ui-egui": pkg("crates/ui-egui", ["photocraft-engine", "photocraft-testkit"]),
    "photocraft": pkg("apps/photocraft", ["photocraft-ui-egui"], feature=True),
    "xtask": pkg("xtask"),
}
WEIGHTS = {n: 10 for n in PKGS} | {"photocraft-ui-egui": 1000, "photocraft-engine": 500}


def affects(*paths):
    return affected.affected(list(paths), PKGS)


def plan(*paths, full=True, groups=affected.GROUPS):
    scope, names = affected.affected(list(paths), PKGS, force_full=full)
    return affected.plan(names, PKGS, scope, groups=groups, weights=WEIGHTS)


class Scope(unittest.TestCase):
    def test_a_leaf_package_tests_itself_and_its_users(self):
        self.assertEqual(affects("crates/ui-egui/src/lib.rs"), ("some", ["photocraft", "photocraft-ui-egui"]))

    def test_a_low_package_tests_every_package_above_it(self):
        scope, names = affects("crates/raster/src/lib.rs")
        self.assertEqual(scope, "some")
        self.assertEqual(
            names,
            ["photocraft", "photocraft-codecs", "photocraft-engine", "photocraft-io", "photocraft-raster", "photocraft-ui-egui"],
        )

    def test_a_dev_dependency_counts(self):
        self.assertIn("photocraft-ui-egui", affects("crates/testkit/src/lib.rs")[1])

    def test_workspace_wide_files_test_everything(self):
        for path in ["Cargo.lock", "Cargo.toml", ".cargo/config.toml", ".config/nextest.toml", ".github/workflows/fork-ci.yml", "fork/known-failures.txt", "fork/ci.sh", "fork/affected.py"]:
            self.assertEqual(affects(path), ("full", sorted(PKGS)), path)

    def test_notes_run_nothing_but_the_format_check(self):
        for path in ["docs/foo.md", "FORK.md", "fork/some-feature.md", "README.md"]:
            self.assertEqual(affects(path), ("none", []), path)

    def test_a_file_outside_every_package_that_is_not_markdown_tests_everything(self):
        for path in ["fork/tests.sh", "assets/logo.png", "LICENSE-MIT"]:
            self.assertEqual(affects(path)[0], "full", path)

    def test_markdown_inside_a_package_counts_for_the_package(self):
        self.assertEqual(affects("crates/geom/README.md")[0], "some")

    def test_the_innermost_package_owns_a_file(self):
        pkgs = dict(PKGS, **{"nested": pkg("crates/raster/nested")})
        scope, names = affected.affected(["crates/raster/nested/a.rs"], pkgs)
        self.assertEqual((scope, names), ("some", ["nested"]))

    def test_a_push_tests_everything(self):
        self.assertEqual(affected.affected([], PKGS, force_full=True), ("full", sorted(PKGS)))


class Groups(unittest.TestCase):
    def test_every_package_is_tested_once_and_every_group_exists(self):
        r = plan()
        self.assertEqual([t["group"] for t in r["tests"]], list(range(1, affected.GROUPS + 1)))
        seen = [c for t in r["tests"] for c in t["crates"] if c not in affected.SHARDS]
        self.assertEqual(sorted(seen), sorted(set(PKGS) - set(affected.SHARDS)))

    def test_only_group_one_runs_the_one_off_steps(self):
        self.assertEqual([t["group"] for t in plan()["tests"] if t["oneoff"]], [1])

    def test_groups_are_balanced(self):
        r = plan()
        plain = [t["weight"] for t in r["tests"] if not t["shard"]]
        self.assertLessEqual(max(plain) - min(plain), max(max(WEIGHTS.values()), affected.ONEOFF_WEIGHT))

    def test_the_seed_starts_group_one(self):
        groups = affected.balance({"a": 10, "b": 10}, 3, ("+oneoff", 50))
        self.assertEqual(groups[0], (50, ["+oneoff"]))

    def test_the_heaviest_package_is_alone_when_the_others_are_light(self):
        self.assertIn((100, ["big"]), affected.balance({"big": 100, "a": 10, "b": 10, "c": 10}, 3))

    def test_balance_is_deterministic(self):
        w = {"a": 5, "b": 5, "c": 5, "d": 5}
        self.assertEqual(affected.balance(w, 3), affected.balance(dict(reversed(list(w.items()))), 3))

    def test_doc_tests_go_to_the_packages_that_have_them(self):
        docs = [n for t in plan()["tests"] for n in t["doc_packages"]]
        self.assertEqual(docs, ["photocraft-io"])

    def test_a_single_package_still_fills_group_one_with_the_one_off_steps(self):
        pkgs = {"only": pkg("crates/only")}
        r = affected.plan(["only"], pkgs, "full", weights={"only": 5})
        self.assertEqual(r["tests"][0]["oneoff"], True)
        self.assertEqual(sum(len(t["crates"]) for t in r["tests"]), 1)

    def test_notes_only_runs_group_one_alone(self):
        r = plan("docs/a.md", full=False)
        self.assertEqual(r["scope"], "none")
        self.assertEqual([t["crates"] for t in r["tests"] if t["crates"]], [])


class Shards(unittest.TestCase):
    def shards_of(self, r, name):
        return [(t["group"], t["shard"]) for t in r["tests"] if name in t["crates"] and t["shard"]]

    def test_a_sharded_package_runs_every_shard_exactly_once(self):
        r = plan()
        for name, k in affected.SHARDS.items():
            got = self.shards_of(r, name)
            self.assertEqual(sorted(s for _, s in got), sorted(f"{i}/{k}" for i in range(1, k + 1)), name)

    def test_a_shard_has_its_machine_to_itself(self):
        r = plan()
        for t in r["tests"]:
            if t["shard"]:
                self.assertEqual(len(t["crates"]), 1)
                self.assertFalse(t["oneoff"])

    def test_the_shards_take_the_last_groups(self):
        r = plan()
        shards = [t["group"] for t in r["tests"] if t["shard"]]
        self.assertEqual(shards, list(range(affected.GROUPS - len(shards) + 1, affected.GROUPS + 1)))

    def test_a_small_run_leaves_a_machine_for_the_rest(self):
        r = plan(groups=3)
        plain = [t for t in r["tests"] if not t["shard"]]
        self.assertGreaterEqual(len(plain), 1)
        self.assertEqual(len(r["tests"]), 3)
        self.assertTrue(plain[0]["oneoff"])

    def test_one_machine_runs_the_sharded_packages_unsplit(self):
        r = plan(groups=1)
        self.assertEqual([t["shard"] for t in r["tests"]], [""])
        self.assertIn("photocraft-ui-egui", r["tests"][0]["crates"])

    def test_a_sharded_package_runs_its_doc_tests_with_shard_one_only(self):
        pkgs = dict(PKGS, **{"photocraft-ui-egui": pkg("crates/ui-egui", ["photocraft-engine"], doc=True)})
        r = affected.plan(sorted(pkgs), pkgs, "full", weights=WEIGHTS)
        docs = [(t["shard"]) for t in r["tests"] if "photocraft-ui-egui" in t["doc_packages"]]
        self.assertEqual(docs, [f"1/{affected.SHARDS['photocraft-ui-egui']}"])

    def test_panic_hunt_runs_once_with_the_engines_first_shard(self):
        r = plan()
        hunts = [t for t in r["tests"] if t["panic_hunt"]]
        self.assertEqual(len(hunts), 1)
        self.assertEqual(hunts[0]["crates"], ["photocraft-engine"])
        self.assertTrue(hunts[0]["shard"].startswith("1/"))

    def test_panic_hunt_follows_the_engine_when_it_is_not_sharded(self):
        r = plan(groups=1)
        self.assertTrue(r["tests"][0]["panic_hunt"])

    def test_a_shard_weighs_its_compile_plus_its_share_of_the_tests(self):
        self.assertEqual(affected.shard_weight("photocraft-ui-egui", 1025, 4), 600 + 425 / 4)
        self.assertEqual(affected.shard_weight("unknown", 100, 4), 25)


class Features(unittest.TestCase):
    def test_a_machine_gets_the_heif_feature_of_every_package_it_builds(self):
        r = plan()
        engine = next(t for t in r["tests"] if t["crates"] == ["photocraft-engine"])
        self.assertEqual(engine["features"], ["photocraft-codecs/heif", "photocraft-io/heif"])  # engine builds io and codecs

    def test_no_feature_no_flag(self):
        self.assertEqual(affected.features_for(["photocraft-geom"], PKGS), [])

    def test_a_package_with_the_feature_gets_its_own(self):
        self.assertIn("photocraft/heif", affected.features_for(["photocraft"], PKGS))

    def test_clippy_runs_the_whole_workspace_on_a_push(self):
        self.assertEqual(affected.clippy_args(plan(), PKGS), "--workspace --features heif")

    def test_clippy_on_a_subset_names_its_packages_and_features(self):
        r = plan("crates/geom/src/a.rs", full=False)
        self.assertTrue(affected.clippy_args(r, PKGS).startswith("-p photocraft"))
        self.assertIn("--features", affected.clippy_args(r, PKGS))


class Input(unittest.TestCase):
    def test_plain_paths(self):
        self.assertEqual(affected.parse_changed("a/b.rs\n\n c.md \n"), ["a/b.rs", "c.md"])

    def test_name_status_lines(self):
        out = "M\tcrates/a/x.rs\nD\told.png\nR100\tfrom.rs\tto.rs\nA\tnew.rs\n"
        self.assertEqual(affected.parse_changed(out), ["crates/a/x.rs", "old.png", "from.rs", "to.rs", "new.rs"])

    def test_empty_diff(self):
        self.assertEqual(affected.parse_changed(""), [])

    def test_parts(self):
        self.assertEqual(affected.parse_part("3/10"), (3, 10))
        self.assertEqual(affected.parse_part("2"), (2, affected.GROUPS))


class ShellVars(unittest.TestCase):
    def vars(self, part, *paths, full=True):
        r = plan(*paths, full=full)
        out = affected.shell_vars(r, part, PKGS)
        return dict(line.split("=", 1) for line in out.splitlines())

    def test_group_one_runs_the_one_off_steps_and_the_others_do_not(self):
        self.assertEqual(self.vars(1)["ONEOFF"], "1")
        self.assertEqual(self.vars(2)["ONEOFF"], "0")
        self.assertEqual(self.vars(affected.GROUPS)["ONEOFF"], "0")

    def test_a_shard_gets_its_partition(self):
        v = self.vars(affected.GROUPS)
        self.assertEqual(v["PARTITION"], f"count:{affected.SHARDS['photocraft-ui-egui']}/{affected.SHARDS['photocraft-ui-egui']}")
        self.assertEqual(v["PACKAGES"], "'-p photocraft-ui-egui'")

    def test_notes_only_has_no_tests_and_no_rust(self):
        v = self.vars(1, "docs/a.md", full=False)
        self.assertEqual((v["SCOPE"], v["RUST"], v["PACKAGES"], v["ONEOFF"]), ("none", "0", "''", "1"))

    def test_a_subset_scope_lists_its_packages(self):
        self.assertEqual(self.vars(1, "crates/ui-egui/src/a.rs", full=False)["SCOPE"], "'photocraft photocraft-ui-egui'")

    def test_a_machine_that_does_not_exist_runs_nothing(self):
        v = self.vars(affected.GROUPS, "xtask/src/a.rs", full=False)
        self.assertEqual((v["PACKAGES"], v["ONEOFF"]), ("''", "0"))


class Drift(unittest.TestCase):
    """The workflow and the steps of the official gate must stay equal to what the plan and fork/ci.sh assume."""

    def test_the_workflow_runs_every_group(self):
        wf = (ROOT / ".github/workflows/fork-ci.yml").read_text(encoding="utf-8")
        parts = re.search(r"part: \[([0-9, ]+)\]", wf)
        self.assertIsNotNone(parts)
        self.assertEqual([int(x) for x in parts.group(1).split(",")], list(range(1, affected.GROUPS + 1)))
        self.assertIn("--part ${{ matrix.part }}/%d" % affected.GROUPS, wf)

    def test_the_workflow_builds_without_debug_info(self):
        wf = (ROOT / ".github/workflows/fork-ci.yml").read_text(encoding="utf-8")
        self.assertRegex(wf, r'dev-debug: "0"')

    def test_xtask_ci_steps_are_the_ones_fork_ci_sh_runs(self):
        """A step upstream adds to `cargo xtask ci` has to be added to fork/ci.sh by hand (the script also fails then)."""
        src = (ROOT / "xtask/src/main.rs").read_text(encoding="utf-8")
        body = src.split("fn cmd_ci()", 1)[1].split("let mut done", 1)[0]
        names = set(re.findall(r'^\s*\(?"([a-z_0-9]+)",', body, re.M))
        script = (ROOT / "fork/ci.sh").read_text(encoding="utf-8")
        known = re.search(r'known_steps="([^"]+)"', script).group(1).split()
        self.assertTrue(names <= set(known), names - set(known))

    def test_the_sharded_packages_exist_in_the_weights(self):
        for name in affected.SHARDS:
            self.assertIn(name, affected.WEIGHT_OVERRIDES)

    def test_the_real_workspace_has_every_package_the_tables_name(self):
        meta = affected.load_metadata()
        pkgs = affected.workspace_packages(meta)
        for name in list(affected.SHARDS) + list(affected.WEIGHT_OVERRIDES):
            self.assertIn(name, pkgs)

    def test_the_real_workspace_plan_covers_every_package_once(self):
        pkgs = affected.workspace_packages(affected.load_metadata())
        scope, names = affected.affected([], pkgs, force_full=True)
        r = affected.plan(names, pkgs, scope)
        seen = sorted(c for t in r["tests"] for c in t["crates"] if not t["shard"])
        self.assertEqual(seen, sorted(set(pkgs) - set(affected.SHARDS)))
        self.assertEqual(len(r["tests"]), affected.GROUPS)


if __name__ == "__main__":
    unittest.main()
