collective() {
  local last=""
  if [[ "$1" == "collect" && " ${*} " == *" --last "* ]]; then
    last="$(fc -ln -1)"
    last="${last#"${last%%[![:space:]]*}"}"
  fi
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" COLLECTIVE_LAST_CMD="$last" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && print -z "$cmd"
}
