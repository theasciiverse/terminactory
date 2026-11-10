# Terminactory

A cozy factory game in the terminal. It grows from basic items: copper, coal and iron, and the many parts, recipes and toys they lead to. Design notes live in `../NOTES.md`.

## Run

Install both commands, `terminactory` and its shortcut `tfac`:

```bash
cargo install --path .
```

Or play straight from the source folder:

```bash
cargo run --release
```

That continues the save you played last. Saves are numbered (`save1`, `save2`, ...):

```bash
tfac --saves   # list them: money, research, time played, when last played
tfac save2     # open one (or just: tfac 2)
tfac --new     # start a fresh one as the next number; the others are kept
tfac --help    # everything tfac can do
```

`--demo` starts next to a small working factory with its own save. Saves and key settings (`controls.json`) live in `~/.terminactory` (saves in `saves/`), so the game finds them from any folder; it saves every 2 minutes, when you quit, and on Ctrl+S. A save can only be open in one window at a time: a second `tfac` on the same save says which window has it. Saves carry a format version: one made by an older version of the game isn't opened (and never deleted). `tfac` tells you, `tfac --saves` marks it "(old version)", and `tfac --new` starts a fresh one.

Best in a terminal at least 120×40. In anything smaller than 100×30 (say, a split next to your agent) it switches to a **small-pane view** by itself: the map fills the pane, one line on top shows money, research, power, idle machines and waiting mail, and the bottom line shows the build menu, what's under the cursor, or messages. `m` switches views by hand. Mouse works in most terminals (iTerm2, Terminal.app, WezTerm, Kitty).

## What's in it

Phase 1: sell for money and research, research the copper press and coking, and make the four key items (copper wire, coke, copper pipe, copper coil), each bringing a letter from a company.

Phase 2: what those key items unlock.
- **Pumps & pipes** (after copper pipe): pumps draw water anywhere on the ground; pipes join into networks that each hold one fluid.
- **Steam power** (after copper coil): boilers burn coal to turn water into steam, out of their front; steam engines turn steam into 40 power each, only as much as their grid needs beyond solar.
- **Blast furnace** (after coke): burns coke, smelts iron ore into iron plate, and makes steel from iron plate. Steel leaves 1 **waste** per steel, which comes out the front too and has to go somewhere (see Waste below).

Phase 3: floors, elevators and machinery.
- **Floors**: support beams (after steel girders) open floor +1; excavation (after steel girders) opens floor -1. Decking or digging costs $10 a tile (more on later floors), never any waiting. Drills underground mine 1.5x faster (2x and 3x on deeper floors later).
- **Elevators**: boxes from 2x2 to 8x8 (`+`/`-` to resize while placing), holding 10 items per tile. Each open floor gets a door; `f` sets a door to load, unload or pass. A car leaves only when full: 0.5 s doors, 1 s per floor, 0.5 s doors, then unloads at 30 items/s out of the door's arrow side.
- **Machinery**: the assembler (after a gearbox) makes any press recipe twice as fast, plus the drilling rig; fast belts (after iron gears) move twice as much.
- **The drilling rig** (4 girders + 2 gearboxes + 4 steel for now) is the most valuable thing you can make so far.
- **Logistics toys** (research them in the Moving group):
  - **Belt tunnels** pass under up to 4 tiles. Place an entrance, then its exit up to 5 tiles further on facing the same way; build anything in between. They need no power.
  - **Long arm**: grabs from 2 tiles behind and drops 2 ahead, over whatever sits in between.
  - **Filter splitter**: `f` picks an item; that item goes out the front and everything else out the sides (with no item picked it shares like a splitter).
  - **Warehouse**: a 2×2 crate holding four crates' worth.
  - Pipes moved next to pumps in the build menu (Create 4, press again for pipes) to make room: Transport is now belts / fast belts / tunnels, splitters / filter splitters, arms / long arms, elevators. Misc 1 is crate / warehouse.
- **Free unlocks**: everything also unlocks free once you've made enough of its milestone item (shown in the tree); research is the shortcut.
- **Idle machines** flash a word saying why: FULL, NEED, COAL, COKE, PWR, WATR, ORE?, STM?.
- **Offline earnings**: when you come back, you get what your factory earned while the game was closed, with no cap.
- **Faster early game**: drills and copper furnaces run twice as fast, and the first research is cheaper.

## Recipe book

`b` opens it on whatever is under the cursor. It has a page for every item (Items tab) and for each machine that makes or mines them (drills, furnaces, press, assembler).

- **Item pages:** what makes it, as a word equation with the fuel over the arrow (`copper ore ─coal─→ copper plate`), plus timing and anything still locked. Also what uses it, what it unlocks, and a chart of research per sale that marks its floor and where you are.
- **Machine pages:** cost, whether you can build it (and what opens it if not), and everything it makes.
- **Tabs:** Tab switches between the Items tab and the Machines tab.
- **Links:** blue names are links: click one to go there, and Backspace goes back.
- **Search:** `/` filters the list as you type; Enter stops typing, Esc clears it.

## Agent telegrams

Your AI agents can message you inside the game. Each message shows up as a telegram (Markdown rendered: headings, lists, checkboxes, quotes, code, tables, **bold**, `code`). Every few minutes a company sends a small delivery with one: about a minute of your factory's earnings.

Agents write plain Markdown, never game text, so a message costs only the words in it. Both commands print nothing on success.

```bash
tfac msg "## Tests fixed\n- 3 flaky tests\n- [ ] needs review"
```

`\n` is a line break; with no text, `tfac msg` reads Markdown from stdin. The sender defaults to the current folder's name (`--from NAME` changes it). Anything can also drop a `*.md` file into `~/.terminactory/inbox/` (optional first line `from: NAME`).

**Claude Code, automatically, at no token cost:** add this to `~/.claude/settings.json`. When an agent finishes, its own last reply becomes the telegram; when it needs permission or input, you get a "needs you" telegram.

```json
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }]
  }
}
```

Telegrams sit at the bottom of the sidebar while the game keeps running: `n` dismisses one and shows the next, Shift+N dismisses all, PgUp/PgDn or the mouse wheel scroll a long one. In the small-pane view a note in the bottom-right corner ("agent finished · n to view") says one is waiting, and `n` opens it. Messages wait while the game is closed (only the newest 8 are kept).

## Rules in one breath

- Buildings take input from **any side** and send output out of their **front** only (the arrow on their edge): onto a belt, or straight into the next building, like a drill directly into a furnace.
- Sizes follow roles: belts, splitters, arms and crates are 1×1; drills, furnaces, solar panels and elevators are 2×2; the trade post is 3×3.
- Drills mine anywhere on the ground (not on decking): press f on one to pick copper or coal (and iron, for basic drills). The letter on a drill shows its ore (C copper, * coal, I iron). Drills burn coal; a drill on coal fuels itself. Middle click copies a drill's ore to the next one you place.
- Furnaces and presses make the recipe they're set to (`f` changes it). Furnaces burn coal for plates; with coking researched they bake coal into coke. Presses turn plates into wire and pipe, and wire into coils. Only the basic drill cuts iron.
- Research (`t`) brings new parts, grouped like the build menu (Making, Moving, Power, Floors); some open only once you've made a key item (copper wire, coke, copper pipe, copper coil, girders, gears, a gearbox), and everything also unlocks free once you've made enough of its milestone item. Enter researches the picked row; clicking a row picks it and clicking it again researches it.
- Belts, splitters and arms need power. Solar panels (reach 2 tiles) and power poles (reach 3 tiles) power every electric part within range; ranges that overlap join into one grid, and each grid shares only its own power. Solar panels never join each other directly, even side by side: link them with poles. The Power figure at the top is **regional** (the grid of the pole, panel or machine under the cursor, and hovering a pole or panel shades just that grid) or **global** (all grids together, with any supply that powers nothing shown as unused). Too little power slows a grid's parts evenly. Elevators carry power to the floor below. Unpowered belts turn red; `v` shows the powered area.
- Trade posts sell what they receive: money, a 40/40 split, or research (press `f` on one).
- **Waste** sells for -$0.50 each and never earns research. Only a trade post set to money takes it (and pays the fee); research and split posts refuse it, so it stays on the belt and backs up (the post flashes `WSTE`). A post also refuses waste when the fee would take your money below zero, or when the last minute's waste fees are more than the last minute's earnings. Removing a crate, warehouse or elevator that holds waste costs the same $0.50 each (refused if you can't afford it); hover it to see the cost. The production panel (`o`) shows waste fees as negative money.
- Research per item fades the more you sell of it, down to a small per-item minimum, so new items are where research comes from. Research earned while the game is closed fades the same way.
- Nothing breaks or jams at random. Removing a part refunds it in full.

## Controls

| Action | Keys |
| --- | --- |
| Move | arrows / WASD |
| Lay belts | Shift+arrows / Shift+WASD, or drag with the mouse |
| Build menu | 1–4 opens a category (1 Create, 2 Transport, 3 Power, 4 Misc); inside it, 1–4 picks a part, and pressing the same number again switches kind (copper drill / basic drill, belt / fast belt / splitter, furnace / blast furnace, press / assembler); `` ` ``, `₹`, `~` or Esc goes back |
| Place / remove | Space or click / x or right-click |
| Copy a part | middle-click it: picks up its kind, facing and setting (recipe, arm filter, trade post mode), so clicks place copies; middle-click empty ground to empty your hand |
| Rotate (belt direction / output side) | r |
| Set up (arm filter, trade post mode) | f |
| Change floor | < > or scroll |
| Research | t (arrows pick, Enter researches; click a row, click again to research) |
| Save | Ctrl+S (it also saves every 2 minutes and on quit) |
| Production panel | o: the last minute per item (made, used, sold, money, research, what the next sale pays); the game keeps running |
| Show power range | v |
| Elevator size | + / - |
| Pause / speed | p / [ ] |
| Recipe book (opens on what's under the cursor) | b |
| Small-pane view on/off | m |
| Next telegram / dismiss all | n / Shift+N (PgUp/PgDn scroll) |
| Help | ? (arrows scroll it in a small pane) |
| Quit | Ctrl+C (saves first) |

## Changing keys

`~/.terminactory/controls.json` (created on first run) lists the keys for every action, like `"back": ["`", "₹", "~", "Esc"]`. A key is one character or Esc, Enter, Space, Tab, Backspace, Delete, Up, Down, Left, Right. Number keys, Ctrl+S and Ctrl+C are fixed. Restart the game after editing.

## What's on screen

Buildings are coloured blocks labelled with their name, and an arrow on their edge marks the output side. A grey block is idle or unpowered.
Belts are dim arrows (`>>`, muted red when unpowered), and items ride on them. There are no ore patches or lakes: drills and pumps work anywhere on the ground. Point at anything and the side panel says what it is (and what a belt is carrying).

| Block | Part | Size | Item glyph |
| --- | --- | --- | --- |
| `DRIL` | Cheap drill | 2×2 | `o` copper ore |
| `>>` | Basic belt | 1×1 | `*` coal |
| `SP` | Basic splitter | 1×1 | `=` copper plate |
| `A>` | Filtering arm | 1×1 | |
| `FURN` | Copper furnace (letter = recipe: P plates, K coke) | 2×2 | `~` wire, `0` pipe |
| `PRES` | Copper press (W wire, P pipe, C coil) | 2×2 | `@` coil, `#` coke |
| `DRL2` | Basic drill (cuts iron) | 2×2 | `o` (blue) iron ore |
| `C>` | Crate | 1×1 | |
| `TRADE` | Trade post (`$ $ $` money, `$ / R` split, `R R R` research) | 3×3 | |
| `SOLR` | Cheap solar panel | 2×2 | |
| `/\` | Power pole | 1×1 | |
| `┌ LIFT ┐` | Elevator: a box, 2×2 to 8×8 | 2×2–8×8 | |
| `==` `\|\|` `++` | Pipe (blue = water, white = steam, grey = empty) | 1×1 | `=` (pale) iron plate |
| `PU` | Water pump (anywhere on the ground) | 1×1 | `%` steel |
| `BLST` | Blast furnace (I iron plate, S steel) | 2×2 | `,` waste |
| `BOIL` | Boiler | 2×2 | |
| `ENGN` | Steam engine | 2×2 | |
| `ASSMBL` | Assembler | 3×3 | `H` girder |
| `>>` (yellow) | Fast belt | 1×1 | `g` gear, `G` gearbox, `R` rig |

## Code

- `src/sim.rs`: the simulation. 60 ticks per second, no randomness, no drawing.
- `src/ui.rs`: terminal drawing (ASCII on a fixed dark map).
- `src/app.rs`: keyboard and mouse.
- `src/content.rs`: items, companies, research and recipes (data, kept apart from mechanics).
- `src/mail.rs`: agent telegrams: the inbox, `tfac msg` / `tfac hook`, and the Markdown renderer.
- `cargo test`: simulation tests plus screen snapshots (`target/snapshot*.html` show the map, tree and a letter in colour).
