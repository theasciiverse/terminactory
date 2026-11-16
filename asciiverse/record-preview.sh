#!/bin/sh
# Records the 100x30 catalog preview, asciiverse/terminactory.cast
# (hub/RECORDING.md): the demo factory at work with the sidebar on, sped
# up, then the production pane and the research tree, then termweb, the
# in-game browser: the Municipal Gazette, the crankshaft.forum boards and
# a wiki search, and quitting at the end. asciinema records a tmux pane
# (own socket, no status line, no user config) that the keys below are
# sent to. A throwaway HOME and XDG_STATE_HOME, so no real save is
# touched. Needs asciinema 3 and tmux.
#   asciiverse/record-preview.sh target/release/terminactory
set -eu
binary=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
here=$(cd "$(dirname "$0")" && pwd)
out=$here/terminactory.cast
home=$(mktemp -d)
trap 'tmux -L avrec kill-server 2>/dev/null; rm -rf "$home"' EXIT
mkdir -p "$home/bin" "$home/state/terminactory"
cp "$binary" "$home/bin/terminactory"
# The full view with its sidebar, which Auto keeps for 120 columns and up.
cat > "$home/state/terminactory/settings.json" <<'EOF'
{ "small_view": "never" }
EOF
cd "$home"  # nothing in the recording names a local path
export HOME="$home" XDG_STATE_HOME="$home/state" \
  PATH="$home/bin:$PATH" USER=player LOGNAME=player \
  TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8
tmux -L avrec -f /dev/null new-session -d -s rec -x 100 -y 30 \
  "terminactory --demo" \; set -g status off

keys() { tmux -L avrec send-keys -t rec "$@"; }
say() { keys -l "$1"; }
drive() {
  sleep 4
  keys ']' ; sleep 0.3; keys ']'; sleep 5      # faster
  keys Right Right Right Right; sleep 2           # the cursor along the line
  keys o; sleep 4                                 # production
  keys Escape; sleep 1
  keys t; sleep 2                                 # research
  keys Down; sleep 1.5; keys Down; sleep 2
  keys Escape; sleep 1
  keys b; sleep 1.5                               # termweb
  say gazette.city; sleep 1; keys Enter; sleep 5
  keys PageDown; sleep 3
  keys +; sleep 1
  say crankshaft.forum; sleep 1; keys Enter; sleep 5
  keys PageDown; sleep 3
  keys /; sleep 0.5
  say 'steel girder'; sleep 2; keys Enter; sleep 4
  keys Escape; sleep 3
  keys C-c
}
drive &
drv=$!
asciinema rec --headless --overwrite -q \
  --window-size 100x30 --idle-time-limit 2 \
  --capture-env "TERM,COLORTERM,LANG" --title "Terminactory" \
  --command "tmux -L avrec attach -t rec" "$out"
wait "$drv"
