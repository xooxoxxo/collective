collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && { READLINE_LINE="$cmd"; READLINE_POINT=${#cmd}; }
}
