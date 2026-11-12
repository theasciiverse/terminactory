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
- Large kiln (5×7, batch of 32, a 20 s firing, $7,500, Underhill; research 10,000 RP or make 1,000 brick): four kilns in one machine, burning four kilns' coal. It is the second version in the kiln's build slot (press the slot's number again).

- Area tool (`c`): mark a rectangle with two corners or a drag, then `x` removes everything in it with a full refund (waste held in storage pays its fee once, after a second `x` when it is over $20). `z` puts back the last area you removed. Free from the start.
- Blueprints (Brannock & Daughters; research 800 RP or make 1,500 copper plate; a new Tools group): `c` (or Space, Enter) copies a chosen rectangle to your hand, a ghost follows the cursor, `r` / Shift+`r` turn it, Space or a click pastes what fits and what you can afford in reading order, and pasting again at the same spot fills in the rest at no double charge. A middle-drag copies a rectangle straight to the hand. Settings, recipes (by name), tunnel pairs, elevators and long machines travel with it.
- Blueprint library (`l`): saved blueprints in `~/.terminactory/blueprints`, one JSON file each with its own `format` number, shared by every save. `Enter` takes one, `f` saves your hand under a name, `x` twice deletes. Files that can't be read, or are from a newer game, are listed and left alone. Saves are unchanged (format version 6).
- Upgrade planner (Swiftline Conveyors; research 3,000 RP or make 60 gearboxes; in the Tools group): with an area chosen, `u` previews ("u again to upgrade 12 parts for $360") and a second `u` swaps each part for its next version in place, paying only the price difference: belt to fast belt, cheap drill to basic drill, cheap solar to basic solar, advanced to precision assembler, large basic to large advanced assembler and drill. Facing, settings, recipes (by name) and contents stay where the new part can hold them. It goes in reading order and stops when the money runs out, and the message counts what is short or locked. Upgradable parts are tinted green while the area is chosen, and `z` swaps the last upgrade back for a refund. Before the tech is researched `u` says what unlocks it. Saves are unchanged (format version 6).

### Changed
- Saves name items, companies, techs and recipes instead of counting positions, so new content never breaks a save. A machine whose recipe has gone goes back to its first recipe, with a note. Pipes keep their fluid across a save, and a save records which game version wrote it.
- Balance pass on prices, research costs and free unlocks, and several recipe routes (see the plan 01 commits).
- Pipes moved from Create 5 to Transport 5 (the key to press for pipes is now `2` `5`), Misc has room for a third slot, and the research screen has a Pipes group. The build menu layout is temporary until a playtest.
- Settings moved from `c` to `g`; `c` is now the area tool, and `l` is the blueprint library, and `u` is the upgrade planner.
- The repo-root sample `controls.json` is gone; the game still writes its own to `~/.terminactory` on first run.

### Fixed
- Windows: saves and settings now go to `USERPROFILE` / `APPDATA` when `HOME` is unset.
- The single-window lock no longer shells out to `kill`; it uses `kill(pid, 0)` on unix and an open lock file on Windows.
- Clippy warnings.
