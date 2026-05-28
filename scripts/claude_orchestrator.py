#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_BACKOFF_SECONDS = [30, 60, 120, 300, 600, 900]
DEFAULT_PROBE_PROMPT = "Reply with exactly OK"
RATE_LIMIT_PATTERN = re.compile(r"API error .*429 .*Too many requests", re.IGNORECASE)
DEFAULT_PROVIDER_ORDER = ["xiaomi", "anyrouter", "deepseek"]


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def ensure_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path: Path, payload: dict[str, Any]) -> None:
    ensure_dir(path.parent)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def truncate(text: str, limit: int = 4000) -> str:
    if len(text) <= limit:
        return text
    return text[: limit - 3] + "..."


@dataclass
class TaskSpec:
    task_id: str
    title: str
    prompt_file: Path
    verify_commands: list[str] = field(default_factory=list)
    parallel_safe: bool = False


@dataclass
class ProviderSlot:
    label: str
    source: str
    env: dict[str, str]
    settings: dict[str, Any] = field(default_factory=dict)


@dataclass
class TaskResult:
    ok: bool
    exit_code: int | None
    rate_limited: bool
    timed_out: bool
    stdout: str
    stderr: str
    debug_tail: str
    log_dir: Path


class ClaudeOrchestrator:
    def __init__(
        self,
        repo_root: Path,
        queue_path: Path,
        runtime_dir: Path,
        task_timeout_sec: int,
        probe_timeout_sec: int,
        max_concurrency: int,
        success_threshold: int,
    ) -> None:
        self.repo_root = repo_root
        self.queue_path = queue_path
        self.runtime_dir = runtime_dir
        self.task_timeout_sec = task_timeout_sec
        self.probe_timeout_sec = probe_timeout_sec
        self.max_concurrency = max(1, max_concurrency)
        self.success_threshold = max(1, success_threshold)
        self.queue_payload = read_json(queue_path)
        self.workdir = Path(self.queue_payload.get("workdir", str(repo_root))).resolve()
        self.backoff_seconds = list(
            self.queue_payload.get("backoffSeconds", DEFAULT_BACKOFF_SECONDS)
        ) or list(DEFAULT_BACKOFF_SECONDS)
        self.probe_prompt = str(
            self.queue_payload.get("probePrompt", DEFAULT_PROBE_PROMPT)
        )
        self.provider_slots = self._load_provider_slots()
        self.tasks = self._load_tasks()
        self.state_path = runtime_dir / "state.json"
        self.logs_dir = runtime_dir / "logs"
        ensure_dir(self.logs_dir)
        self.state = self._load_or_init_state()
        self.current_concurrency = int(self.state.get("currentConcurrency", 1))
        self.success_streak = int(self.state.get("successStreak", 0))
        self.rate_limit_events = int(self.state.get("rateLimitEvents", 0))
        self.provider_cursor = int(
            self.state.get("providerCursor", self.state.get("tokenCursor", 0))
        )

    def _load_tasks(self) -> list[TaskSpec]:
        tasks: list[TaskSpec] = []
        queue_items = self.queue_payload.get("queue", [])
        if not isinstance(queue_items, list) or not queue_items:
            raise ValueError(f"Queue file {self.queue_path} has no tasks")

        for item in queue_items:
            if not isinstance(item, dict):
                raise ValueError("Queue entry must be an object")
            task_id = str(item["id"])
            title = str(item["title"])
            prompt_file = (self.repo_root / str(item["promptFile"])).resolve()
            verify_commands = [str(cmd) for cmd in item.get("verifyCommands", [])]
            parallel_safe = bool(item.get("parallelSafe", False))
            tasks.append(
                TaskSpec(
                    task_id=task_id,
                    title=title,
                    prompt_file=prompt_file,
                    verify_commands=verify_commands,
                    parallel_safe=parallel_safe,
                )
            )
        return tasks

    def _provider_order(self) -> list[str]:
        raw = os.environ.get("CLAUDE_PROVIDER_ORDER", "")
        values = [item.strip().lower() for item in raw.split(",") if item.strip()]
        return values or list(DEFAULT_PROVIDER_ORDER)

    def _include_enabled_plugins(self) -> bool:
        return os.environ.get("CLAUDE_INCLUDE_ENABLED_PLUGINS", "").strip() == "1"

    def _load_provider_slots(self) -> list[ProviderSlot]:
        slots: list[ProviderSlot] = []
        for provider_name in self._provider_order():
            if provider_name == "xiaomi":
                slots.extend(self._load_xiaomi_slots())
                continue
            slots.append(self._load_cc_switch_provider(provider_name))

        if not slots:
            raise RuntimeError("No provider slots configured for the orchestrator.")
        return slots

    def _parse_token_list(self, raw: str) -> list[str]:
        tokens: list[str] = []
        seen: set[str] = set()
        for part in re.split(r"[\n,]", raw):
            token = part.strip()
            if not token or token in seen:
                continue
            seen.add(token)
            tokens.append(token)
        return tokens

    def _load_xiaomi_slots(self) -> list[ProviderSlot]:
        tokens = self._parse_token_list(os.environ.get("XIAOMI_AUTH_TOKENS", ""))
        settings_path = Path.home() / ".claude" / "settings.json"
        settings_payload: dict[str, Any] = {}
        if settings_path.exists():
            try:
                settings_payload = read_json(settings_path)
            except Exception:
                settings_payload = {}

        if not tokens:
            token = settings_payload.get("env", {}).get("ANTHROPIC_AUTH_TOKEN")
            if isinstance(token, str) and token:
                tokens = [token]

        if not tokens:
            raise RuntimeError(
                "No Xiaomi auth token available. Set XIAOMI_AUTH_TOKENS before running the orchestrator."
            )

        env_defaults: dict[str, str] = {}
        enabled_plugins: dict[str, bool] = {}
        env_map = settings_payload.get("env", {}) if isinstance(settings_payload, dict) else {}
        allowed = {
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_MODEL",
            "CLAUDE_CODE_EFFORT_LEVEL",
        }
        if isinstance(env_map, dict):
            for key in allowed:
                value = env_map.get(key)
                if isinstance(value, str) and value:
                    env_defaults[key] = value
        plugin_map = (
            settings_payload.get("enabledPlugins", {})
            if isinstance(settings_payload, dict)
            else {}
        )
        if isinstance(plugin_map, dict):
            enabled_plugins = {
                key: value
                for key, value in plugin_map.items()
                if isinstance(key, str) and isinstance(value, bool)
            }

        slots: list[ProviderSlot] = []
        for index, token in enumerate(tokens, start=1):
            env = dict(env_defaults)
            env["ANTHROPIC_AUTH_TOKEN"] = token
            settings = {"env": dict(env)}
            if enabled_plugins and self._include_enabled_plugins():
                settings["enabledPlugins"] = dict(enabled_plugins)
            label = "xiaomi" if len(tokens) == 1 else f"xiaomi-{index}"
            slots.append(
                ProviderSlot(
                    label=label,
                    source="xiaomi",
                    env=env,
                    settings=settings,
                )
            )
        return slots

    def _load_cc_switch_provider(self, provider_name: str) -> ProviderSlot:
        db_path = Path.home() / ".cc-switch" / "cc-switch.db"
        if not db_path.exists():
            raise RuntimeError(f"cc-switch database not found: {db_path}")

        conn = sqlite3.connect(str(db_path))
        try:
            row = conn.execute(
                """
                select name, settings_config
                from providers
                where app_type = 'claude' and lower(name) = lower(?)
                limit 1
                """,
                (provider_name,),
            ).fetchone()
        finally:
            conn.close()

        if row is None:
            raise RuntimeError(f"Provider '{provider_name}' not found in cc-switch.")

        settings = json.loads(row[1])
        env_map = settings.get("env", {})
        if not isinstance(env_map, dict) or not env_map:
            raise RuntimeError(f"Provider '{provider_name}' has no usable env config.")

        env = {
            key: value
            for key, value in env_map.items()
            if isinstance(key, str) and isinstance(value, str) and value
        }
        env.pop("ANTHROPIC_API_KEY", None)
        settings_payload = {"env": dict(env)}
        if self._include_enabled_plugins():
            enabled_plugins = settings.get("enabledPlugins")
            if isinstance(enabled_plugins, dict) and enabled_plugins:
                settings_payload["enabledPlugins"] = enabled_plugins
        return ProviderSlot(
            label=str(row[0]).strip().lower(),
            source="cc-switch",
            env=env,
            settings=settings_payload,
        )

    def _load_or_init_state(self) -> dict[str, Any]:
        if self.state_path.exists():
            return read_json(self.state_path)

        state = {
            "createdAt": utc_now(),
            "updatedAt": utc_now(),
            "queueFile": str(self.queue_path),
            "workdir": str(self.workdir),
            "currentConcurrency": 1,
            "successStreak": 0,
            "rateLimitEvents": 0,
            "tasks": {
                task.task_id: {
                    "title": task.title,
                    "status": "pending",
                    "attempts": 0,
                    "lastUpdatedAt": None,
                    "lastError": None,
                    "lastLogs": None,
                }
                for task in self.tasks
            },
        }
        write_json(self.state_path, state)
        return state

    def save_state(self) -> None:
        self.state["updatedAt"] = utc_now()
        self.state["currentConcurrency"] = self.current_concurrency
        self.state["successStreak"] = self.success_streak
        self.state["rateLimitEvents"] = self.rate_limit_events
        self.state["providerCursor"] = self.provider_cursor
        self.state.pop("tokenCursor", None)
        write_json(self.state_path, self.state)

    def _task_state(self, task: TaskSpec) -> dict[str, Any]:
        return self.state["tasks"][task.task_id]

    def task_status(self, task: TaskSpec) -> str:
        return str(self._task_state(task)["status"])

    def pending_tasks(self) -> list[TaskSpec]:
        return [task for task in self.tasks if self.task_status(task) == "pending"]

    def next_batch(self) -> list[TaskSpec]:
        pending = self.pending_tasks()
        if not pending:
            return []
        first = pending[0]
        if not first.parallel_safe or self.current_concurrency == 1:
            return [first]

        batch: list[TaskSpec] = []
        for task in pending:
            if not task.parallel_safe:
                break
            batch.append(task)
            if len(batch) >= self.current_concurrency:
                break
        return batch or [first]

    def build_claude_command(
        self,
        prompt: str,
        session_name: str,
        debug_file: Path,
        provider: ProviderSlot,
    ) -> list[str]:
        settings_json = json.dumps(provider.settings, ensure_ascii=False)
        return [
            "claude",
            "-p",
            "-n",
            session_name,
            "--setting-sources",
            "project,local",
            "--settings",
            settings_json,
            "--disallowedTools",
            "Agent",
            "--output-format",
            "text",
            "--permission-mode",
            "bypassPermissions",
            "--dangerously-skip-permissions",
            "--debug-file",
            str(debug_file),
            prompt,
        ]

    def _read_text(self, path: Path) -> str:
        if not path.exists():
            return ""
        return path.read_text(encoding="utf-8", errors="replace")

    def _run_command(
        self,
        cmd: list[str],
        timeout_sec: int,
        log_dir: Path,
        provider: ProviderSlot,
    ) -> TaskResult:
        ensure_dir(log_dir)
        stdout_path = log_dir / "stdout.txt"
        stderr_path = log_dir / "stderr.txt"
        debug_path = log_dir / "debug.txt"
        env = self._build_process_env(provider)
        with stdout_path.open("w", encoding="utf-8") as stdout_handle, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr_handle:
            timed_out = False
            exit_code: int | None = None
            try:
                completed = subprocess.run(
                    cmd,
                    cwd=self.workdir,
                    stdout=stdout_handle,
                    stderr=stderr_handle,
                    text=True,
                    env=env,
                    timeout=timeout_sec,
                    check=False,
                )
                exit_code = completed.returncode
            except subprocess.TimeoutExpired:
                timed_out = True
                exit_code = None

        stdout = self._read_text(stdout_path)
        stderr = self._read_text(stderr_path)
        debug_text = self._read_text(debug_path)
        has_api_429 = bool(RATE_LIMIT_PATTERN.search(debug_text))
        rate_limited = has_api_429 and (exit_code != 0 or not stdout.strip())
        ok = (exit_code == 0) and not timed_out and not rate_limited
        return TaskResult(
            ok=ok,
            exit_code=exit_code,
            rate_limited=rate_limited,
            timed_out=timed_out,
            stdout=stdout,
            stderr=stderr,
            debug_tail=truncate(debug_text[-4000:]),
            log_dir=log_dir,
        )

    def _build_process_env(self, provider: ProviderSlot) -> dict[str, str]:
        env = os.environ.copy()
        for key, value in provider.env.items():
            env[key] = value
        env.pop("ANTHROPIC_API_KEY", None)
        return env

    def current_provider(self) -> ProviderSlot:
        return self.provider_slots[self.provider_cursor % len(self.provider_slots)]

    def advance_provider(self) -> None:
        self.provider_cursor = (self.provider_cursor + 1) % len(self.provider_slots)

    def run_probe(self, attempt: int) -> TaskResult:
        provider = self.current_provider()
        slug = f"probe-{attempt:04d}-{provider.label}"
        log_dir = self.logs_dir / slug
        debug_file = log_dir / "debug.txt"
        cmd = self.build_claude_command(
            self.probe_prompt,
            session_name=f"qingqi-probe-{provider.label}",
            debug_file=debug_file,
            provider=provider,
        )
        result = self._run_command(cmd, self.probe_timeout_sec, log_dir, provider)
        if result.ok and result.stdout.strip() != "OK":
            result.ok = False
        return result

    def run_task(self, task: TaskSpec) -> TaskResult:
        provider = self.current_provider()
        prompt = task.prompt_file.read_text(encoding="utf-8")
        task_state = self._task_state(task)
        attempt = int(task_state["attempts"]) + 1
        task_state["attempts"] = attempt
        task_state["status"] = "running"
        task_state["lastUpdatedAt"] = utc_now()
        self.save_state()

        log_dir = self.logs_dir / f"{task.task_id}-{attempt:03d}-{provider.label}"
        debug_file = log_dir / "debug.txt"
        cmd = self.build_claude_command(
            prompt,
            f"{task.task_id}-{provider.label}",
            debug_file,
            provider=provider,
        )
        result = self._run_command(cmd, self.task_timeout_sec, log_dir, provider)
        return result

    def run_verification(self, task: TaskSpec, attempt: int) -> tuple[bool, str]:
        if not task.verify_commands:
            return True, "no local verification commands configured"

        verify_dir = self.logs_dir / f"{task.task_id}-{attempt:03d}" / "verify"
        ensure_dir(verify_dir)
        output_chunks: list[str] = []
        for index, command in enumerate(task.verify_commands, start=1):
            proc = subprocess.run(
                ["zsh", "-lc", command],
                cwd=self.workdir,
                capture_output=True,
                text=True,
                check=False,
            )
            log_path = verify_dir / f"{index:02d}.log"
            log_path.write_text(
                f"$ {command}\n\n[exit={proc.returncode}]\n\n{proc.stdout}\n{proc.stderr}",
                encoding="utf-8",
            )
            output_chunks.append(
                f"$ {command}\nexit={proc.returncode}\n{truncate(proc.stdout + proc.stderr, 1200)}"
            )
            if proc.returncode != 0:
                return False, "\n\n".join(output_chunks)
        return True, "\n\n".join(output_chunks)

    def mark_rate_limited(self, reason: str, logs: Path) -> None:
        self.current_concurrency = 1
        self.success_streak = 0
        self.rate_limit_events += 1
        self.state["lastRateLimitAt"] = utc_now()
        self.state["lastRateLimitReason"] = reason
        self.state["lastRateLimitLogs"] = str(logs)
        self.advance_provider()
        self.save_state()

    def backoff_sleep(self, attempt: int) -> int:
        index = min(max(1, attempt) - 1, len(self.backoff_seconds) - 1)
        seconds = int(self.backoff_seconds[index])
        print(
            f"[{utc_now()}] Claude provider still limited; sleeping {seconds}s before retry.",
            flush=True,
        )
        time.sleep(seconds)
        return seconds

    def maybe_raise_concurrency(self) -> None:
        if self.current_concurrency >= self.max_concurrency:
            return
        if self.success_streak < self.success_threshold:
            return
        self.current_concurrency += 1
        self.success_streak = 0
        self.save_state()
        print(
            f"[{utc_now()}] Success streak cleared. Raising concurrency to {self.current_concurrency}.",
            flush=True,
        )

    def print_status(self) -> None:
        print(f"Queue: {self.queue_path}")
        print(f"Workdir: {self.workdir}")
        print(f"State: {self.state_path}")
        print(f"Current concurrency: {self.current_concurrency}")
        print(f"Success streak: {self.success_streak}")
        print(f"Rate-limit events: {self.rate_limit_events}")
        print("Provider order: " + " -> ".join(slot.label for slot in self.provider_slots))
        print(
            "Current provider: "
            + self.provider_slots[self.provider_cursor % len(self.provider_slots)].label
        )
        for task in self.tasks:
            task_state = self._task_state(task)
            print(
                f"- {task.task_id}: {task_state['status']} "
                f"(attempts={task_state['attempts']}, logs={task_state['lastLogs']})"
            )

    def run_loop(self, once: bool, probe_only: bool, max_probe_attempts: int | None) -> int:
        probe_attempt = 0
        while True:
            if probe_only:
                if max_probe_attempts is not None and probe_attempt >= max_probe_attempts:
                    return 2
            elif not self.pending_tasks():
                print(f"[{utc_now()}] Queue complete.", flush=True)
                self.save_state()
                return 0

            probe_attempt += 1
            provider = self.current_provider()
            probe_result = self.run_probe(probe_attempt)
            if probe_result.rate_limited:
                self.mark_rate_limited(f"probe-429:{provider.label}", probe_result.log_dir)
                if once:
                    print(
                        f"[{utc_now()}] Probe hit 429 on {provider.label}. "
                        f"Logs: {probe_result.log_dir}",
                        flush=True,
                    )
                    return 3
                self.backoff_sleep(probe_attempt)
                continue

            if not probe_result.ok:
                print(
                    f"[{utc_now()}] Probe failed on {provider.label} without 429.\n"
                    f"stdout:\n{truncate(probe_result.stdout, 1200)}\n\n"
                    f"stderr:\n{truncate(probe_result.stderr, 1200)}",
                    flush=True,
                )
                return 4

            print(f"[{utc_now()}] Probe succeeded on {provider.label}.", flush=True)
            if probe_only:
                return 0

            batch = self.next_batch()
            for task in batch:
                task_state = self._task_state(task)
                attempt = int(task_state["attempts"]) + 1
                provider = self.current_provider()
                print(
                    f"[{utc_now()}] Running task {task.task_id} "
                    f"(attempt {attempt}, provider={provider.label}, concurrency={self.current_concurrency}).",
                    flush=True,
                )
                result = self.run_task(task)
                task_state = self._task_state(task)
                task_state["lastLogs"] = str(result.log_dir)
                task_state["lastUpdatedAt"] = utc_now()

                if result.rate_limited:
                    task_state["status"] = "pending"
                    task_state["lastError"] = "429 Too many requests"
                    self.mark_rate_limited(f"task-429:{provider.label}", result.log_dir)
                    self.save_state()
                    print(
                        f"[{utc_now()}] Task {task.task_id} hit 429 on {provider.label}. "
                        f"Re-queued. Logs: {result.log_dir}",
                        flush=True,
                    )
                    if once:
                        return 3
                    self.backoff_sleep(int(task_state["attempts"]))
                    break

                if result.timed_out:
                    task_state["status"] = "pending"
                    task_state["lastError"] = f"timed out after {self.task_timeout_sec}s"
                    self.success_streak = 0
                    self.save_state()
                    print(
                        f"[{utc_now()}] Task {task.task_id} timed out and was re-queued. Logs: {result.log_dir}",
                        flush=True,
                    )
                    if once:
                        return 5
                    continue

                if not result.ok:
                    task_state["status"] = "failed"
                    task_state["lastError"] = truncate(
                        (result.stderr or result.stdout or result.debug_tail), 2000
                    )
                    self.success_streak = 0
                    self.save_state()
                    print(
                        f"[{utc_now()}] Task {task.task_id} failed. Logs: {result.log_dir}",
                        flush=True,
                    )
                    return 6

                verify_ok, verify_output = self.run_verification(task, int(task_state["attempts"]))
                if not verify_ok:
                    task_state["status"] = "verify_failed"
                    task_state["lastError"] = truncate(verify_output, 2000)
                    self.success_streak = 0
                    self.save_state()
                    print(
                        f"[{utc_now()}] Local verification failed for {task.task_id}. "
                        f"Logs: {result.log_dir}",
                        flush=True,
                    )
                    return 7

                task_state["status"] = "done"
                task_state["lastError"] = None
                self.success_streak += 1
                self.save_state()
                print(
                    f"[{utc_now()}] Task {task.task_id} completed.\n"
                    f"Claude output:\n{truncate(result.stdout, 1600)}\n\n"
                    f"Local verification:\n{truncate(verify_output, 1600)}",
                    flush=True,
                )
                self.maybe_raise_concurrency()

            if once:
                return 0

    @staticmethod
    def validate_paths(queue_path: Path, repo_root: Path) -> None:
        payload = read_json(queue_path)
        for item in payload.get("queue", []):
            prompt_path = (repo_root / str(item["promptFile"])).resolve()
            if not prompt_path.exists():
                raise FileNotFoundError(prompt_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run Claude Code tasks with provider rotation, backoff, and local verification."
    )
    parser.add_argument(
        "command",
        choices=["run", "status"],
        help="run the orchestrator or print the current state",
    )
    parser.add_argument(
        "--queue",
        default="docs/claude-task-queue.json",
        help="Path to the queue JSON file, relative to the repo root",
    )
    parser.add_argument(
        "--runtime-dir",
        default=".tmp/claude-orchestrator",
        help="Runtime state and log directory, relative to the repo root",
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root that contains the queue and task prompt files",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Run a single probe/task cycle and exit",
    )
    parser.add_argument(
        "--probe-only",
        action="store_true",
        help="Only test channel availability, do not run queue tasks",
    )
    parser.add_argument(
        "--max-probe-attempts",
        type=int,
        default=None,
        help="Maximum probe attempts before exiting when --probe-only is set",
    )
    parser.add_argument(
        "--task-timeout-sec",
        type=int,
        default=45 * 60,
        help="Timeout for a real Claude implementation task",
    )
    parser.add_argument(
        "--probe-timeout-sec",
        type=int,
        default=180,
        help="Timeout for a cheap Claude provider probe",
    )
    parser.add_argument(
        "--max-concurrency",
        type=int,
        default=1,
        help="Upper bound for adaptive concurrency when tasks are marked parallelSafe",
    )
    parser.add_argument(
        "--success-threshold",
        type=int,
        default=2,
        help="Successful tasks required before raising concurrency by one",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(args.repo_root).resolve()
    queue_path = (repo_root / args.queue).resolve()
    runtime_dir = (repo_root / args.runtime_dir).resolve()
    ClaudeOrchestrator.validate_paths(queue_path, repo_root)
    orchestrator = ClaudeOrchestrator(
        repo_root=repo_root,
        queue_path=queue_path,
        runtime_dir=runtime_dir,
        task_timeout_sec=args.task_timeout_sec,
        probe_timeout_sec=args.probe_timeout_sec,
        max_concurrency=args.max_concurrency,
        success_threshold=args.success_threshold,
    )
    if args.command == "status":
        orchestrator.print_status()
        return 0
    return orchestrator.run_loop(args.once, args.probe_only, args.max_probe_attempts)


if __name__ == "__main__":
    sys.exit(main())
