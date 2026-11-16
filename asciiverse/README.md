# Terminactory

**The ground is yours, so dig anywhere. No rush.**

So says the first telegram, from Hollowmere Mining Co. and the three
Brannock sisters. Put a drill down anywhere and tell it what to dig,
feed it coal, and belt the copper ore through a furnace, a molder and a
press into a trade post. Then make it bigger: steel from blast furnaces,
steam engines, floors raised on girders and floors dug beneath you,
assemblers from crude to precision, silicon wafers, microprocessors,
computers, and oil.

Terminactory is snipedbywifi's cosy idle factory game. Nothing attacks,
nothing breaks at random, and the factory keeps earning while the game
is closed. A machine that stops says why in one word (FULL, NEED, EC?).
Every new thing you make opens a company that writes to you, fifteen in
all, from Ashgrove Ironstone (*"Copper wire, is it? Fine hands."*) to
Meridian Instruments, who have been waiting for you, and brought snacks.

Press **b** for termweb, the in-game browser: a wiki with a page for
every item and machine, and a small internet of a town, its Municipal
Gazette, its company sites, five forums of plant workers swapping build
orders and old stories, and the plants' locked intranets, whose
passwords are hidden in what people say to one another.

## Playing

Run `terminactory` (or its short name, `tfac`). Any terminal of 60x20
or more will do; below 120 columns the game switches to a compact view
without the sidebar, and **g** (settings) can change that.

- **Arrows** or **WASD**: move; **Shift+arrows**: lay belts as you go
- **1** to **4**: build menu, then a number picks the part; **Space**
  or a click: place
- **r**: rotate; **f**: set it up (ore, recipe, money or research);
  **x** or right-click: remove
- **t** research, **o** production, **b** browser, **g** settings
- **c** area tool, **u** upgrade, **l** blueprint library, **m** mirror,
  **z** undo a removal
- **<** **>** floors, **v** power range, **n** next telegram, **i**
  telegram log
- **p** pause, **[** **]** speed, **Ctrl+S** save, **?** every key,
  **Ctrl+C** save and quit

Options: `--demo` starts beside a small factory that already works,
`--new` starts a fresh save, `--saves` lists them, `terminactory 2`
opens save 2, `--help` lists it all and names the save folder.

Saves, blueprints, `controls.json` (your key bindings) and
`settings.json` live in `$XDG_STATE_HOME/terminactory`, else
`~/.local/state/terminactory` on Linux, else `~/.terminactory`. A
`~/.terminactory` from an earlier install is moved there the first time.

## Credits

- Terminactory: snipedbywifi,
  https://gitlab.com/snipedbywifi/terminactory (Apache-2.0, see LICENSE)
- Built with Ratatui
- This package: Asciiverse, for the av launcher and the catalog at
  https://asciiverse.sh. What we changed is in PATCHES.md, and what
  changed in the game is in CHANGELOG.md.

## Building

`cargo build --release --no-default-features` with Rust 1.88 or newer
builds the game as we ship it; the release workflow
(`.github/workflows/asciiverse-release.yml`) builds it for Linux (static
musl), macOS and Windows. A plain `cargo build` keeps upstream's
`agent-mail` feature, the `tfac msg` and `tfac hook` commands described
in the upstream README in this repository.
