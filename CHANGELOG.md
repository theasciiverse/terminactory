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

- Tubes (phase 1 of the computers branch): five new items (tube $48, transformer $200, lamp matrix $230, tube board $900, terminal $2,100) and a new company, Sablon (key item: tube; research 30,000 RP for the evacuator, or make 40 tubes). The tube is baked in the kiln first ("tube (baked)": lamp + copper sheet + 2 fine wire, a 40 s firing for 8, leaving waste out the back; the large kiln bakes 32), so it can be made before Sablon opens. The evacuator (2x2, 6 EC, $3,500, water from a pipe) makes 2 tubes a run with no waste; it sits in the water slot of the Create menu. The crude assembler makes the transformer and lamp matrix, the advanced assembler the tube board (6 tube + control board + iron sheet), the basic assembler the terminal (a keyboard joins it in phase 5). Transformer and terminal have no use yet. Saves are unchanged (format version 6).
- Silicon (phase 2a of the computers branch), on the old machines with no new raw material: a new fluid, molten silicon (blast furnace: 3 sand + 1 coke gives it and 2 waste; a molder casts the silicon ingot), nine items (silicon ingot $6, silicon scrap $1.50, wafer $12, dopant $10, doped wafer $50, ceramic $8, circuit board $60, transistor $42, transistor board $450, the x3x1), recipes in the lathe, kiln and large kiln, and the advanced and precision assemblers, and a new company, Halverson Smelting (key item: silicon ingot), which sells nothing until the next phase. Saves are unchanged (format version 6).
- Chips (phase 3 of the computers branch), with no new raw material: five new items (etchant $2, mask $40, die $68, microprocessor (x4x1) $200, RAM chip $150), a new machine, the etcher (3×3, 24 EC, $25,000; Lattice & Finch; research 80,000 RP or make 60 transistor boards; sixth slot of Create), and a new company, Meridian Instruments (key item: the x4x1; sells nothing yet). The mixer makes etchant from coal, sand and water (with waste out the back), the advanced assemblers build masks, the etcher turns a doped wafer, a mask and etchant into 2 dies and 2 silicon scrap, and only the precision assembler builds the microprocessor and RAM chip. Six slots in Create fill a narrow build line, so very small panes show the short codes sooner. Saves stay format 6.
- Better silicon (phase 2b of the computers branch), with no new raw material: Halverson Smelting now sells three EC machines, each a better road beside the old blast furnace and lathe routes. The arc furnace (3×3, 40 EC, $10,000; research 40,000 RP or make 100 silicon ingot) melts silicon with no fuel (3 sand + 1 coke gives 2 molten silicon and 1 waste; a coal route; and a waste-free remelt of 3 silicon scrap into 2 molten silicon) and ends the heat slot of Create. The crystal grower (2×4, 12 EC, $7,500; 45,000 RP or 200 silicon ingot) grows a new item, the silicon crystal ($50), from 3 silicon ingots in 20 s, the slowest machine in the game. The wafer saw (2×1, 6 EC, $6,000; 35,000 RP or 100 wafer) saws a crystal and a fine wire into 6 wafers with no scrap. The grower and the saw follow the etcher in the sixth slot of Create. Saves stay format 6.
- Fine etching (phase 4 of the computers branch), with no new raw material: three new items (fine mask $90, fine die $140, microprocessor (x4x2) $420) and Meridian's first machine, the fine etcher (3×5, 96 EC, $60,000; research 150,000 RP or make 100 microprocessor (x4x1); last in slot 6 of Create). It etches a doped wafer, a fine mask and etchant into a fine die and a spare die in 20 s, with no scrap. The precision assembler builds the fine mask (mask + glass + 2 fine wire) and the x4x2 (fine die + ceramic + 4 fine wire). The etcher's die and the x4x1 stay as the old road. Saves are unchanged (format version 6).
- Area tool (`c`): mark a rectangle with two corners or a drag, then `x` removes everything in it with a full refund (waste held in any part, belts and machines included, pays its fee once, after a second `x` when it is over $20). `z` puts back the last area you removed. Free from the start.
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
