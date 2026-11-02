#!/bin/sh
# Fork CI's gate (.github/workflows/fork-ci.yml): the steps of `cargo xtask ci` (`cmd_ci` in
# xtask/src/main.rs) on `custom` and on pull requests to it, minus what fork/known-failures.txt
# accepts, so a red run is a new failure. Official files stay untouched.
#
# A pull request tests only the packages it can affect (fork/affected.py: the changed packages and
# every package that depends on them); a push to `custom` tests the whole workspace. The tests are
# split over parallel jobs with --part; job 1 also runs the other steps.
#
#   fork/ci.sh                   every step, whole workspace
#   fork/ci.sh --part 2/3        the second third of the tests only
#   fork/ci.sh --print           print the commands without running them
#   FORK_CI_FULL=1 fork/ci.sh    whole workspace on a pull request too
#   FORK_CI_SCOPE='photocraft-ui-egui photocraft' fork/ci.sh   only these packages (or `none`)
set -eu
cd "$(dirname "$0")/.."

print=false
part=""
while [ $# -gt 0 ]; do
  case "$1" in
    --print) print=true ;;
    --part) shift; part="$1" ;;
    *) echo "fork/ci.sh: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done
first=true
case "$part" in "" | 1/*) ;; *) first=false ;; esac

# What to test: a pull request's affected packages, else everything.
scope="${FORK_CI_SCOPE:-full}"
if [ -z "${FORK_CI_SCOPE:-}" ] && [ "${GITHUB_EVENT_NAME:-}" = "pull_request" ] && [ -n "${GITHUB_BASE_REF:-}" ] && [ "${FORK_CI_FULL:-}" != 1 ]; then
  if git fetch --quiet --no-tags --depth=1 origin "$GITHUB_BASE_REF" 2>/dev/null; then
    scope="$(git diff --name-only FETCH_HEAD HEAD | python3 -I fork/affected.py)"
  fi
fi
case "$scope" in
  full) GATE="--workspace --locked --features heif"; echo "=== fork ci: scope: the whole workspace" ;;
  none) GATE=""; echo "=== fork ci: scope: no Rust package changed, no Rust checks" ;;
  *) GATE="--locked $(printf -- '-p %s ' $scope)"; echo "=== fork ci: scope: $scope" ;;
esac

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

failed=""
step() {
  name="$1"; shift
  if printf '%s\n' "$skipped_steps" | grep -qx "$name"; then
    echo "=== fork ci: $name: skipped (fork/known-failures.txt)"
    return
  fi
  echo "=== fork ci: $name"
  echo "\$ $*"
  $print && return
  "$@" || failed="$failed $name"
}

tests="--lib --bins --tests --no-tests=pass${part:+ --partition count:$part}"
if $first; then
  step fmt cargo fmt --all -- --check
fi
if [ "$scope" = none ]; then
  $print && exit 0
  [ -z "$failed" ] || { echo "::error::Fork CI failed at:$failed"; exit 1; }
  echo "Fork CI: no Rust package changed"; exit 0
fi
if $first; then
  # shellcheck disable=SC2086
  step clippy cargo clippy $GATE --all-targets --keep-going -- -D warnings
  step layers cargo xtask layers
  step scorecard cargo xtask scorecard --check
fi
if [ -n "$filter" ]; then
  # shellcheck disable=SC2086
  step test cargo nextest run --profile ci $GATE $tests -E "not ($filter)"
else
  # shellcheck disable=SC2086
  step test cargo nextest run --profile ci $GATE $tests
fi
if $first; then
  # shellcheck disable=SC2086
  step panic_hunt cargo nextest run --profile ci $GATE --lib --bins --tests --no-tests=pass --run-ignored only -E 'binary_id(photocraft-engine::panic_hunt)'
  # shellcheck disable=SC2086
  step doctest cargo test $GATE --doc
  step wasm cargo xtask wasm
  # i18n only lists missing translations for the log; it never fails CI.
  $print || cargo xtask i18n-coverage || true
fi

$print && exit 0
if [ -n "$failed" ]; then
  echo "::error::Fork CI failed at:$failed"
  exit 1
fi
echo "Fork CI: every step passed (skipped: $(printf '%s ' $skipped_steps)$(printf '%s\n' "$skipped_tests" | grep -c . || true) known failing tests)"
