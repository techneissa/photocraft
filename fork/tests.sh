#!/bin/sh
# Every feature's tests, run after a merge (see FORK.md): the commands in the ```bash block under
# `## Tests` of each fork/*.md, each command once. A new feature adds its tests to its own
# fork/<slug>.md, never here, so two feature branches never clash on this file.
# `fork/tests.sh --list` prints the commands without running them.
set -e
cd "$(dirname "$0")/.."
cmds="$(for f in fork/*.md; do
  awk '/^## /{t = ($0 == "## Tests")} t && /^```/{b = !b; next} t && b && /^cargo /' "$f"
done | awk '!seen[$0]++')"
if [ "${1:-}" = "--list" ]; then
  printf '%s\n' "$cmds"
  exit 0
fi
printf '%s\n' "$cmds" | while IFS= read -r c; do
  echo "\$ $c"
  sh -c "$c" </dev/null
done
