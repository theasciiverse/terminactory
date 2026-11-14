# Terminactory

A cozy factory game for your terminal. Dig copper, coal and iron, melt it, press it, sell it, and research your way up to computers and oil. There is nothing to fight and nothing breaks at random: you build at your own pace, and the factory keeps earning while you are away.

It is made to sit in a split next to your AI agents. They can send you short messages that show up in the game, so you can play while they work.

```
tfac --demo     # a small working factory to look at
```

## Install

You need [Rust](https://rustup.rs) 1.88 or newer and a UTF-8 terminal. macOS and Linux are supported; on Windows, use [WSL](https://learn.microsoft.com/windows/wsl/install).

```bash
git clone https://gitlab.com/snipedbywifi/terminactory
cd terminactory
cargo install --path .
```

This installs two commands, `terminactory` and the shortcut `tfac`. Make sure `~/.cargo/bin` is on your `PATH` (rustup normally does this), then check it:

```bash
tfac --version
tfac --demo
```

To update, pull and run `cargo install --path . --force`. To remove it, `cargo uninstall terminactory`; your saves in `~/.terminactory` stay.

## Playing

`tfac` continues the save you played last.

```bash
tfac --saves    # list your saves
tfac 2          # open save 2
tfac --new      # start a fresh save; the others are kept
tfac --help     # everything tfac can do
```

The game looks best in a terminal of at least 120×40. In a narrower or shorter pane, such as a split beside an agent, it switches to a compact view by itself. Mouse works in most terminals (iTerm2, Terminal.app, WezTerm, Kitty). `?` shows every key in the game.

### The first hour

1. Press `1` for the Create menu and place a **drill** on the ground, a **furnace** in front of it, and a **molder** after that. Drills mine anywhere; `f` on a drill picks the ore.
2. Lay **belts** (hold Shift with the arrow keys, or drag) to a **trade post** in the Misc menu. Items sell for money, research, or a split.
3. Open research with `t` and buy the press. From there it is a tree: new machines open as you research them, or free once you have made enough of an item.
4. Make a company's **key item** and it sends you a letter and opens its machines.

Machines need **EC** (electric charge): solar panels and EC poles first, steam engines later. Idle machines flash a short word saying why (`FULL`, `NEED`, `EC?`, ...).

### Keys you will use

| Key | Does |
| --- | --- |
| arrows / WASD | move the cursor |
| `1`–`4`, then a number | pick something to build (Create, Transport, EC, Misc) |
| Space or click / `x` or right-click | place / remove |
| `r` / `f` | rotate / set up a machine (recipe, filter, mode) |
| `t` | research |
| `o` | production and earnings history |
| `b` | the in-game browser, on whatever is under the cursor |
| `c` | area tool: copy, remove or upgrade a rectangle |
| `m` | mirror what is in your hand, such as a copied blueprint (Shift: top to bottom) |
| `g` | settings |
| `p`, `[` `]` | pause, speed |
| `?` | help |
| Ctrl+C | save and quit |

Keys can be changed in `~/.terminactory/controls.json`.

### The browser

`b` opens **termweb**, the game's own browser. It has a search page, tabs, bookmarks, history and an address bar. It holds the game's wiki (every item and machine, with recipes, prices and where it sits in the research tree) and a small web of other sites you can look through as you play. Research (`t`) and production (`o`) open in it too.

Inside the browser, `b` opens a new tab, `Esc` puts it away with your tabs kept, and `1`–`9` switch tabs. "Copy page" in the bottom bar puts a page's text on your clipboard.

## Agent messages

Your AI agents can message you inside the game. A message costs the agent only the words in it, and nothing prints on success. Messages wait while the game is closed.

### Claude Code

Add this to `~/.claude/settings.json`. When an agent finishes, its last reply becomes a message; when it needs permission or input, you get a "needs you" message.

```json
{
  "hooks": {
    "Stop": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "tfac hook" }] }]
  }
}
```

Test it with the game open:

```bash
printf '%s' '{"hook_event_name":"Stop","cwd":"/tmp/demo","last_assistant_message":"## It works\n- a message from the hook"}' | tfac hook
```

### Any other agent

Anything that can run a shell command can send one:

```bash
tfac msg --from my-agent "## Tests fixed\n- 3 flaky tests\n- [ ] needs review"
echo "## Done" | tfac msg        # or read Markdown from stdin
```

`\n` is a line break and `--from` says who it is from (the default is the current folder's name). Markdown is rendered: headings, lists, checkboxes, quotes, code and tables. You can also drop a `*.md` file into `~/.terminactory/inbox/`.

Either tell the agent to run `tfac msg` when it finishes (put a line in `AGENTS.md` or its system prompt), or point the agent's own notify hook at it, which costs no tokens. Only Claude Code's hook has been tested so far.

If nothing shows up, check that `tfac --version` works in the shell the agent uses and that its `PATH` includes `~/.cargo/bin`. Messages appear at the bottom of the sidebar; in the compact view, a note in the corner. `n` reads the next one and `i` opens a log of all of them. Quieter styles are in the settings (`g`).

## Saves and settings

Everything lives in `~/.terminactory`: saves in `saves/`, `controls.json`, `settings.json` and your blueprints. The game saves every 30 seconds, when you quit or close the terminal, and with Ctrl+S. A save can be open in one window at a time. Saves carry a format version: a save from an older version of the game is never opened or deleted, and `tfac` tells you. See [CHANGELOG.md](CHANGELOG.md) for what changed between versions.

## More

- [docs/REFERENCE.md](docs/REFERENCE.md): every machine, recipe, price and key, written as the game grew.
- The in-game wiki (`b`) is the page to trust for current numbers.

## Development

```bash
cargo run --release     # same as tfac, from the source folder
cargo test              # simulation tests and screen snapshots
cargo clippy --all-targets
cargo fmt
```

- `src/sim.rs`: the simulation. 60 ticks per second, no randomness, no drawing.
- `src/content.rs`: items, companies, research and recipes, kept apart from mechanics.
- `src/ui.rs`, `src/app.rs`: terminal drawing, and keyboard and mouse.
- `src/web.rs`, `src/search.rs`, `src/wiki.rs`: the in-game browser's pages and search.
- `src/mail.rs`, `src/hints.rs`: agent messages, and the quiet hints on the status line.
- `wiki/`, `forums/`, `sites/`, `intranets/`: Markdown pages for the in-game browser, embedded at build time (`build.rs`).

The simulation is deterministic and separate from the drawing code, so most changes can be tested without a screen.

## License

Apache-2.0. See [LICENSE](LICENSE).
