# Changelog

Each release notes whether saves from the previous one still open. Saves from another
version are refused, never deleted.

## Unreleased

Saves: format version 6 (older saves are refused; start a fresh game with `tfac --new`).

### Added
- Earnings history: a History tab in the production panel, and a compact earnings graph under the build menu.
- Settings pane (`g`): the sidebar graph and its range, the small-pane view and width, the telegram style (Sidebar, Note, Ticker) and Hints.
- Telegram log (`i`) with unread marks, a scrollbar for long telegrams, and company letters that arrive as telegrams and never pause the game.
- A welcome telegram on a new game, and quiet hints on the idle status line (Hints setting).
- Small-pane view for narrow splits next to an agent, with the stuck-machine reason first.
- Boilers burn waste (4 waste give the fire of 1 coal); waste leaves a machine by its back edge.
- `tfac --version` / `-V`.
- Apache-2.0 license and package metadata.
- Basic solar panel (2×2, 20 EC, reach 4, $300, Kettering & Vane; research 1,800 RP or make 60 glass): the second version in the solar slot. Panels still link only through poles.
- Large kiln (5×7, batch of 32, an 80 s firing, $7,500, Underhill; research 10,000 RP or make 1,000 brick): the kiln's recipes in one bigger lump. It is the second version in the kiln's build slot (press the slot's number again).

### Changed
- Saves name items, companies, techs and recipes instead of counting positions, so new content never breaks a save. A machine whose recipe has gone goes back to its first recipe, with a note. Pipes keep their fluid across a save, and a save records which game version wrote it.
- Balance pass on prices, research costs and free unlocks, and several recipe routes (see the plan 01 commits).
- Pipes moved from Create 5 to Transport 5 (the key to press for pipes is now `2` `5`), Misc has room for a third slot, and the research screen has a Pipes group. The build menu layout is temporary until a playtest.
- Settings moved from `c` to `g`; `c`, `l` and `u` stay free for the building tools.
- The repo-root sample `controls.json` is gone; the game still writes its own to `~/.terminactory` on first run.

### Fixed
- Windows: saves and settings now go to `USERPROFILE` / `APPDATA` when `HOME` is unset.
- The single-window lock no longer shells out to `kill`; it uses `kill(pid, 0)` on unix and an open lock file on Windows.
- Clippy warnings.
