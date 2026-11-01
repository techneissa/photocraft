# GitHub builds the app

The workflow `.github/workflows/fork-build.yml` (Actions ▸ Fork build) runs daily, on every push to `custom`, and
by hand. It merges the official update into `custom` the same way the script does, builds the Mac app with the
fonts of the official release (craft-fonts at `release.yml`'s `craft-fonts-ref`), and publishes it as the release
[`custom-build`](https://github.com/techneissa/photocraft/releases/tag/custom-build) (its notes name the commit).
The update script downloads it (above). A run that hits a merge clash fails and changes nothing; GitHub emails the
failure. The run pushes its merge with the secret `FORK_PUSH_TOKEN`, the fine-grained token "BuildCraft forks"
(Contents and Workflows: read and write): GitHub's own token may not push an official update that changes a
workflow file. Without the secret it uses GitHub's own token.

The workflow `.github/workflows/fork-ci.yml` (Actions ▸ Fork CI) runs the official CI (`ci.yml`, `cargo xtask ci` on
Linux) on `custom` after every push to it, the daily merge included, so the full suite runs on GitHub and not on
this Mac (the fork is public, so GitHub's runners are free). The official `CI` workflow must stay enabled on the fork
for it (`gh workflow enable CI -R techneissa/photocraft`).

The official project's other workflows (Docs, FreeBSD, Nix, Packaging lint, Perf nightly, Release, SonarCloud,
Windows arm64) are switched off in this fork: the official project runs them. A workflow the official project
adds later starts switched on; switch it off in Actions unless it is wanted here.
