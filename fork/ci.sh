#!/bin/sh
# Fork CI's gate (.github/workflows/fork-ci.yml): the steps of `cargo xtask ci` (`cmd_ci` in
# xtask/src/main.rs) on `custom` and on pull requests to it, minus what fork/known-failures.txt
# accepts, so a red run is a new failure. Official files stay untouched.
#
#   fork/ci.sh            run every step
#   fork/ci.sh --print    print the commands without running them
set -eu
cd "$(dirname "$0")/.."

GATE="--workspace --locked --features heif"
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

print=false
[ "${1:-}" = "--print" ] && print=true
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

step fmt cargo fmt --all -- --check
# shellcheck disable=SC2086
step clippy cargo clippy $GATE --all-targets --keep-going -- -D warnings
step layers cargo xtask layers
step scorecard cargo xtask scorecard --check
if [ -n "$filter" ]; then
  # shellcheck disable=SC2086
  step test cargo nextest run --profile ci $GATE --lib --bins --tests -E "not ($filter)"
else
  # shellcheck disable=SC2086
  step test cargo nextest run --profile ci $GATE --lib --bins --tests
fi
# shellcheck disable=SC2086
step panic_hunt cargo nextest run --profile ci $GATE --lib --bins --tests --run-ignored only -E 'binary_id(photocraft-engine::panic_hunt)'
# shellcheck disable=SC2086
step doctest cargo test $GATE --doc
step wasm cargo xtask wasm
# i18n only lists missing translations for the log; it never fails CI.
$print || cargo xtask i18n-coverage || true

$print && exit 0
if [ -n "$failed" ]; then
  echo "::error::Fork CI failed at:$failed"
  exit 1
fi
echo "Fork CI: every step passed (skipped: $(printf '%s ' $skipped_steps)$(printf '%s\n' "$skipped_tests" | grep -c . || true) known failing tests)"
