# Give your agent its own browser workspace

Run E2E flows in the guest while you continue using your Mac. The integration is a CLI plus a portable skill, repository instructions, and ready-to-use prompts. Your existing coding agent invokes the CLI; the right-side assistant conversation does not automatically operate the guest.

This guide covers the current Ubuntu browser environment on Apple Silicon. E2EVM is the public product and CLI name.

## Prepare once

Build the native app using the [installation instructions](../README.md), then put the bundled CLI on your terminal's path. From this repository:

```sh
export PATH="$PWD/dist/E2EVM.app/Contents/MacOS:$PATH"
e2evm help
e2evm profile list
```

Use an existing dedicated test profile, or create a new one with an unused ID:

```sh
e2evm profile create e2e --os linux --name "E2E workspace" --cpus 2 --memory 4096
e2evm prepare e2e
e2evm start e2e
e2evm status e2e
```

First boot downloads and installs the browser tools. Wait for `guest_online: true`, then check `e2evm exec e2e -- test -f /var/lib/harborvm/setup-complete`. A running worker alone does not mean Chrome is ready. Use the same `E2EVM_HOME` for the agent and desktop if you configured a custom data directory. Keep this guest separate from production VMs and accounts.

Setup components are enabled by default. At creation, `--without node` omits Node.js, while `--without all` omits the optional workstation tools. Inspect `e2evm profile show e2e` before relying on a component. The Python browser smoke recipe requires desktop, browser, Python, and Playwright; an agent should preserve deliberately disabled choices instead of silently reinstalling them. This Linux guide does not cover macOS guest onboarding.

## Install the skill, or point an agent to it

The complete skill lives at [`skills/guest-e2e`](../skills/guest-e2e/SKILL.md). Copying that directory includes its references. For Codex, run this one command from the repository root. It copies into your skills directory and **fails if `guest-e2e` already exists**; it does not replace existing configuration:

```sh
python3 - <<'PY'
import os
from pathlib import Path
import shutil

source = Path("skills/guest-e2e")
destination = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "skills" / "guest-e2e"
if not (source / "SKILL.md").is_file():
    raise SystemExit("Run this from the E2EVM repository root")
destination.parent.mkdir(parents=True, exist_ok=True)
shutil.copytree(source, destination)
print(f"Installed {destination}")
PY
```

Open a new agent session if its skill list was already loaded, then invoke `$guest-e2e`. No model API key is required by the skill or CLI transport; your chosen agent's own service requirements still apply.

For another agent, provide the local path to `SKILL.md` and let it read the linked references. Use the [repository instruction snippet](../examples/agent-instructions.md) in your project's `AGENTS.md` or `CLAUDE.md`; append it deliberately rather than replacing existing instructions. The snippet gives the agent a specific profile and skill path. This is a documented CLI integration, not a claim of a native plugin for every agent.

## Delegate a test

First check the browser connection using the skill's self-contained fixture:

> Use $guest-e2e with profile e2e. Run its bundled browser smoke recipe against the local HTML fixture and return the assertions, screenshot, and guest command result. Keep the VM running.

For your own application, supply a URL reachable from the guest and its expected results. Passing the fixture verifies the integration, not your application. [More prompts](../examples/prompts.md) cover a complete flow and missing dependencies. The skill contains a [runnable smoke recipe](../skills/guest-e2e/references/browser-smoke.md) that produces host-side evidence.

Your agent can install needed tools **inside the guest**, using existing Ubuntu, npm, or Python tools. It first checks what is installed and follows your project's dependency versions. Long installs and suites run as supervised guest jobs because each CLI command has a 60-second limit.

## What is and is not connected

| Component | Connection |
| --- | --- |
| Coding agent | Runs the host `e2evm` CLI; guest commands execute inside the selected VM. |
| Visible browser | Guest Chrome with a persistent profile; Python Playwright can attach from guest localhost. |
| App assistant | Codex, Ollama, or a compatible chat API; replies do not trigger guest actions. |
| Local project | Not mounted or synchronized automatically. Clone an approved revision inside the guest or arrange an explicit transfer. |
| Local development server | Guest localhost reaches the guest. A Mac-only localhost service needs an explicitly reachable address or a guest deployment. |
| Results | CLI output returns to the agent. Guest files stay in the guest unless explicitly transferred; the smoke recipe returns a small screenshot. |

See the skill's [environment guide](../skills/guest-e2e/references/environment.md) for networking, project transfer, dependency checks, and long jobs. The guest is a separate workspace with networking and a shared command mailbox; it is not a promise that arbitrary untrusted code is harmless. Keep test actions, credentials, and cleanup within the requested scope.
