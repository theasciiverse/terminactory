# Changes from upstream

This is the Asciiverse packaging of
[Terminactory](https://gitlab.com/snipedbywifi/terminactory) by
snipedbywifi (Apache-2.0, `LICENSE`), a cosy idle factory game for the
terminal in Rust and Ratatui, with an in-game browser full of written
lore. Upstream is the `upstream` remote; this build is upstream's `main`
at `6d76220` (2026-11-15), version 0.1.0 (not yet released upstream), so
our tags read `v0.1.0+20261115-av.N`. The `asciiverse` branch carries the
changes below, one commit each.

## Code

- **State folder.** Saves, blueprints, `controls.json`, `settings.json`
  and `glyphs.json` live in `$XDG_STATE_HOME/terminactory`, else
  `~/.local/state/terminactory` on Linux and the BSDs. macOS and Windows
  keep upstream's `~/.terminactory` unless `XDG_STATE_HOME` is set. An
  upstream `~/.terminactory` is moved into the state folder the first
  time the game runs (copied instead when a rename would cross file
  systems), so saves carry on. `tfac --help` names the folder; the
  blueprint library's title and the help screen no longer spell out
  `~/.terminactory`. (`src/lib.rs`, `src/ui.rs`, `src/ui/library.rs`)
- **Agent mail behind a feature.** Upstream's `tfac msg` command, its
  `tfac hook` command for Claude Code's Stop and Notification hooks, and
  the inbox folder the game reads those messages from are now the cargo
  feature `agent-mail`, on by default so a plain `cargo build` is
  upstream's game. Our builds use `--no-default-features`: neither
  command exists (`tfac msg` is an unknown option), no inbox is read, and
  `--help` leaves them out. Company letters, the welcome telegram and the
  telegram log (`n`, `i`) are untouched; the empty log says it keeps
  company letters. (`Cargo.toml`, `src/mail.rs`, `src/lib.rs`,
  `src/sim.rs`, `src/ui/maillog.rs`)
- **Session logging** with `log` and `env_logger`:
  `RUST_LOG=debug tfac 2>tfac.log` logs the arguments, the folder and
  save, every input event as the terminal delivers it, and each save,
  autosave and quit with a line about the factory (tick, money,
  research, floors). Without `RUST_LOG`, only errors. (`src/lib.rs`)

The game itself, its text and its built-in web are as upstream wrote
them. Terminactory makes no network connections: termweb is made of the
Markdown pages in `wiki/`, `sites/`, `forums/` and `intranets/`, built
into the binary. The only program it starts is the platform's clipboard
tool (`pbcopy`, `wl-copy`, `xclip`) for termweb's Copy page.

## Packaging (new files)

- `.github/workflows/asciiverse-release.yml`: av archives for Linux
  (static musl, x86-64 and ARM64), macOS (Intel and Apple silicon) and
  Windows (x86-64 and ARM64), plus SHA256SUMS, built without the
  `agent-mail` feature. Each archive holds `terminactory` and its
  shortcut `tfac`, `LICENSE`, `CHANGELOG.md`, this file and our
  `README.md`.
- `asciiverse/README.md`: the package README, shipped in the archives as
  `README.md` (upstream's README is about agent messages and installing
  with cargo).
- `asciiverse/record-preview.sh`: the catalog preview, the demo factory
  and termweb driven by keys sent to a tmux pane.
- `asciiverse/terminactory.toml`, `asciiverse/terminactory.md`,
  `asciiverse/terminactory.cast`: the catalog manifest, store page and
  preview, drafts for hub.
- `PATCHES.md` (this file).
