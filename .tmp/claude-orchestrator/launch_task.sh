#!/bin/zsh
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: launch_task.sh <session_name> <settings_json> <prompt_file> <log_dir>" >&2
  exit 2
fi

session_name="$1"
settings_json="$2"
prompt_file="$3"
log_dir="$4"

mkdir -p "$log_dir"
prompt="$(cat "$prompt_file")"

exec /opt/homebrew/bin/claude -p -n "$session_name" \
  --setting-sources project,local \
  --settings "$settings_json" \
  --disallowedTools Agent \
  --output-format text \
  --permission-mode bypassPermissions \
  --dangerously-skip-permissions \
  --debug-file "$log_dir/debug.txt" \
  "$prompt" \
  > "$log_dir/stdout.txt" \
  2> "$log_dir/stderr.txt"
