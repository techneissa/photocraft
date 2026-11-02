#!/usr/bin/env python3
"""Which workspace packages a pull request can affect, for Fork CI (fork/ci.sh).

Reads the changed file paths (one per line) on stdin and prints one of:
  full            test everything (a workspace-wide file changed, or a file outside every package)
  none            no Rust code can be affected (only Markdown outside the packages)
  <pkg> <pkg> ... the changed packages plus every workspace package that depends on them,
                  directly or not (normal, dev and build dependencies)

Run from the repository root: `git diff --name-only BASE HEAD | python3 -I fork/affected.py`.
"""

import json
import os
import subprocess
import sys

# Outside the packages, these change how everything builds or is checked: test everything.
WORKSPACE_WIDE = (
    "Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml", "clippy.toml", "rustfmt.toml",
    ".cargo/", ".config/", ".github/", "fork/ci.sh", "fork/affected.py", "fork/known-failures.txt",
)


def main() -> None:
    changed = [line.strip() for line in sys.stdin if line.strip()]
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
        check=True, capture_output=True, text=True,
    ).stdout)
    root = os.path.realpath(meta["workspace_root"])
    members = set(meta["workspace_members"])
    packages = [p for p in meta["packages"] if p["id"] in members]
    # Package folder (relative to the root, with a trailing slash) -> name; longest first.
    dirs = sorted(
        ((os.path.relpath(os.path.dirname(os.path.realpath(p["manifest_path"])), root) + "/", p["name"]) for p in packages),
        key=lambda d: -len(d[0]),
    )
    by_dir = {os.path.dirname(os.path.realpath(p["manifest_path"])): p["name"] for p in packages}
    # Reverse dependencies among the workspace's own packages.
    users = {p["name"]: set() for p in packages}
    for p in packages:
        for d in p["dependencies"]:
            path = d.get("path")
            dep = by_dir.get(os.path.realpath(path)) if path else None
            if dep and dep != p["name"]:
                users[dep].add(p["name"])

    hit = set()
    for f in changed:
        if f in WORKSPACE_WIDE or any(f.startswith(w) for w in WORKSPACE_WIDE if w.endswith("/")):
            print("full")
            return
        owner = next((name for d, name in dirs if f.startswith(d)), None)
        if owner:
            hit.add(owner)
        elif not f.endswith(".md"):
            # Outside every package and not a note: it may be read by a build script or include_*!.
            print("full")
            return
    if not hit:
        print("none")
        return
    todo, seen = list(hit), set(hit)
    while todo:
        for u in users.get(todo.pop(), ()):
            if u not in seen:
                seen.add(u)
                todo.append(u)
    print(" ".join(sorted(seen)))


if __name__ == "__main__":
    main()
