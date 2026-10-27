# E2EVM FAQ

## What is E2EVM?

E2EVM is an open-source local VM workspace for agent-driven browser testing on Apple Silicon. It gives browser tests a separate desktop inside a native Mac app, with a guest command console and an optional AI chat panel. Its Rust core handles profiles, commands, and connections; Apple's Virtualization framework runs the VM. The source is [MIT licensed](../LICENSE).

Start with [installation](install.md), browse the [use cases](use-cases.md), or read the [architecture](architecture.md).

## How does it keep my desktop usable, and why use a headed browser?

The test browser and commands run inside the VM while you keep using your Mac. Agent-triggered startup and profile selection preserve your current app's focus; open E2EVM explicitly when you want to inspect the browser. Guest input does not move the host pointer or synchronize the clipboard.

A headed browser is useful when you want to see a flow or reuse a dedicated guest test login. Headless tests remain useful, and a project can run its own headless suite inside the guest with the required dependencies. E2EVM does not replace Playwright or claim to fix a general headless-browser problem. See [browser usage](usage.md#browser-and-guest-commands) and the [recorded focus/input checks](validation.md#real-guest-and-application-checks).

## How do I install E2EVM?

Give the setup guide to your CLI-capable coding agent, or follow it manually. Paste these two lines into the agent:

```text
Set up E2EVM for me by following this guide:
https://raw.githubusercontent.com/taejs/e2evm/feat/vm-workspace/docs/install.md
```

This preview builds from source. The [installation guide](install.md) covers prerequisites, a dedicated Linux guest, skill installation, and an actual browser check with a screenshot. There is no prebuilt or notarized installer. Passing that setup check verifies the browser connection; your application's own assertions are still needed.

## Which Macs and guest operating systems are supported?

The native workspace requires an Apple Silicon Mac running macOS 14 or newer, and it can run Linux or macOS guests. The supplied browser-testing skills target the Ubuntu guest. Its optional desktop, Chrome, Python tools, Node.js, and Python Playwright setup starts selected and can be disabled.

macOS guests require Apple's first-run setup and a separate manual connection bootstrap. Installation was verified through the visible Setup Assistant; completing a guest account and its connection bootstrap has not been verified end to end. Intel Macs, Linux/Windows host backends, and Windows guests are not implemented. Portable Rust and worker tests can run on Linux. See [macOS setup](macos.md) and [validation limits](validation.md#boundaries-of-this-evidence).

## Does my whole agent run in the VM, and can the chat panel operate it?

Your existing coding agent invokes the host CLI to run browser tests and commands inside the guest; the app's chat panel does not operate the VM. The supplied `e2evm-setup` and `guest-e2e` skills tell a CLI-capable agent how to prepare the environment, run tests, and deliver evidence.

The right-side assistant displays suggestions. Its replies are never executed automatically, and guest files, console output, and screenshots are not attached to its conversation automatically. See the [agent kit](agents.md) and [assistant contract](assistant.md).

## Can I connect Codex, Ollama, or another model API?

The assistant panel supports an existing Codex CLI sign-in, Ollama's native text chat API, and OpenAI-compatible text Chat Completions APIs. Ollama must already be running with a model installed; compatible services need the documented endpoints and streaming format. Support does not extend to every vendor API or authentication scheme.

The chat connection runs from the Mac, so its `localhost` endpoint refers to the host. No model key is needed for E2EVM's skills or command transport, although your chosen coding agent or model service has its own requirements. See [connection setup and protocol details](assistant.md). API adapters were checked with local fixtures; a live Ollama model or paid compatible service was not used in the [recorded validation](validation.md#boundaries-of-this-evidence).

## How do I get results and screenshots back from a test?

Ask your coding agent to return the assertions and attach inspected screenshots from the tested guest browser. The test skill requires captures of the relevant final or failure state before cleanup, explicit transfer to fresh host files, image inspection, and attachment in the reply.

A guest-only file path is not a delivered screenshot, and a screenshot alone does not prove assertions passed. If capture or delivery fails, the agent must report the test outcome and the evidence blocker separately. The [browser smoke recipe](../skills/guest-e2e/references/browser-smoke.md) provides a working small-image transfer example. Native cached-view screenshots can omit some rendered content; inspect them against the [known limits](validation.md#boundaries-of-this-evidence).

## Can the guest access my local project and localhost development server?

Your Mac's project files and `localhost` are not automatically available inside the guest. Clone an authorized revision into the guest or arrange an explicit transfer of the local changes you want tested. Run the app inside the guest, or provide a development/staging address that the guest can reach.

Guest `127.0.0.1` refers to the guest. There is no automatic project sync, host port forwarding, or bulk-copy command. A remote clone does not contain unpublished edits from your Mac. See [project transfer, networking, and dependency guidance](../skills/guest-e2e/references/environment.md).

## Can I repeat or schedule browser automation while I do something else?

You can reuse a named guest for repeated runs, but scheduled execution needs a separately configured scheduler or agent automation. Keep the Mac awake and the guest running and available for each run. Closing E2EVM's workspace window keeps guests running; quitting the app is refused while a VM is active.

E2EVM has no built-in scheduler. Commands have a 60-second request limit; longer suites can use the documented supervised guest-job workflow and report their actual completion status. See [repeatable workflows](use-cases.md#repeat-a-release-smoke-test-in-the-same-workspace) and [long-running jobs](../skills/guest-e2e/references/environment.md#jobs-longer-than-60-seconds).

## Is this a security sandbox for untrusted code?

E2EVM provides a separate VM workspace, not a guarantee that arbitrary untrusted code is safe to run. The guest has its own disk and browser state, plus a dedicated writable command mailbox and a read-only setup kit; your host home directory is not automatically shared.

The Linux guest has passwordless sudo. Guest networking can reach the LAN and internet, and host software running as your user is outside this isolation claim. Use a dedicated test VM and authorized test accounts. The source preview is locally signed, not notarized or independently security-audited. Read the [security policy](../SECURITY.md) for the actual trust boundaries and the [architecture](architecture.md#profiles-and-ownership) for ownership details.
