# Contributing to Tempo

1. Read [AGENTS.md](AGENTS.md) (the rules and the map) and [DESIGN.md](DESIGN.md).
2. `make run` to try it, `make check` before a pull request.
3. A new text: English in `gettext()`, then its French in `po/fr.po`. A new
   colour: in `DESIGN.md` first, then `theme.rs`, whose tests check its contrast
   in both themes.
4. A change to what Tempo writes or runs needs a test, and the real tests
   (`cargo test -p tempo live_ -- --ignored --test-threads=1`) must still pass.
5. Commit messages in English, one change each.

## Releasing

1. Set the version in `Cargo.toml` and `packaging/tempo.spec` (`Version:` and
   a `%changelog` entry), and add its section at the top of `CHANGELOG.md`.
2. Merge that change, then push the tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. `.github/workflows/release.yml` builds the archive, the `.deb` and the
   `.rpm` for amd64 and arm64, checks the versions match, and publishes the
   release with its checksums. `install.sh` always takes the latest release.
