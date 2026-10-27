# Assistant connections

The right panel supports three text-only connections: **Codex CLI** (the default), **Ollama**, and **OpenAI-compatible Chat Completions APIs**. Connections belong to a VM profile. They run on the host; a localhost endpoint means the Mac running HarborVM, not the guest.

The assistant receives the selected profile's ID and name plus the conversation you explicitly submit. It does not automatically receive guest files, terminal output, clipboard contents, screenshots, or VM disks. Replies are advisory text and never execute commands. The guest command console is a separate action.

## Choose a connection

```sh
# Keep or return to the default local CLI.
harborvm profile connection ubuntu codex
harborvm profile assistant ubuntu /absolute/path/to/codex

# Ollama must already be running, with a model installed.
# Omit --model until you have discovered the available model IDs.
harborvm profile connection ubuntu ollama --endpoint http://127.0.0.1:11434
harborvm assistant models ubuntu
harborvm profile connection ubuntu ollama --endpoint http://127.0.0.1:11434 --model YOUR_MODEL_ID

# Use the base URL supplied by your compatible service, including its /v1 path if required.
harborvm profile connection ubuntu openai-compatible --endpoint https://YOUR_SERVICE/v1 --model YOUR_MODEL_ID --key-env HARBORVM_PROVIDER_TOKEN
```

`profile connection` replaces the connection settings; omitted settings are cleared. Existing profiles without `assistant_connection` continue using Codex. The saved JSON contains `provider`, optional `endpoint`, optional `model`, and optional `key_env`. It never contains an API key. `key_env` names an environment variable in the HarborVM process; HarborVM reads only that explicitly configured variable. A desktop app launched from Finder may not inherit terminal environment variables.

In the desktop connection sheet, select a provider, enter its base URL, and load its models or type a model ID. The optional API key field supplies a key through the child process's standard input. The key is held in memory for the current app session, not stored in the profile. A submitted key takes precedence over a configured environment variable. Codex manages its own authentication and does not accept this field.

HarborVM does not install Ollama, download model weights, create keys, or make a paid test request when saving settings. Loading models contacts the configured service; sending a message uses that service's normal account and billing rules.

## API compatibility

| Connection | Model discovery | Chat | Response format |
| --- | --- | --- | --- |
| Ollama | `GET <base>/api/tags` | `POST <base>/api/chat` | NDJSON `message.content`, ending with `done: true` |
| OpenAI-compatible | `GET <base>/models` | `POST <base>/chat/completions` | SSE `choices[0].delta.content`, ending with `[DONE]` |

Requests contain `model`, the full `messages` array, and `stream: true`. HarborVM adds one advisory system message. The adapter does not request tools, images, audio, reasoning fields, or vendor-specific options. Services must implement these endpoints and text streaming shapes; “compatible” does not mean every model service or authentication scheme works. Keys use HTTP Bearer authentication when supplied. Remote endpoints require HTTPS; HTTP is accepted only for loopback IP addresses or `localhost`. URL credentials, queries, and fragments are rejected. Redirects are refused, so a service cannot redirect a key to another endpoint. Automatic request retries are disabled. Proxy environment variables are ignored; TLS certificates must validate against the bundled public roots.

Protocol references: [Ollama chat](https://docs.ollama.com/api/chat), [Ollama model listing](https://docs.ollama.com/api/tags), and the [OpenAI Chat Completions reference](https://platform.openai.com/docs/api-reference/chat).

## CLI conversation contract

```sh
harborvm chat ubuntu 'Explain how to list files in the guest.'

# The JSON entry point avoids putting conversation text in process arguments.
printf '%s' '{"messages":[{"role":"user","content":"What is pwd?"},{"role":"assistant","content":"It prints the current directory."},{"role":"user","content":"Show an example."}]}' | harborvm chat-json ubuntu
```

`chat ID PROMPT` preserves the single-turn interface. `chat-json ID` reads a JSON object from standard input with `messages` and an optional `api_key` string. Message roles are `user` and `assistant`; content is plain text. Supply 1–128 nonempty messages ending with a user message. The desktop submits the selected profile's conversation history. Conversations are not persisted by the Rust backend. The desktop starts a fresh conversation when the connection changes. CLI callers control the history they supply to the selected service.

`assistant models ID` returns a JSON array such as `[{"id":"model-name"}]`, sorted and deduplicated. `assistant models-json ID` accepts `{}` or an object containing `api_key` on standard input. This allows desktop model discovery without putting a key in process arguments. There is no API model discovery for Codex; its CLI selects its own model.

Both JSON entry points also accept `expected: {"connection": CONNECTION, "program": PROGRAM_OR_NULL}`. Long-lived clients should include the complete connection snapshot and assistant program associated with their key and conversation. Rust compares that snapshot with the profile it will actually use before resolving credentials or contacting a service; a mismatch fails without sending. The desktop always supplies this binding and clears stale state after refreshing a changed connection. Omitting it is reserved for CLI callers intentionally selecting the current saved connection at invocation time.

Successful chat output is plain UTF-8 assistant text on stdout, flushed as it arrives. Errors use the CLI's JSON stderr envelope and a nonzero exit. A failed or cancelled turn can have partial text on stdout; callers must not treat partial text as a completed answer.

## Codex CLI

Sign in through Codex's normal terminal flow:

```sh
codex login status
# If needed:
codex login
```

The adapter finds `codex` on `PATH`, then tries `~/.local/bin/codex`. `assistant_program` can select an absolute executable path or a program name on `PATH`. It must implement `codex exec --json`; it is not a shell command. Relative paths and command strings with arguments are not supported. Only select an executable you trust, since it runs under your host account.

Every turn uses:

```text
exec --ignore-user-config --ephemeral --sandbox read-only
     --skip-git-repo-check --json --color never -C <assistant-work> -
```

Prompts and conversation context go to Codex's standard input, never through a shell. The process starts in a private mode-0700 `assistant-work` directory inside the profile. Global user configuration is ignored to avoid unrelated instructions, hooks, or integrations. Authentication stays managed by Codex. Use a Codex release that supports these flags; unsupported versions fail visibly.

The Codex process is ephemeral. For `chat-json`, HarborVM serializes the submitted prior turns into its prompt, so follow-up questions retain the supplied context without resuming a hidden CLI session. Read-only sandboxing restricts changes; it is not a guest-isolation boundary or a guarantee that host files cannot be read. Codex uses its configured account and service, which may consume the normal account allowance.

Codex JSON Lines are filtered to completed `agent_message` text and flushed immediately. This is message-level streaming. Thread IDs, tool events, usage records, and raw diagnostic logs are withheld.

## Limits, errors, and cancellation

Input JSON and conversation content are limited to 64 KiB. API operations have a 120-second deadline and a 10-second connection timeout. Responses are bounded to 8 MiB, with at most 256 KiB per API stream line or SSE event. Invalid JSON, empty replies, missing stream completion, and overflow fail visibly. Model discovery uses the same total response bound. A profile lock prevents simultaneous chat turns across processes.

Authentication and rate-limit errors report their HTTP status. Free-form service errors, URLs from connection failures, and raw diagnostic bodies are withheld because they may contain private data. Configure the final endpoint directly when a redirect is refused. For detailed service diagnostics, use the service's own tools.

The desktop cancels with `SIGTERM`; terminal users can press Ctrl-C. API cancellation drops the active request and closes the stream. Codex cancellation, timeout, overflow, and completion terminate its process group, wait 200 milliseconds, then send `SIGKILL` and reap the direct child. A deliberately escaping descendant is outside cooperative process-group cleanup. Killing HarborVM itself with `SIGKILL` bypasses its cleanup handlers.

## Verification

`tests/assistant_api.rs` uses loopback HTTP fixtures to verify Ollama and compatible streaming, conversation history, both model-list formats, environment and stdin keys, redirect refusal, invalid endpoints, private error handling, truncation, bounds, early output, and cancellation closing the connection. A temporary Codex fixture verifies that JSON conversations include prior turns. `tests/assistant.rs` retains subprocess argument, permissions, authentication, deadlines, bounds, streaming, and descendant cleanup coverage. These fixtures do not call a model and are not evidence of a real Ollama or hosted API response. Real integrations are recorded separately in release validation.
