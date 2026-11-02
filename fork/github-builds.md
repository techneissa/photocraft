# GitHub builds the app

The workflow `.github/workflows/fork-build.yml` (Actions ▸ Fork build) runs daily, on every push to `custom`, and
by hand. It merges the official update into `custom` the same way the script does, builds the Mac app with the
fonts of the official release (craft-fonts at `release.yml`'s `craft-fonts-ref`), and publishes it as the release
[`custom-build`](https://github.com/techneissa/photocraft/releases/tag/custom-build) (its notes name the commit).
The update script downloads it (above). A run that hits a merge clash fails and changes nothing; GitHub emails the
failure. The run pushes its merge with the secret `FORK_PUSH_TOKEN`, the fine-grained token "BuildCraft forks"
(Contents and Workflows: read and write): GitHub's own token may not push an official update that changes a
workflow file. Without the secret it uses GitHub's own token.

The workflow `.github/workflows/fork-ci.yml` (Actions ▸ Fork CI) runs the official Linux gate (the steps of
`cargo xtask ci`, through `fork/ci.sh`) on every pull request to `custom` and every push to it, the daily merge
included, so the full suite runs on GitHub and not on this Mac (the fork is public, so GitHub's runners are free).
It skips the accepted failures in `fork/known-failures.txt`, so a red run is a new failure: fix it, or add the test
there with a `#` line saying why. A pull request tests only the packages it can affect (`fork/affected.py`: the
changed packages and every package that depends on them; a Markdown-only change runs no Rust checks; a change to
`Cargo.toml`, `Cargo.lock`, CI files or a file outside the packages tests everything); a push to `custom` always tests
the whole workspace. The tests run on three machines at once (`--part 1/3` to `3/3`); the first also runs fmt,
clippy, layers, panic_hunt, doctests and the web check. The official `CI` workflow (`ci.yml`, with Windows and web) is switched off in this fork (2026-11-02): it always
stopped at the scorecard check here, and the fork is Mac only.

The official project's other workflows (Docs, FreeBSD, Nix, Packaging lint, Perf nightly, Release, SonarCloud,
Windows arm64) are switched off in this fork: the official project runs them. A workflow the official project
adds later starts switched on; switch it off in Actions unless it is wanted here.
