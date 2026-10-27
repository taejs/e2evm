# Give your coding agent its own browser

Your existing agent invokes the `e2evm` CLI to work inside a dedicated guest.
E2EVM's right-side assistant is a chat panel; its replies do not execute commands.
This kit covers the Ubuntu browser environment on Apple Silicon, macOS 14+.

## Choose the skill for the job

| Skill | When to use it |
| --- | --- |
| [`e2evm-setup`](../skills/e2evm-setup/SKILL.md) | Locate or build E2EVM, reuse or prepare a dedicated Linux guest, verify readiness, and deliver a browser smoke screenshot. |
| [`guest-e2e`](../skills/guest-e2e/SKILL.md) | Test an application in that guest, install necessary guest dependencies, and return assertions with inspected screenshots. |

There are two skills because initial VM setup and day-to-day application testing
have different scopes. Dependency installation stays with the test workflow:
inspect the guest, follow the project's lockfiles, preserve disabled components,
and supervise long jobs.

## Install selected skills

From the E2EVM checkout, inspect the bundle and preview installation:

```sh
python3 scripts/install-agent-skills.py --list
python3 scripts/install-agent-skills.py --agent codex --dry-run
```

Install both into Codex's personal skill directory:

```sh
python3 scripts/install-agent-skills.py --agent codex
```

Or install into Claude Code's personal directory, or an explicit project directory:

```sh
python3 scripts/install-agent-skills.py --agent claude
python3 scripts/install-agent-skills.py --dest /path/to/project/.claude/skills
```

These are alternatives; choose the target you use. `--agent codex` honors
`CODEX_HOME` and otherwise uses `~/.codex/skills`; `--agent claude` uses
`~/.claude/skills`. Claude Code also supports the project's `.claude/skills`
[directory](https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview).
For another agent, choose its documented skill directory with `--dest`, or provide
it the installed `SKILL.md` path explicitly. This is a portable instruction kit,
not a claim of a native plugin for every agent or Claude Desktop chat mode.

Select only what you need with `--skills e2evm-setup` or `--skills guest-e2e`.
`--list --agent codex` inventories target collisions. The installer checks every
selected destination before copying and refuses any existing directory, file, or
symlink. It does not update existing skills, modify agent configuration, or install
E2EVM itself. For an upgrade, inspect the differences and choose a fresh destination
or deliberately manage your old copy. Interrupted copies are reported for inspection.

Reload the agent's skill list or start a new session after installation. The
installer needs only Python 3's standard library. Copying an entire skill folder
manually also works; keep its scripts, references, and metadata together.

## First setup

For manual prerequisites and commands, follow [Install E2EVM](install.md). To have
your agent do the authorized setup, use:

> Use $e2evm-setup to set up E2EVM from [checkout path]. Reuse dedicated profile [ID] if suitable, otherwise create a dedicated Linux E2E guest. Preserve my component choices and other VMs. Verify the guest browser and attach an inspected screenshot. Leave it running.

The setup skill can be installed by itself. Its bundled helper runs a local HTML
fixture inside guest Chrome, asserts the title and heading, returns a screenshot,
and closes only its test tab. A fixture pass proves the integration, not your app.

For browser startup, the agent checks `command -v e2evm-browser` inside the
selected guest. New installs provide that name; an older prepared guest may
only have executable `/usr/local/bin/harbor-browser`, which can be reused as-is.
An already running guest Chrome can be attached directly. Missing the new alias
does not require package reinstallation, a setup rerun, or VM migration. If both
launchers are absent, inspect the guest setup choices and logs; do not switch to
a host browser. The skills include this compatibility check.

Use the same `E2EVM_HOME` as the desktop if you configured a custom data directory.
Setup components default to enabled and creation accepts `--without` options;
agents preserve deliberately disabled choices. The supplied visible Python
browser recipe needs desktop, browser, Python, and Playwright. macOS guest setup
is separate; see [the macOS guide](macos.md).

## Delegate real application work

Once setup passes, supply the source and a URL reachable from the guest:

> Use $guest-e2e with profile [ID] to verify [code change] at [URL]. First confirm that [revision or approved local changes] is running there. Run [assertions or project suite], attach inspected screenshots of the actual result and any failure, and report the guest command outcome. Leave the VM running.

The [project instruction snippet](../examples/agent-instructions.md) works in
`AGENTS.md` or `CLAUDE.md`; append it without replacing existing project rules.
[More prompts](../examples/prompts.md) cover setup, smoke tests, complete flows,
and missing dependencies.

Every browser E2E report requires actual screenshots: capture before cleanup,
transfer them to the host, inspect the images, then embed or attach them in the
reply with captions. Paths alone are insufficient. If evidence cannot be captured
or delivered, report the test outcome separately and mark screenshot evidence
blocked. The app's chat panel does not attach these images automatically.

## Connection boundaries

| Component | Behavior |
| --- | --- |
| Coding agent | Runs the host CLI; `exec` commands run in the selected guest. |
| Guest Chrome | Persistent browser profile; Playwright connects from guest localhost. |
| App assistant | Codex, Ollama, or compatible chat API; no autonomous guest execution. |
| Project files | No automatic mount or sync. Clone an authorized revision or arrange an explicit transfer of local changes. |
| Development server | Guest localhost reaches the guest. A Mac-only localhost service needs an explicitly reachable address or a guest deployment. |
| Results | CLI output returns to the agent. Guest files need explicit transfer; the smoke helpers return small screenshots. |

No model API key is required for the skills or CLI transport. Your chosen coding
agent's service requirements still apply. See the test skill's
[environment reference](../skills/guest-e2e/references/environment.md) for project
transfer, dependency checks, and supervised jobs beyond the 60-second command
limit. It keeps project installs in the guest and never falls back to a host browser.
