# Terminactory

A cozy factory game in the terminal. It grows from basic items: copper, coal and iron, and the many parts, recipes and toys they lead to.

## Install

You need [Rust](https://rustup.rs) 1.88 or newer and a UTF-8 terminal. macOS and Linux are supported. On Windows, use [WSL](https://learn.microsoft.com/windows/wsl/install) for now.

Either way installs both commands, `terminactory` and its shortcut `tfac`.

From a clone of this repository:

```bash
cargo install --path .
```

Or straight from the Git remote, at a release tag (replace the URL and tag):

```bash
cargo install --git https://example.com/OWNER/terminactory --tag vX.Y.Z
```

To update, run the same command again with `--force` (and the new tag). To remove the game, `cargo uninstall terminactory`; your saves in `~/.terminactory` stay.

Check it worked:

```bash
tfac --version   # prints: terminactory 0.1.0
tfac --demo      # starts next to a small working factory
```

Make sure `~/.cargo/bin` is on your `PATH` (rustup normally does this). To try it without installing, `cargo run --release` in the source folder does the same as `tfac`.

## Run

`tfac` continues the save you played last. Saves are numbered (`save1`, `save2`, ...):

```bash
tfac --saves   # list them: money, research, time played, when last played
tfac save2     # open one (or just: tfac 2)
tfac --new     # start a fresh one as the next number; the others are kept
tfac --help    # everything tfac can do
```

`--demo` starts next to a small working factory with its own save. Saves, key settings (`controls.json`) and game settings (`settings.json`) live in `~/.terminactory` (saves in `saves/`), so the game finds them from any folder; it saves every 30 seconds, when you quit or close the terminal, and on Ctrl+S. A save can only be open in one window at a time: a second `tfac` on the same save says which window has it. Saves carry a format version: one made by an older version of the game isn't opened (and never deleted). `tfac` tells you, `tfac --saves` marks it "(old version)", and `tfac --new` starts a fresh one.

Best in a terminal at least 120×40. In anything narrower than 120 columns or shorter than 30 rows (say, a split next to your agent) it switches to a **small-pane view** by itself (the settings pane, `g`, can make that Always or Never, and change the width from 120 to 80, 100, 140 or 160; below 46×12 even Never gives way, since the full view cannot draw there): the map fills the pane, one line on top shows money, research, EC, idle machines and waiting mail, and the bottom line shows the build menu, what's under the cursor, or messages. `m` switches views by hand. Mouse works in most terminals (iTerm2, Terminal.app, WezTerm, Kitty).

## Agent telegrams: setup

Your AI agents can message you inside the game, so you can play while they work. A message costs the agent only the words in it, and the commands below print nothing on success. Messages wait while the game is closed (the newest 8 are kept). Pick the path that fits your agent, then run its test command with the game open.

### Claude Code (automatic, no tokens)

Add this to `~/.claude/settings.json`. When an agent finishes, its own last reply becomes the telegram; when it needs permission or input, you get a "needs you" telegram.

```json
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }]
  }
}
```

Test it by feeding the hook what Claude Code would:

```bash
printf '%s' '{"hook_event_name":"Stop","cwd":"/tmp/demo","last_assistant_message":"## It works\n- a telegram from the hook"}' | tfac hook
```

A telegram from `demo` should show up in the game. (`tfac hook` understands Claude Code's hook JSON only; for anything else use `tfac msg`.)

### Any other agent (`tfac msg`)

Anything that can run a shell command can send a telegram:

```bash
tfac msg --from my-agent "## Tests fixed\n- 3 flaky tests\n- [ ] needs review"
```

`\n` is a line break. With no text, `tfac msg` reads Markdown from stdin (`echo "## Done" | tfac msg`). `--from NAME` says who it's from; the default is the current folder's name. Markdown is rendered: headings, lists, checkboxes, quotes, code, tables, **bold**, `code`. To send a long report, pipe it in, or drop a `*.md` file into `~/.terminactory/inbox/` (an optional first line `from: NAME` sets the sender).

Two ways to use it:

- **Tell the agent.** Put a line like this in the agent's instructions (`AGENTS.md`, `GEMINI.md`, a system prompt, or whatever file it reads): *When you finish a task or need my input, run `tfac msg --from <project> "<two or three lines in Markdown>"`.* This works with any agent that can run commands, and costs a few tokens per message.
- **Use the agent's own hook or notify setting**, if it has one: point it at a command such as `tfac msg --from my-agent "finished"`, which costs no tokens. Check your agent's documentation for where that goes. Only Claude Code's hook has been tested with the game so far.

Test it:

```bash
tfac msg --from test "## Hello\n- this is a test telegram"
```

A telegram from `test` should appear at the bottom of the sidebar (in the small-pane view, a note in the corner). If nothing shows up, check that `tfac --version` works in the same shell your agent uses, and that the agent's shell has `~/.cargo/bin` on its `PATH`.

## What's in it

Phase 1: ore is melted in a furnace into molten metal, a molder casts it into ingots, and a press makes plates. Sell for money and research, research the press and coking, and make the four key items (copper wire, coke, copper pipe, copper coil), each bringing a letter from a company (letters arrive as telegrams, and never pause the game).

Phase 2: what those key items unlock.
- **Pumps & pipes** (after copper pipe): pumps draw water anywhere on the ground; pipes join into networks that each hold one fluid.
- **Basic solar panel** (Kettering & Vane, after copper coil; 1,800 RP or make 60 glass): the second version in the solar slot (press `3` `1` again). 2×2 like the cheap panel, but 20 EC and reach 4 for $300, against 10 EC, reach 2 and $100: $15 an EC instead of $10, you pay for fewer poles. A pole links it from up to 7 tiles away (5 for the cheap one). Panels still never link to each other, only through poles.
- **Steam engines** (after copper coil): boilers burn coal, or **waste** (4 waste burn like 1 coal), to turn water into steam, out of their front; steam engines turn steam into 100 EC each, only as much as their grid needs beyond solar, and a boiler burns only what its engines call for.
- **Blast furnace** (after coke): melts iron ore into molten iron fast (1 ore a go, 2.5 s: 24 molten iron a minute, twice a furnace's 12), or iron grit even better, makes molten steel from iron plate, and hardens plate (see Steel, below). Most of its recipes leave **waste**, which leaves by the **back** and has to go somewhere (see Waste below).

Phase 3: floors, elevators and machinery.
- **Floors**: support beams (after steel girders) open floor +1; excavation (after concrete) opens floor -1. Decking or digging costs $10 a tile (more on later floors), never any waiting. Drills underground mine 1.5x faster (2x and 3x on deeper floors later).
- **Elevators**: boxes from 2x2 to 8x8 (`+`/`-` to resize while placing), holding 10 items per tile. Each open floor gets a door; `f` sets a door to load, unload or pass. A car leaves only when full: 0.5 s doors, 1 s per floor, 0.5 s doors, then unloads at 30 items/s out of the door's arrow side.
- **Machinery**: the crude assembler (4×4, needs no EC, after an iron gear) builds small parts (see Parts, below); fast belts (after a gearbox) move twice as much.
- **Logistics toys** (research them in the Moving group):
  - **Belt tunnels** pass under up to 4 tiles. Place an entrance, then its exit up to 5 tiles further on facing the same way; build anything in between. They need no power.
  - **Long arm**: grabs from 2 tiles behind and drops 2 ahead, over whatever sits in between.
  - **Filter splitter**: `f` picks an item; that item goes out the front and everything else out the sides (with no item picked it shares like a splitter).
  - **Warehouse**: a 2×2 crate holding four crates' worth.
  - Pipes have their own slot, Transport 5, after belts / fast belts / tunnels, splitters / filter splitters, arms / long arms and elevators. Misc 1 is crate / warehouse.
- **Free unlocks**: everything also unlocks free once you've made enough of its milestone item (shown in the tree); research is the shortcut.
- **Idle machines** flash a word saying why: FULL, BACK, NEED, COAL, COKE, EC?, FLUD, WATR, ORE?, STM?. (EC? is no electric charge; FLUD is a molder waiting for molten metal; BACK is waste with nowhere to go out the back.)
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
- **Coke oven**: 2 coal → 1 coke + 1 waste (2 s). It cooks its own coal: no EC, no separate fuel. The furnace cooks coke the slow clean way: 3 coal → 1 coke (5 s).
- **Press** (retimed): copper wire 1 plate → 2 wire (5 s); copper pipe 2 plates → 1 pipe (2.5 s); copper coil 4 wire → 1 coil (5 s); iron gear 2 plates → 1 gear (2.5 s); steel girder 2 steel → 1 girder (5 s); hardened gear 2 hardened plate → 1 gear (5 s); crude sheets 3 copper plate → 1 copper sheet or 3 iron plate → 1 iron sheet (5 s; the rolling mill needs only 2 plates). The gearbox moved to the crude assembler.
- **Wire drawer**: 1 copper plate → 3 copper wire (4 s); 1 copper wire → 2 fine wire (4 s).
- **Winder**: 3 copper wire → 1 copper coil (4 s), or 6 fine wire → 2 copper coil (4 s: more drawers, half the winders); 2 iron rod → 1 spring (5 s).
- **Rolling mill**: 2 copper plate → 1 copper sheet (5 s); 2 iron plate → 1 iron sheet (2.5 s); 1 steel → 1 steel sheet (5 s); 3 steel → 2 steel girder (4 s).
- **Lathe**: 1 iron plate → 2 iron rod (5 s); 1 iron rod → 4 screw (5 s); 1 steel → 2 steel rod (5 s); 1 steel rod → 2 bearing (2.5 s).
- New items and prices: copper grit $0.60, iron grit $0.60, sand $0.25, fine wire $2.50, copper sheet $14, iron rod $5, screw $2, spring $15, iron sheet $18. Existing prices now: copper wire $3, copper pipe $12, copper coil $18, iron gear $18, coke $2.50. Glyphs: `:` grit (copper orange, iron blue) and sand (tan), `~` fine wire (pale), `-` sheet, `!` rod, `x` screw, `z` spring.
- **Companies now** (key item → what they offer): copper wire → Ashgrove (basic drill, crusher); coke → Cinderhall (blast furnace, coke oven); copper pipe → Wellbourne (pump, pipes); copper coil → Kettering (boiler, steam engine, wire drawer, winder); iron gear → Gearwright (assembler, lathe); steel girder → Pillar (floor +1, rolling mill). Swiftline's fast belts now open on a gearbox instead of an iron gear.

## Steel, stone, parts and big goods

Everything the earlier steps pointed at is real now. Times are seconds; prices are sell prices.

**Steel.** The blast furnace makes **molten steel** (it leaves the front into a pipe or a molder, which casts it: 1 molten steel → 1 steel, 2.5 s; steel $24). Three ways from plate, each also leaving waste items out the back (a belt or crate behind the furnace takes them), and a fourth straight from molten iron:

| Recipe | Time | Waste |
| --- | --- | --- |
| 2 iron plate + 1 coke | 10 | 1 |
| 2 iron plate + 2 coal | 10 | 3 |
| 2 iron plate + 2 coke | 5 | 2 |
| 3 molten iron + 1 coke (direct) | 10 | 2 |

The direct route takes molten iron from a pipe, or from an iron blast furnace standing right against it (no pipe, no cast, press or second melt) and gives molten steel out the front. The blast furnace also makes **hardened iron plate** ($14): 1 iron plate + 1 coke (5 s), or 1 iron plate + 2 coal (5 s, 2 waste). The press makes **hardened gear** ($50, 2 hardened plate, 5 s) and steel girder ($75, 2 steel, 5 s). Lathe: steel rod $16, bearing $12. Rolling mill: steel sheet $36, and 3 steel → 2 steel girder.

**Stone.** The **kiln** (3×5, burns coal) fires 8 at once: 2 stone → 1 **brick** ($1), 3 sand → 1 **glass** ($2), a firing takes 20 s (24 a minute): a batch machine fires for one run's time × its batch ÷ 2. The **large kiln** (5×7, $7,500, from Underhill; press the kiln's slot again to switch) has the same recipes and the same 20 s firing but a batch of 32: four kilns in one machine (96 a minute), so it takes 64 stone or 96 sand before it fires and burns 20 coal a firing, like four kilns. The **mixer** (2×2, 4 EC) takes water from a pipe: 2 sand + 1 stone + 1 water → 1 **concrete** ($2); 2 concrete + 1 iron rod + 1 water → 1 **reinforced concrete** ($16); 5 s each.

**Tubes.** A **tube** ($48) is a lamp's glass with the air taken out: lamp + copper sheet + 2 fine wire. The kiln bakes them the slow way ("tube (baked)": 1 tube + 1 waste out the back per run, a firing of 8 takes 40 s, 12 a minute; the large kiln does 32 per 40 s). Sablon's **evacuator** (2×2, 6 EC, $3,500, takes water from a pipe like the mixer) pulls the air out with a water jet instead: 2 tubes per run, 8 s, no waste, no fire.

**Parts** (assemblers; each recipe's time is for the lowest assembler that can make it, and higher ones run it faster: basic 2×, advanced 4× for crude parts; precision 2× for advanced parts).
- Crude assembler (10 s): valve $50 (copper pipe + iron gear), gauge $35 (glass + spring + copper sheet), lamp $15 (glass + 2 fine wire), relay $80 (copper coil + spring + iron sheet), gearbox $120 (3 iron gear + steel), transformer $200 (2 copper coil + 4 iron sheet), lamp matrix $230 (8 lamp + copper sheet + 4 fine wire), and a slow **hand-wired control board** (40 s: the same relay + 4 fine wire + iron sheet, no steel, gearbox or motor needed; 20 s in a basic assembler).
- Basic assembler (10 s): electric motor $160 (2 copper coil + steel rod + 2 bearing), piston $60 (steel rod + hardened plate + 2 screw), terminal $2,100 (lamp matrix + tube board; a keyboard joins it later).
- Advanced assembler (10 s): control board $170 (relay + 4 fine wire + iron sheet; it hides the slow hand-wired recipe). The control board is the "x1x1", the weakest microprocessor; the **tube board** $900 (15 s: 6 tube + control board + iron sheet) is the "x2x1". The precision assembler makes it in 5 s and refuses crude and basic parts; later processors will go there.
- Welder (10 s, 30 EC only while working): boiler shell $80 (steel sheet + 4 brick), silo shell $260 (3 steel sheet + valve), frame $280 (2 steel girder + 8 screw).

**Big goods.** Large basic assembler (20 s): pump unit $900 (motor + 2 valve + silo shell + gauge), engine block $1,150 (frame + 4 piston + gearbox + boiler shell). Large advanced assembler (also does those, twice as fast): generator $1,100 (frame + 2 motor + gauge + control board, 20 s), control panel $900 (2 control board + 4 lamp + 2 gauge + steel sheet, 20 s), **drilling rig** $3,500 (4 girder + 2 gearbox + 4 reinforced concrete + motor + pump unit, 40 s) and **steam tractor** $7,500 (engine block + frame + 4 hardened gear + control panel + generator, 60 s). The drilling rig and steam tractor are **crown items**: they go into nothing, only sell, and keep a high research floor so they keep paying research however many you sell.

Every other item except waste goes into at least one recipe (the transformer and terminal wait for the computer, a later phase).

**Companies and what they offer now** (research cost; the free milestone is the item to make):

| Company (key item) | Machine | Research | Free once you've made |
| --- | --- | --- | --- |
| Wellbourne (copper pipe) | Mixer | 1,500 | 1,000 stone |
| | Advanced pump (2×2, 4× the water) | 2,500 | 250 copper pipe |
| Underhill (concrete) | Excavation (floor -1) | 3,000 | 150 concrete |
| | Kiln | 2,000 | 250 concrete |
| | Large kiln (5×7, $7,500) | 10,000 | 1,000 brick |
| Pillar (steel girder) | Welder | 4,000 | 150 steel girder |
| Swiftline (gearbox) | Basic assembler (2×2) | 6,000 | 100 gearbox |
| | Upgrade planner (a tool, see Controls) | 3,000 | 60 gearbox |
| **Ampleworth** (electric motor) | Advanced drill (4×4, EC) | 8,000 | 40 motor |
| | Large basic drill (8×8, coal) | 20,000 | 150 motor |
| | Large basic assembler (5×5) | 25,000 | 300 motor |
| **Sablon** (tube) | Evacuator (2×2, 6 EC, $3,500) | 30,000 | 40 tube |
| **Lattice & Finch** (control board) | Advanced assembler (3×3) | 14,000 | 40 control board |
| | Precision assembler (3×3) | 30,000 | 120 control board |
| | Large advanced drill (8×8, EC) | 60,000 | 300 control board |
| | Large advanced assembler (5×5) | 100,000 | 800 control board |

Every key item can be made with machines from companies that open before it, so nothing locks: copper and the iron gear first (a press makes both, no blast furnace needed, so Gearwright opens early), then Cinderhall's steel, then Pillar, Underhill, Swiftline and Ampleworth. Lattice & Finch is a second road: a control board can be wired by hand in the crude assembler as soon as you have relays and fine wire, no steel or motors needed.

**Build menu.** Press a category's number, then a slot's number; press the slot again to switch kinds. A category holds up to 6 (Create), 5 (Transport), 4 (EC) or 3 (Misc) slots, filled in order. This layout is temporary: it will change after a playtest.

| Category | Slot | Kinds |
| --- | --- | --- |
| Create (1) | 1 | cheap / basic / advanced / large basic / large advanced drill |
| | 2 | furnace / molder / blast furnace / coke oven / kiln / large kiln |
| | 3 | press / wire drawer / winder / rolling mill / lathe / crusher |
| | 4 | assemblers (crude to large advanced) and the welder |
| | 5 | water pump / advanced pump / mixer / evacuator |
| Transport (2) | 1 | belt / fast belt / tunnel |
| | 2 | splitter / filter splitter |
| | 3 | arm / long arm |
| | 4 | elevator |
| | 5 | pipe |
| EC (3) | 1 | cheap solar panel / basic solar panel |
| | 2 to 4 | pole, boiler, steam engine |
| Misc (4) | 1 | crate / warehouse |
| | 2 | trade post |

Pipes moved from Create 5 to Transport 5. On a short screen the menu folds each group to one kind (marked `+`) until you pick one; in a small pane the bottom line drops prices and shortens names so every number shows.

## Molten metal

- **Furnace** (burns coal): copper ore → molten copper (1 → 1, 5 s); iron ore → molten iron (3 → 2, 10 s, slow); grit melts better (see above); with coking, 3 coal → coke (5 s). Press `f` on it to change the recipe.
- **Molder** (2 EC, $100, Create 2, next to the furnace): casts molten metal into ingots: copper ingot ($2), iron ingot ($1.50), steel (2.5 s each). `f` picks the recipe; it only takes the metal its recipe uses.
- **Press**: copper plate and iron plate come from ingots now (2.5 s), then wire, pipe, coil and the rest as before. Copper ingots are the free milestone for the press (make 50).
- **Fluids in pipes**: a machine that makes a fluid pushes it out of its **front**, into a pipe touching that edge, or straight into a machine that takes it (a furnace right behind a molder needs no pipe). A machine that needs a fluid takes it from pipes on any of its other sides. A pipe network holds one fluid at a time: water blue, steam white, molten copper orange, molten iron red, molten steel pale yellow, grey when empty. Pointing at a pipe or a machine names the fluid.
- A machine can have two outputs (a fluid out the front and items out the front). It waits, saying FULL, until both have room. Waste is its own output, out the back (see Waste below).
- **Stone** ($0.30): drills can mine it (`f` cycles copper, coal, iron, stone). A cheap drill cuts iron at half the speed of a basic drill.

## Recipe book

`b` opens it on whatever is under the cursor. It has a page for every item (Items tab) and for each machine that makes or mines them (Machines tab: every drill, crusher, furnace, molder, blast furnace, coke oven, kiln, mixer, press, wire drawer, winder, rolling mill, lathe, welder, every assembler and the advanced pump). When several assemblers make an item, one equation is shown with each assembler's time.

- **Item pages:** what makes it, as a word equation with the fuel over the arrow (`copper ore ─coal─→ molten copper`), plus timing and anything still locked. Also what uses it, what it unlocks, and a chart of research per sale that marks its floor and where you are.
- **Machine pages:** cost, whether you can build it (and what opens it if not), and everything it makes.
- **Tabs:** Tab switches between the Items tab and the Machines tab.
- **Links:** blue names are links: click one to go there, and Backspace goes back.
- **Search:** `/` filters the list as you type; Enter stops typing, Esc clears it.

## Production and history

The production panel (`o`) has two tabs, switched with `Tab` (or a click): **Production** shows the last minute per item, and **History** charts what you have earned. It charts money and research per second, minute, hour or day: pick the range with `1`-`4` (pause and speed keep working with the panel open). A tall pane shows both charts stacked; a short one shows one, and `r` swaps money and research. Days are your local calendar days; company deliveries are not counted as earnings, what the factory earns while the game is closed is counted in the hour and day you come back, and the seconds view covers this session only. The game keeps running underneath.

## Agent telegrams

Your AI agents can message you inside the game. Each message shows up as a telegram (Markdown rendered: headings, lists, checkboxes, quotes, code, tables, **bold**, `code`). Every few minutes a company sends a small delivery with one: about a minute of your factory's earnings.

Setup for Claude Code and other agents is under *Agent telegrams: setup*, above.

Telegrams sit at the bottom of the sidebar while the game keeps running: `n` or Esc closes one and shows the next, Shift+N dismisses all, PgUp/PgDn or the mouse wheel scroll a long one. In the small-pane view a note in the bottom-right corner ("agent finished · n to view") says one is waiting, and `n` opens it. Messages wait while the game is closed (only the newest 8 are kept waiting). Two quieter styles are in the settings pane (`g`, Telegram style): **Note** shows only the short "agent finished · n to view" note in both views, and **Ticker** only a mail count in the top line; with either, `n` opens the telegram over the map. Every telegram and company letter is also kept in a log (the last 50, saved with the game): `i` opens it, newest first with unread ones marked, and Enter reads one again.

## Hints

When nothing else is on the bottom line, the game may say one quiet thing about what you have already built, worded as a diagnosis and never an order: "Your drill is out of coal: put a drill on coal next to it", "No trade post yet: items have nowhere to go", belts and molders with no EC, a full output, a machine waiting for inputs, "Research comes from selling: f on a trade post", or "You can afford a new part: t". A state has to hold for 20 seconds first; a hint stays 12 seconds, waits 3 minutes before it can return, and shows at most 3 times a session. In the full view, at most one company aside every 10 minutes (the first after 8 minutes, each line at most twice) says something about what you built, in that company's voice. The line is ranked: a message from something you just did comes first, then what is stuck under the cursor, then hints, then asides, then the key list. Settings (`g`) has one switch, Hints, that turns hints and asides off. Nothing is saved.

## Rules in one breath

- Buildings take input from **any side** and send their products out of their **front** (the arrow on their edge): onto a belt, or straight into the next building, like a drill directly into a furnace. Waste is the one exception: it leaves by the **back**, the edge opposite the arrow.
- Sizes follow roles: belts, splitters, arms and crates are 1×1; drills, solar panels and elevators are 2×2; furnaces, lathes and the trade post are 3×3. Presses, wire drawers and winders (2×1) and pumps (1×2) are long machines: the size given is facing east or west, and they swap width and height when turned north or south. The rotate key (`r`) shows the turned shape while placing, and turning a placed one needs the room (it keeps its top-left corner).
- Drills mine anywhere on the ground (not on decking): press f on one to pick copper, coal, iron or stone. The letter on a drill shows its ore (C copper, * coal, I iron). Drills burn coal; a drill on coal fuels itself. Middle click copies a drill's ore to the next one you place.
- Furnaces, molders and presses make the recipe they're set to (`f` changes it). Furnaces burn coal to melt ore; with coking researched they bake coal into coke. Molders cast ingots, presses turn ingots into plates, plates into wire and pipe, and wire into coils. A basic drill cuts iron twice as fast as a cheap one.
- Research (`t`) brings new parts, in groups (Making, Assembling, Drilling, Moving, Pipes, EC, Floors, Tools; the list scrolls); some open only once you've made a key item (copper wire, coke, copper pipe, copper coil, iron gear, gearbox, girders, concrete, electric motor, control board), and everything also unlocks free once you've made enough of its milestone item. Enter researches the picked row; clicking a row picks it and clicking it again researches it.
- Power is called **EC** (electric charge). Belts, splitters, arms, molders, presses and the other shaping machines need it (coke ovens don't). Solar panels (reach 2 tiles; the basic panel 4) and EC poles (reach 3 tiles) charge every electric part within range; ranges that overlap join into one grid, and each grid shares only its own EC. Solar panels never join each other directly, even side by side: link them with poles. The EC figure at the top is **regional** (the grid of the pole, panel or machine under the cursor, and hovering a pole or panel shades just that grid) or **global** (all grids together, with any supply that powers nothing shown as unused). Too little EC slows a grid's parts evenly. Elevators carry EC to the floor below. Belts without EC turn red; `v` shows the area it reaches.- Trade posts sell what they receive: money, a 40/40 split, or research (press `f` on one).
- **Waste** leaves a machine by its **back** edge, never the front: put a belt (facing away), crate or trade post behind the coke oven or blast furnace, so a row of machines can share a waste alley behind it. A belt facing into the machine doesn't take it. If nothing behind can take waste, the machine fills up on it, stops, and flashes `BACK` (its products leave the front as usual until then). A **boiler** takes waste as fuel (from a belt, or straight from a machine whose back it touches): 4 waste give the fire of 1 coal, mixed freely with coal, and it burns only as much as its grid needs, so a boiler at full load (one engine, 100 EC) eats 15 coal or 60 waste a minute. Whatever it can't burn stays on the belt. Waste sells for -$0.50 each and never earns research. Only a trade post set to money takes it (and pays the fee); research and split posts refuse it, so it stays on the belt and backs up (the post flashes `WSTE`). A post also refuses waste when the fee would take your money below zero, or when the last minute's waste fees are more than the last minute's earnings. Removing a crate, warehouse or elevator that holds waste costs the same $0.50 each (refused if you can't afford it); hover it to see the cost. Removing an area pays each fee once, and asks first when the total is over $20. The production panel (`o`) shows waste fees as negative money.
- Research per item fades the more you sell of it, down to a small per-item minimum, so new items are where research comes from. Research earned while the game is closed fades the same way.
- Nothing breaks or jams at random. Removing a part refunds it in full.

## Controls

| Action | Keys |
| --- | --- |
| Move | arrows / WASD |
| Lay belts | Shift+arrows / Shift+WASD, or drag with the mouse |
| Build menu | 1–4 opens a category (1 Create, 2 Transport, 3 EC, 4 Misc); inside it, a number picks a part (Create has 1–5, Transport 1–5), and pressing the same number again switches kind (the drills, belt / fast belt / tunnel, the assemblers, pump / advanced pump / mixer...); `` ` ``, `₹`, `~` or Esc goes back |
| Place / remove | Space or click / x or right-click |
| Area tool | `c` starts a rectangle: press Space (or Enter) at one corner, move, and Space again at the opposite corner, or drag with the left button. The rectangle is tinted and the parts in it light up. Then `x` removes everything in it, with a full refund, `c` copies it (see Blueprints), or `u` upgrades it (see Upgrade planner). Esc or a right-click cancels; a digit or a floor change closes the tool too. Waste held in crates, warehouses and elevators costs the usual $0.50 each, once for the whole area, and over $20 you press `x` a second time to pay. Elevators go with all their doors and their car, and a long machine with one tile in the rectangle comes whole. `z` puts back what the last area remove took (settings and contents too, at the full price again), or swaps back what the last upgrade changed, whichever came last |
| Upgrade planner | Needs the **Upgrade planner** tech (Swiftline Conveyors, in the Tools group: 3,000 RP, or free once you have made 60 gearboxes); until then `u` says what unlocks it and changes nothing. With a rectangle chosen, `u` swaps each part that has a next version for it, **in place**: the same spot, facing and settings, and you pay only the difference in price (money, nothing else). It is a swap for another machine, never a change to the one you have. The pairs: basic belt → fast belt, cheap drill → basic drill, cheap solar → basic solar, advanced assembler → precision assembler, large basic assembler → large advanced assembler, large basic drill → large advanced drill (a coal drill becomes an EC drill, so give it power). Each pair has the same footprint, so a layout never moves. Parts in the rectangle that can be upgraded are tinted green once it is chosen, and the sidebar counts them. The first `u` shows a one-line preview ("u again to upgrade 12 parts for $360"); the second does it. What a part holds comes along where the new one can hold it: belts keep every item, a drill its ore and output (coal it can no longer burn is dropped), an assembler its stock, output and recipe (matched **by name**; its progress starts over). An assembler whose recipe the new version cannot make (a precision assembler makes none of the crude or basic ones) is left as it is and counted. It goes in reading order and stops at the first part you cannot pay for, like a paste: the message counts what was upgraded, what is short of money (and by how much), and what is locked (a target you have not researched); press `u` twice again after earning more. `z` swaps the last upgrade back and pays the difference back (parts removed or changed since are left alone). Not included: arm → long arm (reach changes what it grabs), pump → advanced pump and crate → warehouse (the footprint grows) |
| Blueprints | Needs the **Blueprints** tech (Brannock & Daughters, in the Tools group: 800 RP, or free once you have made 1,500 copper plate); until then `c` on a chosen rectangle says so. With a rectangle chosen, `c`, Space or Enter copies it to your hand (a middle-drag does it in one go). Everything the rectangle touches comes, long machines whole, with its facing and settings: arm and splitter filters, trade post modes, a drill's ore, an elevator's size and door, tunnel entrances and exits, and recipes **by name**. Items, fuel and progress are not copied: a blueprint is a layout. A ghost follows the cursor with its top-left corner there: yellow sprites will be placed, `xx` on red can't be (something in the way, locked, off the map, or the wrong floor for it), `..` on orange can't be afforded yet; parts already standing there aren't drawn over. `r` turns it a quarter clockwise, Shift+`r` back; Space or a click pastes, Esc empties the hand. A paste places every part that fits and that you can afford, **in reading order** (top row first, left to right), and costs each part's price plus any digging or decking; press again at the same spot to fill in the rest as you earn, and parts already there cost nothing, so nothing is charged twice. Dragging never repeats a paste. Tunnel pairs stay pairs when turned; a recipe you haven't researched is left at the machine's first one (the message counts them); an elevator needs room on every open floor and gets default (unload) doors on the floors other than the one it was copied from. The blueprint stays in your hand across floors, so you can copy on the ground and paste in the basement |
| Blueprint library | `l` (needs Blueprints): the saved blueprints over the map (the game keeps running). Up/Down pick, Enter (or a second click) takes one into your hand, `f` saves the blueprint in your hand (or else the last one you copied) under a name you type, `x` or Delete twice removes the picked file, Esc or `l` closes. Blueprints are plain JSON files in `~/.terminactory/blueprints`, one per blueprint, shared by every save; each has its own `format` number (currently 1), so saves are untouched. A file from a newer game, or one that can't be read, is listed as such with the reason and is never changed or deleted. A blueprint that uses a part you haven't unlocked yet shows `(Kiln locked)` and still pastes the rest |
| Copy a part | middle-click it: picks up its kind, facing and setting (recipe, arm filter, trade post mode), so clicks place copies; middle-click empty ground to empty your hand (a middle-drag copies a rectangle instead, see Blueprints) |
| Rotate (belt direction / output side) | r |
| Set up (arm filter, trade post mode) | f |
| Change floor | < > or scroll |
| Research | t (arrows pick, Enter researches; click a row, click again to research) |
| Save | Ctrl+S (it also saves every 2 minutes and on quit) |
| Production panel | o: the last minute per item (made, used, sold, money, research, what the next sale pays); the game keeps running |
| Settings | g: the sidebar earnings graph (Off / Money / Research / Both) and its range (Seconds / Minutes / Hours), the small-pane view (Auto / Always / Never) and the width Auto switches at (80 / 100 / 120 / 140 / 160 columns), the telegram style (Sidebar / Note / Ticker) and Hints (On / Off); arrows or j/k pick a row, Left/Right or Enter change it, Esc closes; the game keeps running. Saved in `~/.terminactory/settings.json`; a file from an older version, or with fields missing, still loads |
| Show EC range | v |
| Elevator size | + / - |
| Pause / speed | p / [ ] |
| Recipe book (opens on what's under the cursor) | b |
| Small-pane view on/off (for this session) | m |
| Next telegram / dismiss all | n / Shift+N (PgUp/PgDn scroll; Esc also closes) |
| Telegram log | i (Enter reads one again) |
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
| `SOLR` | Cheap solar panel (10 EC) | 2×2 | |
| `SOL2` | Basic solar panel (20 EC, reach 4) | 2×2 | |
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
| `LATHE` | Lathe (R rod, S screw, r steel rod, b bearing) | 3×3 | |
| `KILN` `WELDER` `MIXR` `EVAC` | Kiln (3×5), welder (3×3), mixer (2×2), evacuator (2×2) | | `b` brick, `u` glass, `C` reinforced concrete, `t` tube |
| `BASM` `ADVASM` `PRCASM` | Basic (2×2), advanced (3×3), precision (3×3) assembler | | `V` valve `Q` gauge `L` lamp `r` relay `M` motor `P` piston `B` control board |
| `LRG ASM B` `LRG ASM A` | Large basic and advanced assembler | 5×5 | `D` boiler shell `S` silo shell `F` frame `U` pump unit `E` generator `N` engine block `T` control panel `Y` steam tractor |
| `ADV DRL` `LRG … DRILL` `APMP` | Advanced drill (4×4), large drills (8×8), advanced pump | | |

## Code

- `src/sim.rs`: the simulation. 60 ticks per second, no randomness, no drawing.
- `src/ui.rs`: terminal drawing (ASCII on a fixed dark map).
- `src/app.rs`: keyboard and mouse.
- `src/content.rs`: items, companies, research and recipes (data, kept apart from mechanics).
- Saves are format version 3 (the item and tech lists grew); older saves aren't opened.
- `src/hints.rs`: the hints and asides for the bottom line, their triggers and pacing (`ui::status_slot` ranks the slot).
- `src/mail.rs`: agent telegrams: the inbox, `tfac msg` / `tfac hook`, and the Markdown renderer.
- `cargo test`: simulation tests plus screen snapshots (`target/snapshot*.html` show the map, tree and a letter in colour).

## License

Apache-2.0. See `LICENSE`.
