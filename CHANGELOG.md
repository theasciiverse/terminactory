# Changelog

Each release notes whether saves from the previous one still open. Saves from another
version are refused, never deleted.

## 0.1.0 (unreleased)

First public version. Saves are format version 8 (version 7 saves still open and are converted as they are read).

### The game
- Phase 1: drills, furnaces, molders and presses; trade posts that sell for money, research or a split; a research tree with free unlocks for making enough of an item; companies that open when you make their key item and send a letter.
- Phase 2: pumps and pipes with one fluid per network, solar panels and EC poles, boilers and steam engines, the blast furnace and its waste, which leaves a machine by its back edge.
- Phase 3: floors and elevators, assemblers from crude to precision (and large ones), kilns, mixers, welders, belt tunnels, long arms, filter splitters, overflow gates and warehouses.
- Layout presets in the settings (`g`): Dense (the full screen, the default), Calm (compact view, no sidebar graph, no hints, telegrams as a short note) and Custom, which you get by changing any of those settings yourself.
- Item glyphs can be Nerd Font icons (settings, Icons), and `glyphs.json` in `~/.terminactory` replaces any item's glyph with a character of your choice. Machines keep their ASCII pictures.
- Silicon, chips and computers: wafers, etchers, microprocessors, and the computer as the branch's crown item.
- Oil: crude oil, naphtha, fuel oil and ethylene, the oil extractor, the still and the liquid trader, and a seventh Create slot for chemistry.
- Tools: the area tool (`c`) to remove, copy or upgrade a rectangle, blueprints saved to `~/.terminactory/blueprints` that can be turned (`r`) and mirrored (`m`), and the upgrade planner (`u`).
- Offline earnings: the factory earns while the game is closed, with no cap.
- An unbounded map: build in any direction, as far as you like. A floor stores only what is on it, so saves stay small and the simulation only visits buildings.

### The interface
- **termweb**, the in-game browser (`b`): a search start page, tabs, bookmarks, search history, an address bar, a wiki with a page for every item and machine (recipes, prices, research path, a chart of how research fades with sales), and a small web of other sites. Research (`t`) and production (`o`) open as sites in it. Pages can be copied to the clipboard.
- Production and earnings history (`o`) with charts by second, minute, hour and day, and a compact earnings graph in the sidebar.
- Settings (`g`), key remapping in `controls.json`, quiet hints on the status line, and a compact view for narrow splits.

### Agent messages
- Claude Code hook (`tfac hook`) and `tfac msg` for any other agent, Markdown rendered in the game, a log with unread marks (`i`), and company letters that arrive as messages and never pause the game.

### Under the hood
- The simulation is deterministic (60 ticks a second, no randomness) and separate from drawing.
- Saves name items, companies, techs and recipes instead of counting positions, so new content does not break them.
- Apache-2.0 license and package metadata.
