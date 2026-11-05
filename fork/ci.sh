#!/bin/sh
# Fork CI's gate (.github/workflows/fork-ci.yml): the steps of `cargo xtask ci` (`cmd_ci` in xtask/src/main.rs) on
# `custom` and on pull requests to it, minus what fork/known-failures.txt accepts, so a red run is a new failure.
# Official files stay untouched.
#
# The work is split over ten machines (fork/affected.py: GROUPS, SHARDS, WEIGHT_OVERRIDES). A push to `custom` tests
# the whole workspace; a pull request tests only the packages it can affect (the changed packages and every package
# that depends on them). Compiling is most of a machine's time, so each machine builds and tests only its own
# packages (`-p`), and the slow packages are cut into shards, one machine per shard (`cargo nextest --partition
# count:i/n`). Machine 1 also runs the one-off steps (format, clippy, layers, wasm); the engine's first shard also
# runs panic_hunt.
#
#   fork/ci.sh --part 1/10          machine 1 of ten: the one-off steps and its share of the tests (the default)
#   fork/ci.sh --part 2/10          the second machine
#   fork/ci.sh --part 7/10 --print  print the commands without running them
#   FORK_CI_FULL=1 fork/ci.sh --part 1/10    the whole workspace on a pull request too
set -eu
cd "$(dirname "$0")/.."

print=false
part="1/10"
while [ $# -gt 0 ]; do
  case "$1" in
    --print) print=true ;;
    --part) shift; part="$1" ;;
    *) echo "fork/ci.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

# What to test: a pull request's affected packages, else everything.
changed=""
full=--full
if [ "${GITHUB_EVENT_NAME:-}" = "pull_request" ] && [ -n "${GITHUB_BASE_REF:-}" ] && [ "${FORK_CI_FULL:-}" != 1 ]; then
  if git fetch --quiet --no-tags --depth=1 origin "$GITHUB_BASE_REF" 2>/dev/null; then
    changed="$(git diff --name-only FETCH_HEAD HEAD)"
    full=""
  fi
fi
# shellcheck disable=SC2086
eval "$(printf '%s\n' "$changed" | python3 -I fork/affected.py $full --sh --part "$part")"
echo "=== fork ci: part $part, scope: $SCOPE${SHARD:+, shard $SHARD}: ${PACKAGES:-no tests}"

known="$(sed -e 's/#.*//' -e 's/[[:space:]]*$//' -e '/^$/d' fork/known-failures.txt)"
skipped_steps="$(printf '%s\n' "$known" | sed -n 's/^step  *//p')"
skipped_tests="$(printf '%s\n' "$known" | grep -v '^step ' || true)"

# The steps this script knows. A step upstream adds to cmd_ci fails the run until it is added here.
known_steps="fmt clippy layers scorecard test panic_hunt doctest wasm i18n"
upstream_steps="$(awk '/^fn cmd_ci\(\)/{f=1} f && /let mut done/{exit} f' xtask/src/main.rs |
  sed -n 's/^[[:space:]]*(\{0,1\}"\([a-z_0-9]*\)",.*/\1/p')"
for s in $upstream_steps; do
  case " $known_steps " in *" $s "*) ;; *)
    echo "::error::cargo xtask ci has a new step '$s': add it to fork/ci.sh"; exit 1 ;;
  esac
done

filter=""
for t in $skipped_tests; do
  filter="${filter:+$filter | }test(=$t)"
done

started=$(date +%s)
failed=""
timings=""
# step <name> <command...>: runs the command, notes a failure and goes on, so one run lists every failing step
step() {
  name="$1"; shift
  if printf '%s\n' "$skipped_steps" | grep -qx "$name"; then
    echo "=== fork ci: $name: skipped (fork/known-failures.txt)"
    return
  fi
  echo "=== fork ci: $name"
  echo "\$ $*"
  $print && return
  t0=$(date +%s)
  "$@" || failed="$failed $name"
  timings="$timings
$name: $(( $(date +%s) - t0 )) s"
}

# Unquoted on purpose below: $CLIPPY_ARGS, $PACKAGES, $FEATURES and the like are lists of arguments (`-p a -p b`).
# shellcheck disable=SC2086
if [ "$ONEOFF" = 1 ]; then
  step affected-tests python3 -I fork/test_affected.py
  step fmt cargo fmt --all -- --check
  if [ "$RUST" = 1 ]; then
    step clippy cargo clippy --locked $CLIPPY_ARGS --all-targets --keep-going -- -D warnings
    step layers cargo xtask layers
    step scorecard cargo xtask scorecard --check
    step wasm cargo xtask wasm
  fi
fi
tests="--lib --bins --tests"
# shellcheck disable=SC2086
if [ -n "$PACKAGES" ]; then
  # the build apart from the run, so a log shows what each cost
  step build cargo test --locked $PACKAGES $FEATURES $tests --no-run
  nextest="cargo nextest run --profile ci --locked $PACKAGES $FEATURES $tests --no-tests=pass${PARTITION:+ --partition $PARTITION}"
  if [ -n "$filter" ]; then
    step test $nextest -E "not ($filter)"
  else
    step test $nextest
  fi
  if [ "$PANIC_HUNT" = 1 ]; then
    step panic_hunt cargo nextest run --profile ci --locked $PACKAGES $FEATURES $tests --no-tests=pass --run-ignored only -E 'binary_id(photocraft-engine::panic_hunt)'
  fi
fi
# shellcheck disable=SC2086
if [ -n "$DOC_PACKAGES" ]; then
  step doctest cargo test --locked $DOC_PACKAGES $FEATURES --doc
fi
if [ "$ONEOFF" = 1 ] && [ "$RUST" = 1 ]; then
  # i18n only lists missing translations for the log; it never fails CI.
  $print || cargo xtask i18n-coverage || true
fi

$print && exit 0
total=$(( $(date +%s) - started ))
summary="Fork CI part $part: $total s (scope: $SCOPE${SHARD:+, shard $SHARD})$timings"
echo "$summary"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  { echo '### Fork CI'; echo '```'; echo "$summary"; echo '```'; } >> "$GITHUB_STEP_SUMMARY"
fi
if [ -n "$failed" ]; then
  echo "::error::Fork CI part $part failed at:$failed"
  exit 1
fi
echo "Fork CI part $part: every step passed (skipped: $(printf '%s ' $skipped_steps)$(printf '%s\n' "$skipped_tests" | grep -c . || true) known failing tests)"
