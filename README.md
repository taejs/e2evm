# E2EVM

![E2EVM — Let your agent test. Keep your desktop.](assets/brand/social-preview.png)

A local VM workspace for agent-driven browser testing on Apple Silicon, with a Rust core and CLI.

Keep a real VM desktop, guest command console, and AI conversation in one native Mac window. Linux machines can install a desktop, Chrome, Python tools, Node.js, and Playwright on first boot. Every optional component starts selected and can be turned off. Choose an existing Codex CLI, an Ollama server, or an OpenAI-compatible chat API in the right panel.

![E2EVM workspace with a live Linux browser, guest console, and assistant panel](docs/images/workspace.png)

## What you can do

- Create separate, named machines and start, pause, resume, or shut them down.
- Browse in a visible guest Chrome window with a persistent browser profile.
- Run commands inside the guest and inspect output, exit status, and timeouts.
- Ask an assistant follow-up questions, select API models, and cancel a response.
- Use the same profile, lifecycle, command, and chat operations from a Rust CLI.

The native shell uses SwiftUI, AppKit, and Apple's Virtualization framework. Rust owns profiles, preparation, guest command transport, and assistant connections. The app runs Linux and macOS guests on **Apple Silicon with macOS 14 or newer**. macOS guests require manual first-run account setup and connection bootstrap; see the [macOS guide](docs/macos.md). It is an early source release; there is no notarized installer or automatic updater.

## Build and open your first machine

Install [Rust](https://www.rust-lang.org/tools/install) stable (1.85 or newer), [Xcode Command Line Tools](https://developer.apple.com/xcode/resources/), and [Node.js](https://nodejs.org/en/download) 22.15 or newer on the Mac. Node is used to decompress the Ubuntu boot kernel. Python 3 is needed for contributor tests. Preparation and first boot need internet access and room for a 24 GiB sparse disk, downloaded images, and build output. The sparse disk grows as the guest writes data.

```sh
git clone https://github.com/taejs/e2evm.git
cd e2evm
sh scripts/build-macos.sh
export PATH="$PWD/dist/E2EVM.app/Contents/MacOS:$PATH"

e2evm profile create ubuntu --name "Ubuntu Workspace"
e2evm prepare ubuntu
e2evm start ubuntu
e2evm app
```

The default machine has 2 CPUs, 2 GiB memory, and a 24 GiB disk. First boot installs the desktop and browser tools, which can take several minutes. The command worker can become available before the desktop finishes. Chrome opens the included welcome page when setup completes. The desktop's **New machine** and **Prepare Linux** controls provide the same workflow.

```sh
e2evm exec ubuntu -- e2evm-browser https://example.com
e2evm exec ubuntu -- python3 --version
e2evm run ubuntu 'cd ~/Workspace && .venv/bin/python -m playwright --version'
e2evm pause ubuntu
e2evm resume ubuntu
e2evm stop ubuntu
```

`start` launches the workspace in the background so an agent does not take focus from your current app. `app` brings the window forward when you want to inspect it. `stop` requests a clean shutdown; check `e2evm status ubuntu` until the machine is stopped. Closing the window keeps guests running. Reopen it with `e2evm app`. Quitting is refused while a machine is active.

## Give an E2E agent its own workspace

Ship the test, keep working on your Mac. The [agent kit](docs/agents.md) includes a portable skill, project instructions, and copyable prompts for an existing coding agent. It explains how to start the selected guest, run Playwright, install task dependencies inside that guest, and return assertions with actual screenshots attached. Screenshot delivery is required for a completed report; a capture failure must be reported as incomplete evidence.

The [instructions](examples/agent-instructions.md) can be added to a project's `AGENTS.md` or `CLAUDE.md`. The [prompts](examples/prompts.md) cover a smoke test, a complete application flow, and missing dependencies. Your agent invokes the CLI; this does not turn the assistant chat panel into an automatic tool runner.

For a minimal Linux machine, use `--without all`. To omit selected tools, use `--without node,playwright`. Turning off a prerequisite also turns off its dependents. The core connection runtime remains available. See [setup choices](docs/usage.md).

## Connect an assistant

Codex is the default and uses your installed CLI and its existing sign-in. E2EVM does not install or authenticate it. Select a different connection in the assistant's settings, or configure one from the terminal:

```sh
e2evm profile connection ubuntu ollama --endpoint http://127.0.0.1:11434
e2evm assistant models ubuntu
e2evm profile connection ubuntu ollama --endpoint http://127.0.0.1:11434 --model YOUR_MODEL_ID
e2evm chat ubuntu 'How can I check the Python version in my guest?'
```

Ollama must already be running with a model installed. For compatible services, configure the service's base URL and model; the desktop accepts an optional API key held only in memory for the app session. See [assistant connections](docs/assistant.md) for Codex setup, OpenAI-compatible examples, supported protocols, privacy, and cancellation.

Assistant replies are suggestions. They do not run guest commands. Conversations live in the app session and are sent to the selected provider; guest files, browser sessions, screenshots, and console output are not attached automatically.

## Learn more

- [macOS guests and Apple licensing](docs/macos.md)
- [Agent skill, instructions, and prompts](docs/agents.md)
- [Usage and troubleshooting](docs/usage.md)
- [Architecture and isolation](docs/architecture.md)
- [Design references](docs/design.md)
- [Recorded validation and known limits](docs/validation.md)
- [Contributing](CONTRIBUTING.md), [security](SECURITY.md), and [community conduct](CODE_OF_CONDUCT.md)

The console runs individual commands, not an interactive terminal. Windows guests, snapshots, disk import, automatic assistant actions, and cross-platform host backends are not implemented. Native screenshot capture has platform rendering limits; see the validation record before relying on it for automation.

E2EVM source is [MIT licensed](LICENSE). Ubuntu, Chrome, Apple frameworks, and other dependencies have their own licenses; see [dependencies](docs/dependencies.md). Chrome is downloaded separately into the guest during setup and is not covered by E2EVM's MIT license.
