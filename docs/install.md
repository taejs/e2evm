# Set up E2EVM

Use this guide when installing E2EVM yourself or asking a coding agent to prepare its browser workspace. E2EVM currently builds from source on Apple Silicon with macOS 14 or newer. It does not have a prebuilt or notarized installer.

## For the agent handling setup

Carry out the setup below, reusing suitable existing tools and a dedicated test VM. Build or locate the app, install the two supplied skills for the calling agent using the [skill installer](agents.md#install-selected-skills), and run the bundled browser check. Infer the agent's install target from your current environment; for other clients, use their documented skill directory or load the local skill paths. Preserve existing skill files if the installer reports a collision.

Keep the workspace in the background. Finish by opening the actual returned screenshot for inspection and attaching it with the two assertion results, CLI path, and selected profile ID. If a prerequisite needs the user's interaction, explain that specific step and continue independent setup work. Do not finish with commands for the user to run when you can carry out the authorized setup.

## Check the Mac

```sh
uname -s
uname -m
sw_vers -productVersion
xcode-select -p
rustc --version
cargo --version
node --version
python3 --version
```

The host must report `Darwin`, `arm64`, and macOS 14+. It needs [Xcode Command Line Tools](https://developer.apple.com/xcode/resources/), [Rust stable](https://www.rust-lang.org/tools/install) 1.85+, [Node.js](https://nodejs.org/en/download) 22.15+, and Python 3 for the agent setup and smoke recipe. Node decompresses the Ubuntu boot kernel; it does not run the guest browser on the host. Install missing host tools using their official instructions and the user's existing package manager. An Apple installation dialog may need the user to finish it before setup can continue.

Preparation and first boot need internet access and disk space for downloaded images, build output, and a 24 GiB sparse disk. The sparse disk grows as the guest writes data. A new browser VM below uses 2 CPUs and 4 GiB memory; check available memory before starting it.

## Build the app and CLI

Clone into an unused directory. If a checkout already exists, inspect its origin and local changes and reuse it deliberately instead of replacing files or resetting it.

```sh
git clone https://github.com/taejs/e2evm.git
cd e2evm
sh scripts/build-macos.sh
export PATH="$PWD/dist/E2EVM.app/Contents/MacOS:$PATH"
e2evm help
e2evm profile list
```

The build produces `dist/E2EVM.app` and its bundled `e2evm` CLI. The `PATH` setting above lasts for this terminal session. Agents should retain the absolute CLI and checkout paths rather than assume a later shell inherits that setting. To keep VM data outside the default `~/Library/Application Support/E2EVM`, set `E2EVM_HOME` consistently for the desktop and every CLI call.

## Prepare a dedicated browser VM

Inspect existing profiles first. Reuse only a profile selected for this task, or choose an unused ID. Do not prepare or overwrite a VM belonging to another application.

```sh
e2evm profile create e2e --os linux --name "E2E workspace" --cpus 2 --memory 4096
e2evm prepare e2e
e2evm start e2e
e2evm status e2e
```

`prepare` downloads and verifies the pinned Ubuntu image and creates the VM disk. First boot installs the selected desktop and browser tools and can take several minutes. Poll status sequentially with a bounded wait. Wait for `state: running` and `guest_online: true`, then check setup completion:

```sh
e2evm exec e2e -- test -f /var/lib/harborvm/setup-complete
```

An online worker does not mean browser setup is complete. If the marker is absent, inspect setup progress and retry within the bounded wait. Do not start a competing package installation. Setup components start enabled and can be disabled; preserve a user's deliberate choices. See [setup choices and troubleshooting](usage.md).

`start` keeps the app in the background. Open it with `e2evm app` when you want to inspect the VM: the first presentation enters full screen, with the console collapsed. Use **⌘J** for the console and **⌃⌘F** for full screen. An agent can test without opening that window.

## Give the agent its skills

Follow the [agent kit](agents.md) to install the supplied skills or provide their local `SKILL.md` paths to your existing agent. Read the selected skill before testing; the assistant panel itself does not execute commands. Project instructions and copyable task prompts are included.

Run the bundled [browser smoke recipe](../skills/guest-e2e/references/browser-smoke.md) inside this guest. It checks a local HTML fixture, asserts its title and heading, and transfers a screenshot to the host. Open the returned image and attach it in the final report with the assertion result. A missing image or a path-only report is incomplete evidence. Keep the VM available unless shutdown was requested.

Passing this fixture verifies the VM/browser integration. It does not verify the user's application. For an application flow, follow the skill to bring the requested revision or reachable development URL into the guest, install its dependencies there, and run the relevant E2E checks.

macOS guests have separate manual onboarding and are not a substitute for this Linux browser recipe. See the [macOS guide](macos.md).
