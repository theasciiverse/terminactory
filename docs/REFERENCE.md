# Terminactory reference

The details of the game: machines, recipes, prices and keys. This file grew with the game and is a reference, not a tutorial; the README has the short version, and the in-game wiki (`b`) is the page to trust for current numbers.

## What's in it

Phase 1: ore is melted in a furnace into molten metal, a molder casts it into ingots, and a press makes plates. Sell for money and research, research the press and coking, and make the four key items (copper wire, coke, copper pipe, copper coil), each bringing a letter from a company (letters arrive as telegrams, and never pause the game).

Phase 2: what those key items unlock.
- **Pumps & pipes** (after copper pipe): pumps draw water anywhere on the ground; pipes join into networks that each hold one fluid.
- **Basic solar panel** (Kettering & Vane, after copper coil; 1,800 RP or make 60 glass): the second version in the solar slot (press `3` `1` again). 2×2 like the cheap panel, but 20 EC and reach 4 for $300, against 10 EC, reach 2 and $100: $15 an EC instead of $10, you pay for fewer poles. A pole links it from up to 7 tiles away (5 for the cheap one). Panels still never link to each other, only through poles.
- **Steam engines** (after copper coil): boilers burn coal to turn water into steam, out of their front; steam engines turn steam into 100 EC each, only as much as their grid needs beyond solar, and a boiler burns only what its engines call for.
- **Blast furnace** (after coke): melts iron ore into molten iron fast (1 ore a go, 2.5 s: 24 molten iron a minute, twice a furnace's 12), or iron grit even better, makes molten steel from iron plate, and hardens plate (see Steel, below). Most of its recipes leave **waste**, which leaves by the **back** and has to go somewhere (see Waste below).

Phase 3: floors, elevators and machinery.
- **Floors**: support beams (after steel girders) open floor +1; excavation (after concrete) opens floor -1. Decking or digging costs $10 a tile (more on later floors), never any waiting. Drills underground mine 1.5x faster (2x and 3x on deeper floors later).
- **Elevators**: boxes from 2x2 to 8x8 (`+`/`-` to resize while placing), holding 10 items per tile. Each open floor gets a door; `f` sets a door to load, unload or pass. A car leaves only when full: 0.5 s doors, 1 s per floor, 0.5 s doors, then unloads at 30 items/s out of the door's arrow side.
- **Machinery**: the crude assembler (4×4, needs no EC, after an iron gear) builds small parts (see Parts, below); fast belts (after a gearbox) move twice as much.
- **Logistics toys** (research them in the Moving group):
  - **Belt tunnels** pass under up to 4 tiles. Place an entrance, then its exit up to 5 tiles further on facing the same way; build anything in between. They need no power.
  - **Long arm**: grabs from 2 tiles behind and drops 2 ahead, over whatever sits in between.
  - **Filter splitter**: `f` picks an item; that item goes out the front and everything else out the sides (with no item picked it shares like a splitter).
  - **Overflow gate** (comes with the filter splitter): everything goes out the front; only when the front can't take an item does it turn to the left and right, taking turns.
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

**Silicon.** No new raw material and no new machine: sand and coke go into the blast furnace ("molten silicon": 3 sand + 1 coke → molten silicon + 2 waste, 10 s, the dirty way), a molder casts it into a **silicon ingot** ($6), and the lathe slices 2 ingots into 2 **wafers** ($12) and 2 **silicon scrap** ($1.50, which sells) in 40 s. The kiln fires **dopant** ($10: 4 stone, 20 s a batch), **doped wafers** ($50: wafer + 2 dopant, 32 s) and **ceramic** ($8: 2 brick + sand → 2, 32 s), all burning coal; the large kiln does them too. The advanced and precision assemblers build the **circuit board** ($60: ceramic + copper sheet + 4 fine wire), the **transistor** ($42: doped wafer + 6 fine wire + iron sheet → 3) and the **transistor board** ($450, the x3x1: 6 transistors + circuit board + 4 fine wire), 10 s each. Making a silicon ingot opens Halverson Smelting, whose machines are a better road (below).

**Chips.** Four new items and a machine, from the same raw materials. The mixer makes **etchant** ($2: coal + sand + water → 2 etchant + 1 waste out the back, 5 s). The advanced and precision assemblers build a **mask** ($40: glass + copper sheet + 2 fine wire, 10 s). Lattice & Finch's **etcher** (3×3, 24 EC, $25,000, sixth slot of Create) etches a doped wafer with a mask and etchant into 2 **dies** ($68) and 2 silicon scrap, 15 s. The precision assembler alone turns a die + ceramic + fine wire into a **microprocessor (x4x1)** ($200: 4 fine wire) or a **RAM chip** ($150: 2 fine wire), 10 s each. Making a microprocessor opens Meridian Instruments.

**Fine etching.** Meridian's first machine is a better road, never the only one: the etcher's die and the x4x1 stay. The precision assembler builds a **fine mask** ($90: mask + glass + 2 fine wire, 10 s) and, from a **fine die** ($140), the **microprocessor (x4x2)** ($420: fine die + ceramic + 4 fine wire, 10 s). The **fine etcher** (3×5, 96 EC, $60,000, last in slot 6 of Create) etches 1 doped wafer + 1 fine mask + 1 etchant into 1 fine die and 1 die, 20 s, with no scrap and both out the front: very hungry, but every wafer gives a fine die.

**Better silicon.** Halverson's three machines (all EC, no fuel) are better roads, never the only ones: the blast furnace and lathe routes stay. The **arc furnace** (3×3, 40 EC, $10,000) melts silicon with no fuel: 3 sand + 1 coke → 2 molten silicon + 1 waste (10 s), 3 sand + 2 coal → 1 + 2 waste, or a clean remelt of 3 silicon scrap → 2 molten silicon (5 s). The **crystal grower** (2×4, 12 EC, $7,500) grows a **silicon crystal** ($50) from 3 silicon ingots in 20 s, the slowest machine in the game. The **wafer saw** (2×1, 6 EC, $6,000) saws a crystal and a fine wire into 6 wafers in 10 s, with no scrap: twice the lathe's wafers per ingot.

**Parts** (assemblers; each recipe's time is for the lowest assembler that can make it, and higher ones run it faster: basic 2×, advanced 4× for crude parts; precision 2× for advanced parts).
- Crude assembler (10 s): valve $50 (copper pipe + iron gear), gauge $35 (glass + spring + copper sheet), lamp $15 (glass + 2 fine wire), relay $80 (copper coil + spring + iron sheet), gearbox $120 (3 iron gear + steel), transformer $200 (2 copper coil + 4 iron sheet), keyboard $260 (4 spring + circuit board + iron sheet), lamp matrix $230 (8 lamp + copper sheet + 4 fine wire), and a slow **hand-wired control board** (40 s: the same relay + 4 fine wire + iron sheet, no steel, gearbox or motor needed; 20 s in a basic assembler).
- Basic assembler (10 s): electric motor $160 (2 copper coil + steel rod + 2 bearing), piston $60 (steel rod + hardened plate + 2 screw), terminal $2,100 (lamp matrix + keyboard + tube board), disk drive $1,100 (electric motor + steel sheet + transistor board).
- Advanced assembler (10 s): control board $170 (relay + 4 fine wire + iron sheet; it hides the slow hand-wired recipe). The control board is the "x1x1", the weakest microprocessor; the **tube board** $900 (15 s: 6 tube + control board + iron sheet) is the "x2x1". The precision assembler makes it in 5 s and refuses crude and basic parts; it alone builds the x4x1 microprocessor and RAM chip, and the **mainboard** (see The computer). Both assemblers build the **mask** (see Chips).
- Welder (10 s, 30 EC only while working): boiler shell $80 (steel sheet + 4 brick), silo shell $260 (3 steel sheet + valve), frame $280 (2 steel girder + 8 screw).

**Big goods.** Large basic assembler (20 s): pump unit $900 (motor + 2 valve + silo shell + gauge), engine block $1,150 (frame + 4 piston + gearbox + boiler shell). Large advanced assembler (also does those, twice as fast): generator $1,100 (frame + 2 motor + gauge + control board, 20 s), control panel $900 (2 control board + 4 lamp + 2 gauge + steel sheet, 20 s), **drilling rig** $3,500 (4 girder + 2 gearbox + 4 reinforced concrete + motor + pump unit, 40 s) and **steam tractor** $7,500 (engine block + frame + 4 hardened gear + control panel + generator, 60 s). The drilling rig and steam tractor are **crown items** (the computer is the third): they go into nothing, only sell, and keep a high research floor so they keep paying research however many you sell.

**The computer.** The branch's one crown item, and Meridian's second machine. The precision assembler builds a **mainboard** ($1,800: circuit board + microprocessor (x4x2) + 4 RAM chip, 10 s), or the coarse way, "mainboard (two coarse)", with 2 x4x1 in place of the one x4x2 (20 s: one x4x2 does the work of two x4x1). The basic assembler builds a **disk drive** and the crude one a **keyboard** (see Parts). The **large precision assembler** (5×5, 160 EC, $100,000, in the assembler slot of Create right after the large advanced assembler) runs the large advanced goods twice as fast, builds nothing below them, and alone builds the **computer** ($11,000: mainboard + transformer + terminal + disk drive + frame, 60 s). The computer is worth 22,000 research, never goes into anything, and keeps the highest research floor in the game.

Every item except waste goes into at least one recipe, except the crown items (the drilling rig, steam tractor and computer), which only sell.

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
| **Halverson** (silicon ingot) | Arc furnace (3×3, 40 EC, $10,000) | 40,000 | 100 silicon ingot |
| | Crystal grower (2×4, 12 EC, $7,500) | 45,000 | 200 silicon ingot |
| | Wafer saw (2×1, 6 EC, $6,000) | 35,000 | 100 wafer |
| **Meridian** (microprocessor x4x1) | Fine etcher (3×5, 96 EC, $60,000) | 150,000 | 100 microprocessor (x4x1) |
| | Large precision assembler (5×5, 160 EC, $100,000) | 300,000 | 40 mainboard |
| **Lattice & Finch** (control board) | Advanced assembler (3×3) | 14,000 | 40 control board |
| | Precision assembler (3×3) | 30,000 | 120 control board |
| | Etcher (3×3, 24 EC, $25,000) | 80,000 | 60 transistor board |
| | Large advanced drill (8×8, EC) | 60,000 | 300 control board |
| | Large advanced assembler (5×5) | 100,000 | 800 control board |

Every key item can be made with machines from companies that open before it, so nothing locks: copper and the iron gear first (a press makes both, no blast furnace needed, so Gearwright opens early), then Cinderhall's steel, then Pillar, Underhill, Swiftline and Ampleworth. Lattice & Finch is a second road: a control board can be wired by hand in the crude assembler as soon as you have relays and fine wire, no steel or motors needed.

**Build menu.** Press a category's number, then a slot's number; press the slot again to switch kinds. A category holds up to 6 (Create), 5 (Transport), 4 (EC) or 3 (Misc) slots, filled in order. This layout is temporary: it will change after a playtest.

| Category | Slot | Kinds |
| --- | --- | --- |
| Create (1) | 1 | cheap / basic / advanced / large basic / large advanced drill |
| | 2 | furnace / molder / blast furnace / coke oven / kiln / large kiln / arc furnace |
| | 3 | press / wire drawer / winder / rolling mill / lathe / crusher |
| | 4 | assemblers (crude to large precision) and the welder |
| | 5 | water pump / advanced pump / mixer / evacuator |
| | 6 | etcher / crystal grower / wafer saw / fine etcher |
| Transport (2) | 1 | belt / fast belt / tunnel |
| | 2 | splitter / filter splitter / overflow gate |
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
- **Fluids in pipes**: a machine that makes a fluid pushes it out of its **front**, into a pipe touching that edge, or straight into a machine that takes it (a furnace right behind a molder needs no pipe). A machine that needs a fluid takes it from pipes on any of its other sides. A pipe network holds one fluid at a time: water blue, steam white, molten copper orange, molten iron red, molten steel pale yellow, crude oil dark brown, naphtha orange, fuel oil olive, ethylene pink, grey when empty. Pointing at a pipe or a machine names the fluid.
- A machine can have two outputs (a fluid out the front and items out the front). It waits, saying FULL, until both have room. Waste is its own output, out the back (see Waste below).

## Oil

**Oil** is a second liquid line, made and sold with three machines from Wellbourne and Halverson.

**Crude oil** ($2) comes out of the ground. The **oil extractor** (1×5, 6 EC, $400, Wellbourne; research 800 RP or make 200 copper pipe) draws one unit every two seconds into a pipe at its front, and stops when that pipe is full. It works on any open ground, and deeper floors are richer, like the drills'. The first crude oil opens **Halverson Smelting** (its letter depends on which of its keys opened it: silicon ingot or crude oil).

The **still** (2×3, no EC, no fuel, $350, Halverson; research 1,500 RP or pump 400 crude oil) sits in a new seventh slot of Create, **chemistry** (the refinery and bottler will join it). It takes crude oil and naphtha from pipes and coal from a belt: 6 crude oil → 2 **naphtha** ($7) + 1 waste, 3 naphtha → 2 **ethylene** ($11) + 1 waste, and 4 coal + 1 **fuel oil** ($2) → 2 **carbon black** ($3) + 1 waste. Fuel oil has no maker until the refinery of a later phase, so the carbon black recipe waits in the book. Carbon black sells but goes into nothing yet.

The **liquid trader** (2×2, 2 EC, $300, Halverson; research 1,200 RP or pump 300 crude oil) is a trade post for fluids: it sells up to 4 units a second out of any pipe it touches, at that fluid's price, in any of the trade post's three modes (`f`). It refuses water and steam, and its research fades by units sold like an item's. It shares the trade post's slot in Misc (press the number again to switch). Its earnings show in the production panel beside the items'.
- **Stone** ($0.30): drills can mine it (`f` cycles copper, coal, iron, stone). A cheap drill cuts iron at half the speed of a basic drill.

## The browser

`b` opens **termweb**, the game's own browser, on whatever is under the cursor. Its start page is a search box: search the wiki and the rest of the in-game web, or follow the links under the box. Tabs, bookmarks, history and an address bar work as you would expect. Inside it, `b` opens a new tab, `Esc` puts the browser away with its tabs kept, and `1`-`9` pick a tab when no box has the keys. The coloured dots top left close it, minimize it, or make it fullscreen (double-clicking the title bar or tab strip does the same).

The wiki (`terminactory.wiki`) has a page for every item (Items) and every machine that makes or mines them (Machines):

- **Item pages:** what makes it, as a word equation with the fuel over the arrow (`copper ore ─coal─→ molten copper`), timing, anything still locked, what uses it and what it unlocks. The infobox charts research per sale as it fades toward the item's floor, and marks where you are. A "Research path" section shows where the item sits in the research tree.
- **Machine pages:** cost, whether you can build it (and what opens it if not), and everything it makes.
- **Links:** names are links: click one to go there, and Backspace goes back. Copy page (bottom bar) puts a page's text on the clipboard.

`o` opens **production** (`production.lan`) and `t` opens **research** (`research.lan`) as sites in the same browser, each in a tab of its own.

## Production and history

The production site (`o`) has two tabs, switched with `Tab` (or a click): **Production** shows the last minute per item, and **History** charts what you have earned. It charts money and research per second, minute, hour or day: pick the range with `1`-`4` (pause and speed keep working). A tall pane shows both charts stacked; a short one shows one, and `r` swaps money and research. Days are your local calendar days; company deliveries are not counted as earnings, what the factory earns while the game is closed is counted in the hour and day you come back, and the seconds view covers this session only. The game keeps running underneath.

## Agent telegrams

Your AI agents can message you inside the game. Each message shows up as a telegram (Markdown rendered: headings, lists, checkboxes, quotes, code, tables, **bold**, `code`). Every few minutes a company sends a small delivery with one: about a minute of your factory's earnings.

Setup for Claude Code and other agents is in the README.

Telegrams sit at the bottom of the sidebar while the game keeps running: `n` or Esc closes one and shows the next, Shift+N dismisses all, PgUp/PgDn or the mouse wheel scroll a long one. In the small-pane view a note in the bottom-right corner ("agent finished · n to view") says one is waiting, and `n` opens it. Messages wait while the game is closed (only the newest 8 are kept waiting). Two quieter styles are in the settings pane (`g`, Telegram style): **Note** shows only the short "agent finished · n to view" note in both views, and **Ticker** only a mail count in the top line; with either, `n` opens the telegram over the map. Every telegram and company letter is also kept in a log (the last 50, saved with the game): `i` opens it, newest first with unread ones marked, and Enter reads one again.

## Hints

When nothing else is on the bottom line, the game may say one quiet thing about what you have already built, worded as a diagnosis and never an order: "Your drill is out of coal: put a drill on coal next to it", "No trade post yet: items have nowhere to go", belts and molders with no EC, a full output, a machine waiting for inputs, "Research comes from selling: f on a trade post", or "You can afford a new part: t". A state has to hold for 20 seconds first; a hint stays 12 seconds, waits 3 minutes before it can return, and shows at most 3 times a session. In the full view, at most one company aside every 10 minutes (the first after 8 minutes, each line at most twice) says something about what you built, in that company's voice. The line is ranked: a message from something you just did comes first, then what is stuck under the cursor, then hints, then asides, then the key list. Settings (`g`) has one switch, Hints, that turns hints and asides off. Nothing is saved.

## Rules in one breath

- Buildings take input from **any side** and send their products out of their **front** (the arrow on their edge): onto a belt, or straight into the next building, like a drill directly into a furnace. Waste is the one exception: it leaves by the **back**, the edge opposite the arrow.
- Sizes follow roles: belts, splitters, arms and crates are 1×1; drills, solar panels and elevators are 2×2; furnaces, lathes and the trade post are 3×3. Presses, wire drawers and winders (2×1), pumps (1×2) and the oil extractor (1×5) are long machines: the size given is facing east or west, and they swap width and height when turned north or south. The rotate key (`r`) shows the turned shape while placing, and turning a placed one needs the room (it keeps its top-left corner).
- Drills mine anywhere on the ground (not on decking): press f on one to pick copper, coal, iron or stone. The letter on a drill shows its ore (C copper, * coal, I iron). Drills burn coal; a drill on coal fuels itself. Middle click copies a drill's ore to the next one you place.
- Furnaces, molders and presses make the recipe they're set to (`f` changes it). Furnaces burn coal to melt ore; with coking researched they bake coal into coke. Molders cast ingots, presses turn ingots into plates, plates into wire and pipe, and wire into coils. A basic drill cuts iron twice as fast as a cheap one.
- Research (`t`, in the browser) brings new parts, in groups (Making, Assembling, Drilling, Moving, Pipes, EC, Floors, Tools; the list scrolls); some open only once you've made a key item (copper wire, coke, copper pipe, copper coil, iron gear, gearbox, girders, concrete, electric motor, control board), and everything also unlocks free once you've made enough of its milestone item. Enter researches the picked row; clicking a row picks it and clicking it again researches it.
- Power is called **EC** (electric charge). Belts, splitters, arms, molders, presses and the other shaping machines need it (coke ovens don't). Solar panels (reach 2 tiles; the basic panel 4) and EC poles (reach 3 tiles) charge every electric part within range; ranges that overlap join into one grid, and each grid shares only its own EC. Solar panels never join each other directly, even side by side: link them with poles. The EC figure at the top is **regional** (the grid of the pole, panel or machine under the cursor, and hovering a pole or panel shades just that grid) or **global** (all grids together, with any supply that powers nothing shown as unused). Too little EC slows a grid's parts evenly. Elevators carry EC to the floor below. Belts without EC turn red; `v` shows the area it reaches.- Trade posts sell what they receive: money, a 40/40 split, or research (press `f` on one).
- **Waste** leaves a machine by its **back** edge, never the front: put a belt (facing away), crate or trade post behind the coke oven or blast furnace, so a row of machines can share a waste alley behind it. A belt facing into the machine doesn't take it. If nothing behind can take waste, the machine fills up on it, stops, and flashes `BACK` (its products leave the front as usual until then). Waste is never fuel: boilers, furnaces and kilns all refuse it. Waste sells for -$0.50 each and never earns research. Only a trade post set to money takes it (and pays the fee); research and split posts refuse it, so it stays on the belt and backs up (the post flashes `WSTE`). A post also refuses waste when the fee would take your money below zero, or when the last minute's waste fees are more than the last minute's earnings. Removing any part that holds waste (a crate, warehouse, elevator, belt, tunnel, splitter, arm, drill or machine) costs the same $0.50 each (refused if you can't afford it); hover it to see the cost. Removing an area pays each fee once, and asks first when the total is over $20. The production site (`o`) shows waste fees as negative money.
- Research per item fades the more you sell of it, down to a small per-item minimum, so new items are where research comes from. Research earned while the game is closed fades the same way.
- Nothing breaks or jams at random. Removing a part refunds it in full.

## Controls

| Action | Keys |
| --- | --- |
| Move | arrows / WASD |
| Lay belts | Shift+arrows / Shift+WASD, or drag with the mouse |
| Build menu | 1–4 opens a category (1 Create, 2 Transport, 3 EC, 4 Misc); inside it, a number picks a part (Create has 1–5, Transport 1–5), and pressing the same number again switches kind (the drills, belt / fast belt / tunnel, the assemblers, pump / advanced pump / mixer...); `` ` ``, `₹`, `~` or Esc goes back |
| Place / remove | Space or click / x or right-click |
| Area tool | `c` starts a rectangle: press Space (or Enter) at one corner, move, and Space again at the opposite corner, or drag with the left button. The rectangle is tinted and the parts in it light up. Then `x` removes everything in it, with a full refund, `c` copies it (see Blueprints), or `u` upgrades it (see Upgrade planner). Esc or a right-click cancels; a digit or a floor change closes the tool too. Waste held in any part costs the usual $0.50 each, once for the whole area, and over $20 you press `x` a second time to pay. Elevators go with all their doors and their car, and a long machine with one tile in the rectangle comes whole. `z` puts back what the last area remove took (settings and contents too, at the full price again), or swaps back what the last upgrade changed, whichever came last |
| Upgrade planner | Needs the **Upgrade planner** tech (Swiftline Conveyors, in the Tools group: 3,000 RP, or free once you have made 60 gearboxes); until then `u` says what unlocks it and changes nothing. With a rectangle chosen, `u` swaps each part that has a next version for it, **in place**: the same spot, facing and settings, and you pay only the difference in price (money, nothing else). It is a swap for another machine, never a change to the one you have. The pairs: basic belt → fast belt, cheap drill → basic drill, cheap solar → basic solar, advanced assembler → precision assembler, large basic assembler → large advanced assembler → large precision assembler (one step a press, and only with a recipe both make: engine blocks and pump units stay with the large basic and advanced), large basic drill → large advanced drill (a coal drill becomes an EC drill, so give it power). Each pair has the same footprint, so a layout never moves. Parts in the rectangle that can be upgraded are tinted green once it is chosen, and the sidebar counts them. The first `u` shows a one-line preview ("u again to upgrade 12 parts for $360"); the second does it. What a part holds comes along where the new one can hold it: belts keep every item, a drill its ore and output (coal it can no longer burn is dropped), an assembler its stock, output and recipe (matched **by name**; its progress starts over). An assembler whose recipe the new version cannot make (a precision assembler makes none of the crude or basic ones) is left as it is and counted. It goes in reading order and stops at the first part you cannot pay for, like a paste: the message counts what was upgraded, what is short of money (and by how much), and what is locked (a target you have not researched); press `u` twice again after earning more. `z` swaps the last upgrade back and pays the difference back (parts removed or changed since are left alone). Not included: arm → long arm (reach changes what it grabs), pump → advanced pump and crate → warehouse (the footprint grows) |
| Blueprints | Needs the **Blueprints** tech (Brannock & Daughters, in the Tools group: 800 RP, or free once you have made 1,500 copper plate); until then `c` on a chosen rectangle says so. With a rectangle chosen, `c`, Space or Enter copies it to your hand (a middle-drag does it in one go). Everything the rectangle touches comes, long machines whole, with its facing and settings: arm and splitter filters, trade post modes, a drill's ore, an elevator's size and door, tunnel entrances and exits, and recipes **by name**. Items, fuel and progress are not copied: a blueprint is a layout. A ghost follows the cursor with its top-left corner there: yellow sprites will be placed, `xx` on red can't be (something in the way, locked, off the map, or the wrong floor for it), `..` on orange means the money won't cover it; parts already standing there aren't drawn over. `r` turns it a quarter clockwise, Shift+`r` back; Space or a click pastes, Esc empties the hand. A paste needs the money for **all** of it (every part's price plus any digging or decking): if you are short, nothing is placed and nothing is charged, and the message says how much more you need. Otherwise every part that fits is placed in reading order (top row first, left to right); parts already there cost nothing, so pasting again at the same spot charges nothing twice. Dragging never repeats a paste. Tunnel pairs stay pairs when turned; a recipe you haven't researched is left at the machine's first one (the message counts them); an elevator needs room on every open floor and gets default (unload) doors on the floors other than the one it was copied from. The blueprint stays in your hand across floors, so you can copy on the ground and paste in the basement |
| Blueprint library | `l` (needs Blueprints): the saved blueprints over the map (the game keeps running). Up/Down pick, Enter (or a second click) takes one into your hand, `f` saves the blueprint in your hand (or else the last one you copied) under a name you type, `x` or Delete twice removes the picked file, Esc or `l` closes. Blueprints are plain JSON files in `~/.terminactory/blueprints`, one per blueprint, shared by every save; each has its own `format` number (currently 1), so saves are untouched. A file from a newer game, or one that can't be read, is listed as such with the reason and is never changed or deleted. A blueprint that uses a part you haven't unlocked yet shows `(Kiln locked)` and still pastes the rest |
| Copy a part | middle-click it: picks up its kind, facing and setting (recipe, arm filter, trade post mode), so clicks place copies; middle-click empty ground to empty your hand (a middle-drag copies a rectangle instead, see Blueprints) |
| Rotate (belt direction / output side) | r |
| Set up (arm filter, trade post mode) | f |
| Change floor | < > or scroll |
| Research | t: opens the research site in the browser (arrows pick, Enter researches; click a row, click again to research) |
| Save | Ctrl+S (it also saves every 30 seconds and on quit) |
| Production | o: opens the production site in the browser (the last minute per item, and the History tab); the game keeps running |
| Settings | g: the sidebar earnings graph (Off / Money / Research / Both) and its range (Seconds / Minutes / Hours), the small-pane view (Auto / Always / Never) and the width Auto switches at (80 / 100 / 120 / 140 / 160 columns), the telegram style (Sidebar / Note / Ticker) and Hints (On / Off); arrows or j/k pick a row, Left/Right or Enter change it, Esc closes; the game keeps running. Saved in `~/.terminactory/settings.json`; a file from an older version, or with fields missing, still loads |
| Show EC range | v |
| Elevator size | + / - |
| Pause / speed | p / [ ] |
| Browser (opens on what's under the cursor) | b |
| Mirror what is in your hand, a blueprint or a part (Shift: top to bottom); with nothing in hand, the part under the cursor | m |
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
| `==` `\|\|` `++` | Pipe (blue water, white steam, orange copper, red iron, pale steel, light blue silicon, grey empty) | 1×1 | `=` (pale) iron plate |
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
| `ARC` `GROW` `SW` | Arc furnace (3×3), crystal grower (2×4), wafer saw (2×1) | | `y` silicon crystal `W` wafer |
| `ETCH` `FETCH` | Etcher (3×3), fine etcher (3×5) | | `h` etchant `m` mask `k` die `H` microprocessor `R` RAM chip `n` fine mask `j` fine die `U` microprocessor x4x2 |
| `BASM` `ADVASM` `PRCASM` | Basic (2×2), advanced (3×3), precision (3×3) assembler | | `V` valve `Q` gauge `L` lamp `r` relay `M` motor `P` piston `B` control board |
| `LRG ASM B` `LRG ASM A` `LRG ASM P` | Large basic, advanced and precision assembler | 5×5 | `D` boiler shell `S` silo shell `F` frame `U` pump unit `E` generator `N` engine block `T` control panel `Y` steam tractor `l` keyboard `I` disk drive `w` mainboard `&` computer |
| `ADV DRL` `LRG … DRILL` `APMP` | Advanced drill (4×4), large drills (8×8), advanced pump | | |
