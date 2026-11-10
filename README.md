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

Best in a terminal at least 120×40. In anything smaller than 100×30 (say, a split next to your agent) it switches to a **small-pane view** by itself: the map fills the pane, one line on top shows money, research, EC, idle machines and waiting mail, and the bottom line shows the build menu, what's under the cursor, or messages. `m` switches views by hand. Mouse works in most terminals (iTerm2, Terminal.app, WezTerm, Kitty).

## What's in it

Phase 1: ore is melted in a furnace into molten metal, a molder casts it into ingots, and a press makes plates. Sell for money and research, research the press and coking, and make the four key items (copper wire, coke, copper pipe, copper coil), each bringing a letter from a company.

Phase 2: what those key items unlock.
- **Pumps & pipes** (after copper pipe): pumps draw water anywhere on the ground; pipes join into networks that each hold one fluid.
- **Steam engines** (after copper coil): boilers burn coal to turn water into steam, out of their front; steam engines turn steam into 40 EC each, only as much as their grid needs beyond solar.
- **Blast furnace** (after coke): burns coke, melts iron ore into molten iron fast (1 ore a go, 5 s), or iron grit even better, and makes steel from iron plate. Steel leaves 1 **waste** per steel, which comes out the front too and has to go somewhere (see Waste below).

Phase 3: floors, elevators and machinery.
- **Floors**: support beams (after steel girders) open floor +1; excavation (after steel girders) opens floor -1. Decking or digging costs $10 a tile (more on later floors), never any waiting. Drills underground mine 1.5x faster (2x and 3x on deeper floors later).
- **Elevators**: boxes from 2x2 to 8x8 (`+`/`-` to resize while placing), holding 10 items per tile. Each open floor gets a door; `f` sets a door to load, unload or pass. A car leaves only when full: 0.5 s doors, 1 s per floor, 0.5 s doors, then unloads at 30 items/s out of the door's arrow side.
- **Machinery**: the crude assembler (4×4, after an iron gear) makes any press recipe twice as fast, plus the drilling rig; fast belts (after a gearbox) move twice as much.
- **The drilling rig** (4 girders + 2 gearboxes + 4 steel for now) is the most valuable thing you can make so far.
- **Logistics toys** (research them in the Moving group):
  - **Belt tunnels** pass under up to 4 tiles. Place an entrance, then its exit up to 5 tiles further on facing the same way; build anything in between. They need no power.
  - **Long arm**: grabs from 2 tiles behind and drops 2 ahead, over whatever sits in between.
  - **Filter splitter**: `f` picks an item; that item goes out the front and everything else out the sides (with no item picked it shares like a splitter).
  - **Warehouse**: a 2×2 crate holding four crates' worth.
  - Pipes moved next to pumps in the build menu (Create 4, press again for pipes) to make room: Transport is now belts / fast belts / tunnels, splitters / filter splitters, arms / long arms, elevators. Misc 1 is crate / warehouse.
- **Free unlocks**: everything also unlocks free once you've made enough of its milestone item (shown in the tree); research is the shortcut.
- **Idle machines** flash a word saying why: FULL, NEED, COAL, COKE, EC?, FLUD, WATR, ORE?, STM?. (EC? is no electric charge; FLUD is a molder waiting for molten metal.)
- **Offline earnings**: when you come back, you get what your factory earned while the game was closed, with no cap.
- **Faster early game**: drills run twice as fast, and the first research is cheaper.

## The iron and coal step

Six new machines (all in Create), each from a company that opens when you first make its key item, then researched or earned free like the rest:

| Machine | Size · EC · cost | Company (opens on) | Research | Free once you've made |
| --- | --- | --- | --- | --- |
| **Crusher** | 2×2 · 6 · $250 | Ashgrove (copper wire) | 150 | 300 copper ore |
| **Winder** | 2×1 · 2 · $200 | Kettering (copper coil) | 200 | 40 copper coil |
| **Wire drawer** | 2×1 · 4 · $250 | Kettering (copper coil) | 250 | 400 copper wire |
| **Coke oven** | 2×2 · none · $150 | Cinderhall (coke) | 300 | 60 coke |
| **Lathe** | 3×3 · 4 · $200 | Gearwright (iron gear) | 600 | 300 iron plate |
| **Rolling mill** | 2×2 · 8 · $400 | Pillar (steel girder) | 1,200 | 500 iron plate |

- **Crusher**: 1 copper ore → 2 copper grit (4 s); 1 iron ore → 2 iron grit (4 s); 1 stone → 2 sand (2.5 s). Grit melts further than ore: 3 copper grit → 2 molten copper in a furnace, so crushed copper gives a third more ingots per ore. 2 iron grit → 1 molten iron in a furnace (5 s); 3 iron grit → 2 molten iron in a blast furnace (2.5 s).
- **Coke oven**: 2 coal → 1 coke + 1 waste (2 s). It cooks its own coal: no EC, no separate fuel. The furnace also gets a fast and dirty coke recipe: 7 coal → 2 coke + 3 waste (2.5 s); the slow 3 coal → 1 coke (5 s) stays.
- **Press** (retimed): copper wire 1 plate → 2 wire (5 s); copper pipe 2 plates → 1 pipe (2.5 s); copper coil 4 wire → 1 coil (5 s), or 5 fine wire → 1 coil (2.5 s); iron gear 2 plates → 1 gear (2.5 s). The assembler copies them at half the time.
- **Wire drawer**: 1 copper plate → 3 copper wire (4 s); 1 copper wire → 2 fine wire (4 s).
- **Winder**: 3 copper wire → 1 copper coil (4 s); 2 iron rod → 1 spring (5 s).
- **Rolling mill**: 2 copper plate → 1 copper sheet (5 s); 2 iron plate → 1 iron sheet (2.5 s).
- **Lathe**: 1 iron plate → 2 iron rod (5 s); 1 iron rod → 4 screw (5 s).
- New items and prices: copper grit $0.80, iron grit $1.20, sand $0.25, fine wire $2.50, copper sheet $12, iron rod $5, screw $4, spring $15, iron sheet $18. Existing prices now: copper wire $3, copper pipe $12, copper coil $18, iron gear $18, coke $2.50. Glyphs: `:` grit (copper orange, iron blue) and sand (tan), `~` fine wire (pale), `-` sheet, `!` rod, `x` screw, `z` spring.
- **Companies now** (key item → what they offer): copper wire → Ashgrove (basic drill, crusher); coke → Cinderhall (blast furnace, coke oven); copper pipe → Wellbourne (pump, pipes); copper coil → Kettering (boiler, steam engine, wire drawer, winder); iron gear → Gearwright (assembler, lathe); steel girder → Pillar (floor +1, rolling mill). Swiftline's fast belts now open on a gearbox instead of an iron gear.
- The Create menu groups them: 1 drills and crusher, 2 furnace / molder / blast furnace / coke oven, 3 press / wire drawer / winder / rolling mill / lathe / assembler, 4 pump / pipe.

## Molten metal

- **Furnace** (burns coal): copper ore → molten copper (1 → 1, 5 s); iron ore → molten iron (3 → 2, 10 s, slow); grit melts better (see above); with coking, 3 coal → coke (5 s) or 7 coal → 2 coke + 3 waste (2.5 s). Press `f` on it to change the recipe.
- **Molder** (2 EC, $100, Create 2, next to the furnace): casts molten metal into ingots: copper ingot ($2), iron ingot ($1.50), steel (2.5 s each). `f` picks the recipe; it only takes the metal its recipe uses.
- **Press**: copper plate and iron plate come from ingots now (2.5 s), then wire, pipe, coil and the rest as before. Copper ingots are the free milestone for the press (make 50).
- **Fluids in pipes**: a machine that makes a fluid pushes it out of its **front**, into a pipe touching that edge, or straight into a machine that takes it (a furnace right behind a molder needs no pipe). A machine that needs a fluid takes it from pipes on any of its other sides. A pipe network holds one fluid at a time: water blue, steam white, molten copper orange, molten iron red, molten steel pale yellow, grey when empty. Pointing at a pipe or a machine names the fluid.
- A machine can have two outputs (a fluid out the front and items out the front). It waits, saying FULL, until both have room.
- **Stone** ($0.30): drills can mine it (`f` cycles copper, coal, iron, stone). A cheap drill cuts iron at half the speed of a basic drill.

## Recipe book

`b` opens it on whatever is under the cursor. It has a page for every item (Items tab) and for each machine that makes or mines them (drills, crusher, furnace, molder, blast furnace, coke oven, press, wire drawer, winder, rolling mill, lathe, crude assembler).

- **Item pages:** what makes it, as a word equation with the fuel over the arrow (`copper ore ─coal─→ molten copper`), plus timing and anything still locked. Also what uses it, what it unlocks, and a chart of research per sale that marks its floor and where you are.
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
- Sizes follow roles: belts, splitters, arms and crates are 1×1; drills, solar panels and elevators are 2×2; furnaces, lathes and the trade post are 3×3. Presses, wire drawers and winders (2×1) and pumps (1×2) are long machines: the size given is facing east or west, and they swap width and height when turned north or south. The rotate key (`r`) shows the turned shape while placing, and turning a placed one needs the room (it keeps its top-left corner).
- Drills mine anywhere on the ground (not on decking): press f on one to pick copper, coal, iron or stone. The letter on a drill shows its ore (C copper, * coal, I iron). Drills burn coal; a drill on coal fuels itself. Middle click copies a drill's ore to the next one you place.
- Furnaces, molders and presses make the recipe they're set to (`f` changes it). Furnaces burn coal to melt ore; with coking researched they bake coal into coke. Molders cast ingots, presses turn ingots into plates, plates into wire and pipe, and wire into coils. A basic drill cuts iron twice as fast as a cheap one.
- Research (`t`) brings new parts, grouped like the build menu (Making, Moving, EC, Floors); some open only once you've made a key item (copper wire, coke, copper pipe, copper coil, iron gear, gearbox, girders), and everything also unlocks free once you've made enough of its milestone item. Enter researches the picked row; clicking a row picks it and clicking it again researches it.
- Power is called **EC** (electric charge). Belts, splitters, arms, molders, presses and the other shaping machines need it (coke ovens don't). Solar panels (reach 2 tiles) and EC poles (reach 3 tiles) charge every electric part within range; ranges that overlap join into one grid, and each grid shares only its own EC. Solar panels never join each other directly, even side by side: link them with poles. The EC figure at the top is **regional** (the grid of the pole, panel or machine under the cursor, and hovering a pole or panel shades just that grid) or **global** (all grids together, with any supply that powers nothing shown as unused). Too little EC slows a grid's parts evenly. Elevators carry EC to the floor below. Belts without EC turn red; `v` shows the area it reaches.- Trade posts sell what they receive: money, a 40/40 split, or research (press `f` on one).
- **Waste** sells for -$0.50 each and never earns research. Only a trade post set to money takes it (and pays the fee); research and split posts refuse it, so it stays on the belt and backs up (the post flashes `WSTE`). A post also refuses waste when the fee would take your money below zero, or when the last minute's waste fees are more than the last minute's earnings. Removing a crate, warehouse or elevator that holds waste costs the same $0.50 each (refused if you can't afford it); hover it to see the cost. The production panel (`o`) shows waste fees as negative money.
- Research per item fades the more you sell of it, down to a small per-item minimum, so new items are where research comes from. Research earned while the game is closed fades the same way.
- Nothing breaks or jams at random. Removing a part refunds it in full.

## Controls

| Action | Keys |
| --- | --- |
| Move | arrows / WASD |
| Lay belts | Shift+arrows / Shift+WASD, or drag with the mouse |
| Build menu | 1–4 opens a category (1 Create, 2 Transport, 3 EC, 4 Misc); inside it, 1–4 picks a part, and pressing the same number again switches kind (copper drill / basic drill, belt / fast belt / splitter, furnace / molder / blast furnace / coke oven, press / wire drawer / winder / rolling mill / lathe / crude assembler); `` ` ``, `₹`, `~` or Esc goes back |
| Place / remove | Space or click / x or right-click |
| Copy a part | middle-click it: picks up its kind, facing and setting (recipe, arm filter, trade post mode), so clicks place copies; middle-click empty ground to empty your hand |
| Rotate (belt direction / output side) | r |
| Set up (arm filter, trade post mode) | f |
| Change floor | < > or scroll |
| Research | t (arrows pick, Enter researches; click a row, click again to research) |
| Save | Ctrl+S (it also saves every 2 minutes and on quit) |
| Production panel | o: the last minute per item (made, used, sold, money, research, what the next sale pays); the game keeps running |
| Show EC range | v |
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

Buildings are coloured blocks labelled with their name, and an arrow on their edge marks the output side. A grey block is idle or has no EC.
Belts are dim arrows (`>>`, muted red without EC), and items ride on them. There are no ore patches or lakes: drills and pumps work anywhere on the ground. Point at anything and the side panel says what it is (and what a belt is carrying).

| Block | Part | Size | Item glyph |
| --- | --- | --- | --- |
| `DRIL` | Cheap drill | 2×2 | `o` copper ore |
| `>>` | Basic belt | 1×1 | `*` coal |
| `SP` | Basic splitter | 1×1 | `=` copper plate |
| `A>` | Filtering arm | 1×1 | |
| `FURN` | Furnace (letter = recipe: M molten copper, I molten iron, K coke) | 3×3 | `~` wire, `0` pipe |
| `MOLD` | Molder (C copper ingot, I iron ingot, S steel) | 2×2 | `i` ingot (copper orange, iron blue) |
| `PRW>` | Press (= copper plate, - iron plate, W wire, P pipe, C coil; the letter sits before the arrow) | 2×1 | `@` coil, `#` coke |
| `DRL2` | Basic drill (iron twice as fast) | 2×2 | `o` (blue) iron ore, `o` (grey) stone |
| `C>` | Crate | 1×1 | |
| `TRADE` | Trade post (`$ $ $` money, `$ / R` split, `R R R` research) | 3×3 | |
| `SOLR` | Cheap solar panel | 2×2 | |
| `/\` | EC pole | 1×1 | |
| `┌ LIFT ┐` | Elevator: a box, 2×2 to 8×8 | 2×2–8×8 | |
| `==` `\|\|` `++` | Pipe (blue water, white steam, orange copper, red iron, pale steel, grey empty) | 1×1 | `=` (pale) iron plate |
| `PU` `MP` | Water pump (anywhere on the ground) | 1×2 | `%` steel |
| `BLST` | Blast furnace (I molten iron, S steel) | 2×2 | `,` waste |
| `BOIL` | Boiler | 2×2 | |
| `ENGN` | Steam engine | 2×2 | |
| `CRD ASM` | Crude assembler | 4×4 | `H` girder |
| `>>` (yellow) | Fast belt | 1×1 | `g` gear, `G` gearbox, `R` rig |
| `CRSH` | Crusher (C copper grit, I iron grit, S sand) | 2×2 | `:` grit and sand |
| `COKE` | Coke oven | 2×2 | `-` sheet, `!` rod |
| `WDW>` | Wire drawer (W wire, F fine wire) | 2×1 | `x` screw, `z` spring |
| `WNC>` | Winder (C coil, S spring) | 2×1 | |
| `ROLL` | Rolling mill (C copper sheet, I iron sheet) | 2×2 | |
| `LATHE` | Lathe (R rod, S screw) | 3×3 | |

## Code

- `src/sim.rs`: the simulation. 60 ticks per second, no randomness, no drawing.
- `src/ui.rs`: terminal drawing (ASCII on a fixed dark map).
- `src/app.rs`: keyboard and mouse.
- `src/content.rs`: items, companies, research and recipes (data, kept apart from mechanics).
- Later machines are in the simulation but not yet in the build menu (`Kind::in_game`): the assembler ladder (basic, advanced and precision 2×2 / 3×3 / 3×3, large basic and advanced 5×5; each assembler also makes the tiers below it faster, except the precision one, which makes only the top tiers), the welder (draws 30 EC only while it welds), the kiln (3×5, fires 8 runs at once), the mixer (water from pipes), the advanced drill (4×4, EC), large basic and advanced drills (8×8) and the advanced pump (2×2, 4× the water). Their recipes are placeholders for now.
- `src/mail.rs`: agent telegrams: the inbox, `tfac msg` / `tfac hook`, and the Markdown renderer.
- `cargo test`: simulation tests plus screen snapshots (`target/snapshot*.html` show the map, tree and a letter in colour).
