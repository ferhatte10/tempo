# Contributing to Tempo

1. Read [AGENTS.md](AGENTS.md) (the rules and the map) and [DESIGN.md](DESIGN.md).
2. `make run` to try it, `make check` before a pull request.
3. A new text: English in `gettext()`, then its French in `po/fr.po`. A new
   colour: in `DESIGN.md` first, then `theme.rs`, whose tests check its contrast
   in both themes.
4. A change to what Tempo writes or runs needs a test, and the real tests
   (`cargo test -p tempo live_ -- --ignored --test-threads=1`) must still pass.
5. Commit messages in English, one change each.
