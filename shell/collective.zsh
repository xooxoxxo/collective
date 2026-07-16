collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && print -z "$cmd"
}
