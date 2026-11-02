#!/bin/sh
# Every feature's tests, run after a merge (see FORK.md): the commands in the ```bash block under
# `## Tests` of each fork/*.md, each command once. A new feature adds its tests to its own
# fork/<slug>.md, never here, so two feature branches never clash on this file.
# `fork/tests.sh --list` prints the commands without running them.
# `fork/tests.sh <slug>...` runs only those features' tests (a batch: the features it merged, plus the
# Mac-only ones Fork CI's Linux machines cannot run, FORK.md step 6); `--list` may come first.
set -e
cd "$(dirname "$0")/.."
list=false
if [ "${1:-}" = "--list" ]; then list=true; shift; fi
files="fork/*.md"
if [ $# -gt 0 ]; then
  files=""
  for slug in "$@"; do
    [ -f "fork/$slug.md" ] || { echo "fork/tests.sh: no fork/$slug.md" >&2; exit 2; }
    files="$files fork/$slug.md"
  done
fi
cmds="$(for f in $files; do
  awk '/^## /{t = ($0 == "## Tests")} t && /^```/{b = !b; next} t && b && /^cargo /' "$f"
done | awk '!seen[$0]++')"
if $list; then
  printf '%s\n' "$cmds"
  exit 0
fi
printf '%s\n' "$cmds" | while IFS= read -r c; do
  echo "\$ $c"
  sh -c "$c" </dev/null
done
