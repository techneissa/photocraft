#!/usr/bin/env python3
"""Which workspace packages a run can affect, and how Fork CI splits them over machines (fork/ci.sh).

Reads the changed file paths (one per line) on stdin and prints what to run:

    git diff --name-only BASE HEAD | python3 -I fork/affected.py               # `full`, `none` or the packages
    git diff --name-only BASE HEAD | python3 -I fork/affected.py --sh --part 2/10   # shell variables for fork/ci.sh
    python3 -I fork/affected.py --full --summary                               # the whole plan, for a check by hand

A change tests the packages with a changed file plus every workspace package that depends on one of them
(normal, dev and build dependencies, from `cargo metadata`). A workspace-wide file, or a file outside every package
that is not Markdown, tests every package; Markdown outside the packages runs the format check and nothing else.
A push to `custom` passes --full: everything.

The tests are split over --part N/T machines ("groups"), so the slowest one is as short as it can be. Compiling is
most of a machine's time (a package's tests need the package and everything below it), so a package is never
spread thinly: the slow packages (SHARDS) are cut into shards, one shard per machine, by `cargo nextest
--partition count:i/n`, and the other packages share the remaining machines by weight. Group 1 also runs the
one-off steps (format, lint, layers, wasm). Tests: `python3 -I fork/test_affected.py`. Why and how: fork/github-builds.md.
"""

import argparse
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Outside the packages, these change how everything builds or is checked: test everything.
WORKSPACE_WIDE = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", "clippy.toml", "rustfmt.toml",
    ".cargo/", ".config/", ".github/", "fork/ci.sh", "fork/affected.py", "fork/test_affected.py", "fork/known-failures.txt",
)

# Seconds one machine needs for a package: the part of the compile only it needs (the shared libraries below it are
# compiled by every machine anyway) plus its tests on 4 cores (a third of the tests ran in 308 s on the first run
# of 2026-11-03: 4911 tests, 3935 s of test time in all). A package missing here counts DEFAULT_WEIGHT. Measured from
# the Fork CI runs of 2026-11-03 and 2026-11-05 (fork/github-builds.md, "Measured").
WEIGHT_OVERRIDES = {
    "photocraft-engine": 540,
    "photocraft-ui-egui": 1025,
    "photocraft-algo": 200,
    "photocraft-io": 170,
    "photocraft-gpu": 170,
    "photocraft-compose": 90,
    "photocraft-format": 60,
    "photocraft-codecs": 70,
    "photocraft": 300,
    "photocraft-cli": 150,
    "photocraft-web": 150,
    "photocraft-web-boot": 150,
    "photocraft-automation": 60,
    "photocraft-psd": 40,
    "xtask": 90,
}
DEFAULT_WEIGHT = 30
# The compile part of a sharded package's weight: every shard pays it again, only the test part divides by the shard
# count (ui-egui's lib tests compile for about ten minutes; its 5000 tests need 7 on four cores).
COMPILE_WEIGHT = {"photocraft-ui-egui": 600, "photocraft-engine": 250}

# Packages whose tests are cut into shards, one machine per shard (nextest `--partition count:i/n` takes every n-th
# test of the package's test binaries). A shard still compiles the package, so more shards than this buy little.
SHARDS = {"photocraft-ui-egui": 4, "photocraft-engine": 2}

# Machines (groups) a run uses when --part gives no total; .github/workflows/fork-ci.yml's matrix must match.
# The other packages and the one-off steps take the first groups, the shards the last ones.
GROUPS = 10

# The one-off steps of group 1 (format, clippy on the planned packages, layers, wasm): about 2 minutes warm.
ONEOFF_WEIGHT = 150

# The workspace feature `cargo test --workspace --features heif` turns on, for every package that has it.
FEATURE = "heif"


def run(cmd, cwd=ROOT):
    out = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    if out.returncode != 0:
        sys.exit(f"{' '.join(cmd)}: {out.stderr.strip()}")
    return out.stdout


def load_metadata(path=None):
    text = Path(path).read_text(encoding="utf-8") if path else run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"])
    return json.loads(text)


def workspace_packages(meta, root=ROOT):
    """The workspace packages: name -> folder (relative, with a trailing slash), workspace dependencies, doc-test support,
    and whether it declares the FEATURE."""
    root = os.path.realpath(meta.get("workspace_root") or root)
    members = set(meta["workspace_members"])
    pkgs, by_dir = {}, {}
    for p in meta["packages"]:
        if p["id"] not in members:
            continue
        folder = os.path.realpath(os.path.dirname(p["manifest_path"]))
        by_dir[folder] = p["name"]
        has_doctests = any(
            t.get("doctest", False) and any(k in ("lib", "rlib", "proc-macro") for k in t["kind"]) for t in p["targets"]
        )
        pkgs[p["name"]] = {
            "dir": os.path.relpath(folder, root).replace(os.sep, "/") + "/",
            "deps": set(),
            "doc": has_doctests,
            "feature": FEATURE in p["features"],
            "_raw": p["dependencies"],
        }
    for name, p in pkgs.items():
        for d in p.pop("_raw"):
            path = d.get("path")
            dep = by_dir.get(os.path.realpath(path)) if path else None
            if dep and dep != name:
                p["deps"].add(dep)
    return pkgs


def users_of(pkgs):
    users = {n: set() for n in pkgs}
    for n, p in pkgs.items():
        for d in p["deps"]:
            users[d].add(n)
    return users


def closure(names, pkgs):
    """`names` plus everything they depend on, directly or not."""
    seen, todo = set(names), list(names)
    while todo:
        for d in pkgs[todo.pop()]["deps"]:
            if d not in seen:
                seen.add(d)
                todo.append(d)
    return seen


def affected(changed, pkgs, force_full=False):
    """(scope, packages): scope is `full`, `none` or `some`; packages the sorted names to test."""
    everything = sorted(pkgs)
    if force_full:
        return "full", everything
    dirs = sorted(((p["dir"], n) for n, p in pkgs.items()), key=lambda d: -len(d[0]))
    hit = set()
    for f in changed:
        if f in WORKSPACE_WIDE or any(f.startswith(w) for w in WORKSPACE_WIDE if w.endswith("/")):
            return "full", everything
        owner = next((name for d, name in dirs if f.startswith(d)), None)
        if owner:
            hit.add(owner)
        elif not f.endswith(".md"):
            # Outside every package and not a note: a build script or include_*! may read it.
            return "full", everything
    if not hit:
        return "none", []
    users, seen, todo = users_of(pkgs), set(hit), list(hit)
    while todo:
        for u in users.get(todo.pop(), ()):
            if u not in seen:
                seen.add(u)
                todo.append(u)
    return "some", sorted(seen)


def balance(weights, groups, seed=None):
    """Spread the packages over `groups` machines, heaviest first onto the lightest machine. `seed` is (name, weight)
    already on machine 1. Returns [(weight, [names])], deterministic."""
    out = [[0, []] for _ in range(groups)]
    if seed:
        out[0][0] += seed[1]
        out[0][1].append(seed[0])
    for name, w in sorted(weights.items(), key=lambda kv: (-kv[1], kv[0])):
        lightest = min(range(groups), key=lambda i: (out[i][0], i))
        out[lightest][0] += w
        out[lightest][1].append(name)
    return [(w, names) for w, names in out]


def shard_counts(packages, groups):
    """How many shards (machines) each sharded package in `packages` gets: SHARDS, cut down evenly until at least one
    machine is left for everything else. A package left with one shard is not sharded."""
    counts = {n: SHARDS[n] for n in sorted(packages) if n in SHARDS}
    while counts and sum(counts.values()) > groups - 1:
        big = max(counts, key=lambda n: (counts[n], n))
        counts[big] -= 1
    # A lone package cannot take every machine: keep one for the rest (done above); one shard means no split.
    return {n: k for n, k in counts.items() if k >= 2}


def shard_weight(name, weight, k):
    compile_part = min(COMPILE_WEIGHT.get(name, 0), weight)
    return compile_part + (weight - compile_part) / k


def features_for(names, pkgs):
    """`pkg/heif` for every package that has the feature and is in the closure of `names`: a subset build turns on what
    the whole-workspace build turns on, or the tests of the packages above `photocraft-io` would see a different io."""
    return [f"{n}/{FEATURE}" for n in sorted(closure(names, pkgs)) if pkgs[n]["feature"]]


def doc_features_for(names, pkgs):
    """The features a doc-test run may name: cargo takes `pkg/feature` only for a selected package or a direct
    dependency of one, so the deeper part of `features_for` (and a package without a library, like the app) is left out."""
    near = set(names).union(*(pkgs[n]["deps"] for n in names)) if names else set()
    return [f"{n}/{FEATURE}" for n in sorted(near) if pkgs[n]["feature"]]


def plan(scope_pkgs, pkgs, scope="full", groups=GROUPS, weights=None):
    """The machines: a list of dicts, group 1 first. Each has `crates` (the packages it tests), `shard` (`i/n` or ``),
    `oneoff` (runs the one-off steps), `panic_hunt`, `features`, `doc_packages`, `weight`."""
    w = dict(weights if weights is not None else {n: WEIGHT_OVERRIDES.get(n, DEFAULT_WEIGHT) for n in pkgs})
    counts = shard_counts(scope_pkgs, groups)
    others = [n for n in scope_pkgs if n not in counts]
    shard_groups = sum(counts.values())
    plain = groups - shard_groups
    tests = []
    for i, (weight, names) in enumerate(balance({n: w.get(n, DEFAULT_WEIGHT) for n in others}, plain, ("+oneoff", ONEOFF_WEIGHT))):
        names = [n for n in names if n != "+oneoff"]
        tests.append({"group": i + 1, "crates": sorted(names), "shard": "", "oneoff": i == 0, "weight": weight})
    for name, k in counts.items():
        for s in range(1, k + 1):
            tests.append({"group": len(tests) + 1, "crates": [name], "shard": f"{s}/{k}", "oneoff": False, "weight": shard_weight(name, w.get(name, DEFAULT_WEIGHT), k)})
    for t in tests:
        index, _, count = t["shard"].partition("/")
        first = index in ("", "1")
        # panic_hunt is an ignored test of the engine: it runs where the engine's first (or only) shard does.
        t["panic_hunt"] = "photocraft-engine" in t["crates"] and first
        t["features"] = features_for(t["crates"], pkgs) if t["crates"] else []
        # Doc tests of a sharded package run once, with its first shard; the build is the tests' own, so they are cheap.
        t["doc_packages"] = [n for n in t["crates"] if pkgs[n]["doc"] and first]
    return {"scope": scope, "crates": sorted(scope_pkgs), "tests": tests}


def parse_part(text):
    n, _, total = text.partition("/")
    return int(n), int(total) if total else GROUPS


def pkg_args(names):
    return " ".join(f"-p {n}" for n in names)


def clippy_args(result, pkgs):
    if result["scope"] == "full":
        return f"--workspace --features {FEATURE}"
    feats = features_for(result["crates"], pkgs) if result["crates"] else []
    return pkg_args(result["crates"]) + (" --features " + ",".join(feats) if feats else "")


def shell_vars(result, part, pkgs):
    """KEY=VALUE lines for `eval` in fork/ci.sh: what machine `part` runs."""
    t = next((t for t in result["tests"] if t["group"] == part), None)
    rust = result["scope"] != "none"
    scope = result["scope"] if result["scope"] != "some" else " ".join(result["crates"])
    v = {
        "SCOPE": scope,
        "RUST": "1" if rust else "0",
        "ONEOFF": "1" if t and t["oneoff"] else "0",
        "PACKAGES": pkg_args(t["crates"]) if t else "",
        "PARTITION": f"count:{t['shard']}" if t and t["shard"] else "",
        "FEATURES": ("--features " + ",".join(t["features"])) if t and t["features"] else "",
        "DOC_PACKAGES": pkg_args(t["doc_packages"]) if t else "",
        "DOC_FEATURES": ("--features " + ",".join(doc_features_for(t["doc_packages"], pkgs))) if t and doc_features_for(t["doc_packages"], pkgs) else "",
        "PANIC_HUNT": "1" if t and t["panic_hunt"] else "0",
        "SHARD": t["shard"] if t else "",
        # clippy runs in group 1 over everything in scope
        "CLIPPY_ARGS": clippy_args(result, pkgs),
    }
    return "\n".join(f"{k}={shlex.quote(str(val))}" for k, val in v.items())


def parse_changed(text):
    """Paths from plain lines or `git diff --name-status` lines (`M<TAB>path`, `R100<TAB>old<TAB>new`)."""
    out = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split("\t")
        out.extend(parts[1:] if len(parts) > 1 and parts[0][:1] in "MADRCTU" else parts[:1])
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--full", action="store_true", help="a push: test everything")
    ap.add_argument("--sh", action="store_true", help="print shell variables for one machine (needs --part)")
    ap.add_argument("--part", default=f"1/{GROUPS}")
    ap.add_argument("--summary", action="store_true", help="print the whole plan")
    ap.add_argument("--metadata", help="a saved `cargo metadata --no-deps` file instead of running cargo")
    args = ap.parse_args()
    pkgs = workspace_packages(load_metadata(args.metadata))
    changed = [] if args.full else parse_changed(sys.stdin.read())
    scope, names = affected(changed, pkgs, force_full=args.full)
    part, total = parse_part(args.part)
    result = plan(names, pkgs, scope, groups=total)
    if args.sh:
        print(shell_vars(result, part, pkgs))
    elif args.summary:
        print(f"scope: {scope}")
        for t in result["tests"]:
            extra = (" +one-off steps" if t["oneoff"] else "") + (" +panic_hunt" if t["panic_hunt"] else "")
            print(f"group {t['group']:2}: {t['weight']:6.0f} s  {t['shard'] or '-':4} {' '.join(t['crates']) or '(nothing)'}{extra}")
    else:
        print(scope if scope != "some" else " ".join(names))


if __name__ == "__main__":
    main()
