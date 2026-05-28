#!/bin/zsh
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: dispatch_task.sh <task_key> <prompt_file> <log_base> <workdir>" >&2
  exit 2
fi

task_key="$1"
prompt_file="$2"
log_base="$3"
workdir="$4"

launcher="$workdir/.tmp/claude-orchestrator/launch_task.sh"
dispatch_log="${log_base}-dispatch.log"
poll_seconds="${DISPATCH_POLL_SECONDS:-5}"

if [[ ! -x "$launcher" ]]; then
  echo "missing launcher: $launcher" >&2
  exit 2
fi

mkdir -p "$(dirname "$log_base")"

log_line() {
  local message="$1"
  local now
  now="$(date '+%Y-%m-%dT%H:%M:%S%z')"
  print -r -- "$now $message" | tee -a "$dispatch_log" >&2
}

settings_for_provider() {
  local provider="$1"
  case "$provider" in
    xiaomi)
      print -r -- "/Users/fwfx1234/.claude/settings.json"
      ;;
    anyrouter)
      print -r -- "/tmp/anyrouter-claude-settings.json"
      ;;
    deepseek)
      print -r -- "/tmp/deepseek-claude-settings.json"
      ;;
    *)
      return 1
      ;;
  esac
}

should_fallback() {
  local provider_log_dir="$1"
  local debug_file="$provider_log_dir/debug.txt"

  if [[ ! -f "$debug_file" ]]; then
    return 1
  fi

  if /opt/homebrew/bin/rg -q 'Too many requests|API error .* 429 |"code":"429"' "$debug_file"; then
    print -r -- "rate-limit-429"
    return 0
  fi

  if /opt/homebrew/bin/rg -q 'Insufficient Balance|API error .* 402 |"code":"402"' "$debug_file"; then
    print -r -- "provider-balance-402"
    return 0
  fi

  return 1
}

run_provider() {
  local provider="$1"
  local settings_json="$2"
  local provider_log_dir="${log_base}-${provider}"
  local session_name="${task_key}-${provider}"
  local fallback_reason=""
  local exit_code=0

  mkdir -p "$provider_log_dir"
  log_line "starting provider=${provider} session=${session_name} logs=${provider_log_dir}"

  (
    cd "$workdir"
    "$launcher" "$session_name" "$settings_json" "$prompt_file" "$provider_log_dir"
  ) &
  local child_pid=$!

  while true; do
    if fallback_reason="$(should_fallback "$provider_log_dir")"; then
      log_line "provider=${provider} fallback=${fallback_reason} pid=${child_pid}"
      kill "$child_pid" 2>/dev/null || true
      wait "$child_pid" 2>/dev/null || true
      return 90
    fi

    if ! kill -0 "$child_pid" 2>/dev/null; then
      if wait "$child_pid"; then
        log_line "provider=${provider} success logs=${provider_log_dir}"
        return 0
      else
        exit_code=$?
        log_line "provider=${provider} failed exit=${exit_code} logs=${provider_log_dir}"
        return "$exit_code"
      fi
    fi

    sleep "$poll_seconds"
  done
}

providers=(xiaomi anyrouter deepseek)

for provider in "${providers[@]}"; do
  settings_json="$(settings_for_provider "$provider")"
  if [[ ! -f "$settings_json" ]]; then
    log_line "skipping provider=${provider} missing-settings=${settings_json}"
    continue
  fi

  if run_provider "$provider" "$settings_json"; then
    exit 0
  else
    status=$?
  fi

  if [[ "$status" -eq 90 ]]; then
    continue
  fi

  exit "$status"
done

log_line "all-providers-exhausted task=${task_key}"
exit 1
