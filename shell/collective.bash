collective() {
  local last=""
  if [[ "$1" == "collect" && " ${*} " == *" --last "* ]]; then
    last="$(history 1 | sed 's/^ *[0-9]* *//')"
  fi
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" COLLECTIVE_LAST_CMD="$last" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && { READLINE_LINE="$cmd"; READLINE_POINT=${#cmd}; }
}
