# Local assistant

HarborVM runs the installed **Codex CLI** for each assistant turn. It does not create API keys, read credential files, or implement its own model service. Sign in using Codex's normal terminal flow before using the panel:

```sh
codex login status
# If needed:
codex login
```

The adapter finds `codex` on `PATH`, then tries `~/.local/bin/codex`. A profile's optional `assistant_program` field can select another absolute executable path or a program name on `PATH`. This executable must support the Codex `exec --json` protocol; it is not an arbitrary shell command. Relative paths and command strings containing arguments are not supported. Choosing an executable authorizes that local program to run with your host account's permissions.

Each turn uses these arguments, with the private work directory supplied as a separate argument:

```text
exec --ignore-user-config --ephemeral --sandbox read-only
     --skip-git-repo-check --json --color never -C <assistant-work> -
```

The prompt goes to standard input, never through a shell or a process argument. Quotes, backticks, dollar signs, and other shell metacharacters remain text. The process starts in a mode-0700 `assistant-work` directory within the selected profile. A lock prevents concurrent turns for that profile. Global user configuration is ignored to avoid importing unrelated instructions, hooks, or integrations. Existing Codex authentication remains managed by Codex. Use a current Codex release that supports the listed flags; unsupported versions fail visibly.

## Context and boundaries

HarborVM sends the profile ID, profile name, and the user's current prompt. It does not automatically send guest files, guest command output, clipboard contents, VM disks, or previous chat turns. Each turn is ephemeral and independent. Relevant context can be pasted explicitly into the prompt.

The assistant's response is advisory text. HarborVM never interprets it as a guest command, starts a VM from it, or executes generated commands. The guest command console remains a separate, explicit action.

Codex runs on the **host**, using Codex's read-only sandbox. This is not a guest-isolation boundary or a guarantee that host files cannot be read: read-only restricts changes, and the local CLI's own sandbox rules still apply. Only run an assistant executable you trust. Model requests use the account and service configured in Codex and may consume that account's normal usage allowance.

## Output, failures, and cancellation

The adapter parses JSON Lines and flushes each completed `agent_message` to standard output immediately. This is message-level streaming; the inspected Codex protocol does not provide token deltas here. Thread identifiers, tool events, usage records, and raw diagnostic logs are not displayed. Empty successful replies and invalid JSON produce explicit errors.

Structured errors are classified as authentication, usage-limit, connection, or general request failures. Free-form error strings and stderr logs are withheld because they can contain prompts, private URLs, or credentials. For detailed diagnostics, run Codex directly in a terminal. A nonzero subprocess exit remains a failed HarborVM turn even when partial assistant text was already shown.

Default limits are 64 KiB per user prompt, 120 seconds per turn, and 8 MiB of combined subprocess stdout and stderr. The output bound includes unfinished JSON lines. `RunLimits` lets library callers choose a shorter deadline or a different output bound.

The desktop cancel action must send `SIGTERM` to the HarborVM assistant process; terminal users can use Ctrl-C (`SIGINT`). HarborVM starts Codex in a separate process group and, on cancellation, timeout, output overflow, or completion, sends that group `SIGTERM`, waits 200 milliseconds, and follows with `SIGKILL` before reaping the direct child. Descendants that remain in the process group are included. A program that deliberately escapes its process group is outside this cooperative cancellation guarantee. As with other processes, killing HarborVM itself with `SIGKILL` bypasses its cleanup handlers.

## Verification

`tests/assistant.rs` uses temporary executable fixtures to verify argument and prompt boundaries, private work-directory permissions, filtered JSON output, authentication and missing-program errors, deadlines, output limits, and descendant cleanup. These fixtures do not call a model and are not evidence of an AI response. Real CLI and desktop integration checks are recorded separately in the release validation document.
