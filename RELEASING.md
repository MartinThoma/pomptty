# Releasing pomptty

pomptty uses [Semantic Versioning](https://semver.org/). Releases are cut by
pushing a `vX.Y.Z` tag; `.github/workflows/release.yml` then builds the
`.deb`, `.rpm`, Windows `.exe` and macOS binary and publishes a GitHub
Release with the changelog section as the notes.

## Steps

1. Pick the new version `X.Y.Z`.
2. In one commit, bump all three in lockstep:
   - `version` in `Cargo.toml` (then `cargo build` to refresh `Cargo.lock`),
   - move the `## [Unreleased]` items in `CHANGELOG.md` under a new
     `## [X.Y.Z]` heading and add the compare/tag links at the bottom,
   - add a matching `pomptty (X.Y.Z-1) …` block at the top of
     `packaging/changelog` (the Debian changelog) and bump the `.TH` line
     in `packaging/pomptty.1`.
3. `git tag -a vX.Y.Z -m "pomptty X.Y.Z"` and `git push --follow-tags`.
4. The release workflow's `verify` job fails the run if the tag doesn't
   match `Cargo.toml` or `CHANGELOG.md` has no `## [X.Y.Z]` section — fix
   and re-tag if so.
5. The draft/published Release appears under **Releases** with the four
   artifacts attached.

## Notes

- The tag pattern is exactly `v` + three dot-separated integers
  (`v1.2.3`); pre-release suffixes aren't wired up.
- Nothing is published to crates.io — pomptty is `publish = false`.
