# Claude Code Controller

This repo now includes a small controller for the Claude Code CLI with
provider rotation:

- script: `/Users/fwfx1234/develop/qingqi/scripts/claude_orchestrator.py`
- queue: `/Users/fwfx1234/develop/qingqi/docs/claude-task-queue.json`
- prompts: `/Users/fwfx1234/develop/qingqi/docs/claude-tasks/`
- runtime state: `/Users/fwfx1234/develop/qingqi/.tmp/claude-orchestrator/state.json`
- logs: `/Users/fwfx1234/develop/qingqi/.tmp/claude-orchestrator/logs/`

## Why this exists

The current Claude Code setup can run through Xiaomi plus extra providers from
`cc-switch`, and any one of them may rate-limit or reject a request.

The controller avoids hammering the active provider:

1. It starts with a cheap `OK` probe.
2. If the channel is still rate-limited, it backs off.
3. It runs real implementation tasks only after a probe succeeds.
4. It starts at single-instance concurrency.
5. After a configurable success streak, it can raise concurrency for tasks marked `parallelSafe`.
6. Any `429` drops concurrency back to `1`.
7. When a provider is limited, it advances to the next configured provider slot.

## Recommended usage

Export the remaining Xiaomi token before running:

```bash
export XIAOMI_AUTH_TOKENS="tp-cljh0egx9iq9rqkmm2w660qqy7vz3kwyd4fj1yoxtowbq4qe"
```

The controller can rotate across multiple Claude-compatible providers.

Default provider order:

```text
xiaomi -> anyrouter -> deepseek
```

Override the order if needed:

```bash
export CLAUDE_PROVIDER_ORDER="xiaomi,anyrouter,deepseek"
```

The Xiaomi provider uses `XIAOMI_AUTH_TOKENS` plus the base URL and model fields
from `~/.claude/settings.json`. The `anyrouter` and `deepseek` providers are
loaded from `~/.cc-switch/cc-switch.db`.

The controller now launches Claude with explicit per-provider
`--setting-sources project,local --settings <json>` overrides, so provider
switches do not fall back to the global Xiaomi user settings by accident.

By default, the controller does not pass `enabledPlugins` from the provider
settings, which keeps fragile Claude-side plugins from interfering with task
execution. Re-enable them only when needed:

```bash
export CLAUDE_INCLUDE_ENABLED_PLUGINS=1
```

Probe the current provider once:

```bash
python3 scripts/claude_orchestrator.py run --probe-only --once
```

Run the queue continuously with single-instance startup:

```bash
python3 scripts/claude_orchestrator.py run --max-concurrency 3
```

Print controller state:

```bash
python3 scripts/claude_orchestrator.py status
```

## Current queue

The initial queue is intentionally conservative:

1. `system-settings-b1`
2. `ftp-sftp-ssh-register`
3. `http-capture-foundation`

All three are marked non-parallel for now because they edit the same worktree and could conflict if Xiaomi suddenly recovers and multiple Claude instances start writing at once.
