#!/usr/bin/env bash
# herdr plugin pane entrypoint: if vibegate is installed, run its TUI;
# otherwise show an install menu (↑/↓ + Enter). Declining exits — the pane
# closes with the command (plugin pane lifecycle).
set -u
# Pure-bash dirname: locate the script's own directory (builtin-only).
case "$0" in
  */*) DIR="${0%/*}" ;;
  *) DIR="$PWD" ;;
esac

if command -v vibegate >/dev/null 2>&1; then
  exec vibegate tui
fi

# ── options: [local install] [github install] [cancel] ──
OPTS=()
CMDS=()
if [ -f "$DIR/install.sh" ]; then
  OPTS+=("Install — local checkout (cargo build)")
  CMDS+=("local")
fi
OPTS+=("Install — from GitHub (prebuilt)")
CMDS+=("github")
OPTS+=("Cancel")
CMDS+=("cancel")
N=${#OPTS[@]}

sel=0
LINES=$((N + 1)) # header + options

draw() {
  [ "${1:-}" = "redraw" ] && printf "\e[%dA\r\e[J" "$LINES"
  echo "vibegate is not installed — choose one:"
  local i
  for ((i = 0; i < N; i++)); do
    if [ "$i" -eq "$sel" ]; then
      printf "  \e[1;36m❯ %d. %s\e[0m\r\n" $((i + 1)) "${OPTS[i]}"
    else
      printf "   %d. %s\r\n" $((i + 1)) "${OPTS[i]}"
    fi
  done
}

read_key() {
  local c
  if ! IFS= read -rsn1 c; then echo eof; return; fi
  case "$c" in
    $'\x1b')
      IFS= read -rsn2 -t 0.05 c
      case "$c" in
        '[A') echo up ;;
        '[B') echo down ;;
        *) echo esc ;;
      esac
      ;;
    "" | $'\r' | $'\n') echo enter ;;
    [1-9]) echo "num:$c" ;;
    [qQ]) echo quit ;;
    *) echo other ;;
  esac
}

run_selected() {
  echo
  case "${CMDS[sel]}" in
    local)
      echo "── installing (local checkout) ──"
      VIBEGATE_NONINTERACTIVE=1 VIBEGATE_FALLBACK_BUILD=1 bash "$DIR/install.sh" || exit 1
      ;;
    github)
      echo "── installing (GitHub) ──"
      curl -fsSL https://raw.githubusercontent.com/second-state/vibegate/main/install.sh | bash || exit 1
      ;;
    *)
      exit 0 # Cancel: the pane closes with the process
      ;;
  esac
  command -v vibegate >/dev/null 2>&1 || {
    echo "vibegate still not found after install — check your PATH (see install output)"
    exit 1
  }
  echo "── install complete, starting vibegate tui ──"
  sleep 1
  exec vibegate tui
}

draw
# Drain keystrokes still queued from the launch line so a stale Enter
# can't trigger the first menu item.
while read -rsn1 -t 0.25 _; do :; done
while key=$(read_key); do
  case "$key" in
    up) sel=$(((sel + N - 1) % N)); draw redraw ;;
    down) sel=$(((sel + 1) % N)); draw redraw ;;
    enter) run_selected; break ;;
    num:*)
      n=${key#num:}
      if [ "$n" -ge 1 ] && [ "$n" -le "$N" ]; then sel=$((n - 1)); draw redraw; fi
      ;;
    quit | esc)
      sel=$((N - 1))
      run_selected
      break
      ;;
    eof) exit 0 ;;
  esac
done
