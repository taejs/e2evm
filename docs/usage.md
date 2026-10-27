# Usage and troubleshooting

Build the app as described in the [README](../README.md). Examples below assume the bundled CLI is on `PATH`:

```sh
export PATH="$PWD/dist/E2EVM.app/Contents/MacOS:$PATH"
e2evm help
```

## Machines

```sh
e2evm profile create research --name "Research" --cpus 2 --memory 4096
e2evm profile list
e2evm profile show research
e2evm prepare research
e2evm start research
e2evm status research
```

`start` brings up the native workspace in the background and preserves the current app's focus. Use `e2evm app` when you want to bring its window forward. Selecting a machine with the CLI also preserves host focus.

While the native app is running, `status` also reports `workspace.visible`, `workspace.active`, `workspace.full_screen`, and the requested profile's `workspace.console_visible`. Querying these fields does not bring the app forward. After the app quits, a stale private socket is treated as stopped; invalid or unsafe socket paths and connected protocol failures still return errors.

Memory is in MiB. Creation accepts 1–32 CPUs and 512–65536 MiB; the host framework still validates resources before boot. Linux defaults to a 24 GiB sparse disk. Use `--disk GIB` at creation to select a different size. macOS profiles use `--os macos` and default to 4 GiB RAM and a 64 GiB disk; see [macOS setup](macos.md). There is no supported command to resize an existing machine or import another VM's disk.

`prepare` downloads the pinned Ubuntu ARM64 image and checks official SHA-256 manifests. `prepare ID --cache DIRECTORY` may read the three matching cached artifacts, but it still fetches current manifests from the pinned upstream directory and verifies every file. A cache is not a disk import feature.

Linux first boot installs the selected workstation components. In New machine, every optional component starts checked. CLI creation supports `--without desktop,browser,python,node,playwright` or `--without all`. Disabling desktop also disables browser/Playwright; disabling browser or Python tools also disables Playwright. Python development tools are optional, while the core Python command runtime remains part of the base connection. A running VM with `guest_online: true` may still be installing its desktop. Wait for Chrome's welcome page before treating graphical setup as finished.

```sh
e2evm pause research
e2evm resume research
e2evm stop research
e2evm status research
```

A graceful stop remains pending until the guest exits. `e2evm poweroff research` force-stops the machine and can lose unsaved data. Resume a paused machine before requesting a graceful shutdown. Closing the workspace window leaves guests running; `e2evm app` reopens it.

## Browser and guest commands

For the Linux guest, the user is `harbor`. Its desktop logs in automatically. The included browser launcher opens URLs in the guest's Chrome session:

```sh
e2evm exec research -- e2evm-browser https://example.com
e2evm exec research -- /usr/bin/printf 'Hello, %s\n' '안녕하세요'
e2evm run research 'cd ~/Workspace && .venv/bin/python -m playwright --version'
e2evm exec research -- node --version
```

`exec ID -- PROGRAM ARG...` preserves argument boundaries. `run ID 'SHELL-LINE'` intentionally uses `/bin/sh -lc` in the guest; quote the line so the host shell does not expand it first. The command timeout is 60 seconds. Results include output, exit code, timeout, and truncation state. Failed guest commands make the CLI exit nonzero. Commands never fall back to the Mac when the guest is unavailable.

Use the desktop console for the same command-and-result workflow. It starts collapsed; choose **Show console** or press **⌘J**. The first explicit `e2evm app` opening enters full screen, which can be toggled with **⌃⌘F**. Agent-triggered `start` remains in the background.

The console does not support interactive programs waiting for terminal input. Long-running background jobs should redirect their standard streams and be supervised inside the guest. After a command timeout, output draining is limited to another 0.5 seconds so a detached child cannot block later requests by holding stdout open. A detached process may still be running; inspect the guest before retrying work.

Chrome keeps its profile at `/home/harbor/.local/share/harborvm/chrome`. The Python environment is `/home/harbor/Workspace/.venv`; Python Playwright is installed there. Node.js and npm come from Ubuntu packages. Browser state persists on the VM disk across normal restarts.

A Playwright script running inside the guest can connect to the already visible browser:

```python
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222")
    page = browser.contexts[0].new_page()
    page.goto("https://example.com")
    print(page.title())
    page.close()
```

Save the script in the guest and run it with `~/Workspace/.venv/bin/python`. The debugging port is guest-local, and the browser retains its sandbox. Do not expose the port to an untrusted network. The default setup includes browser tools, not private accounts, login cookies, publishing workflows, or schedules.

## Input and screenshots

Select a running machine in a visible workspace before sending input:

```sh
e2evm select research
e2evm input research click 0.5 0.5
e2evm input research key ctrl+l
e2evm input research text 'https://example.com'
e2evm input research key enter
e2evm screenshot research guest.png
e2evm screenshot research workspace.png --app
```

Click coordinates run from 0 to 1, with the origin at the top left of the guest display. They account for letterboxing. Virtual keyboard text is limited to the supported US keyboard characters; use guest commands for full Unicode data. Input targets the VM view rather than moving the host pointer or posting global events. Clipboard synchronization is off.

Screenshot files must not already exist. Native cached-view capture can omit GPU-backed guest content; check [validation](validation.md) for the observed rendering limits. Do not treat an empty or incomplete image as proof of guest state.

## Assistant

See [assistant connections](assistant.md) for provider setup, model discovery, full conversation input, key handling, and cancellation. A localhost model endpoint runs on the Mac, not in the guest. Codex must already be installed and signed in; Ollama must already have a running server and model. OpenAI-compatible support means the documented text Chat Completions protocol, not every vendor's API.

## Data and recovery

Default data is stored under `~/Library/Application Support/E2EVM`. Use a separate root for testing:

```sh
export E2EVM_HOME="$PWD/.runtime"
e2evm profile create test
```

Use the same root consistently for the desktop and CLI. `E2EVM_APP` can select a different native app path. The earlier `HARBORVM_HOME` and `HARBORVM_APP` names remain accepted; the E2EVM variables take precedence when both are set. Never open the same writable disk from two roots or VM applications at once. After a clean shutdown, back up the whole profile directory together with its boot artifacts and configuration.

| Symptom | What to check |
| --- | --- |
| Preparation fails | Rerun `prepare ID` if installation completed and only final file promotion was interrupted: a durable journal resumes matching prepared files. Incomplete installation without a journal, changed files, or unrelated existing disks are retained and refused. Inspect `profiles/ID/preparing`; use a fresh profile ID for a clean attempt rather than deleting a disk. |
| Kernel extraction fails | Verify the Mac has Node.js 22.15+ on `PATH`. |
| VM runs but no desktop appears | Inspect the profile's `mailbox/bootstrap.log` and `serial.log`. Package installation may still be active. |
| Initial setup was interrupted | Reboot the guest to retry unfinished setup, or run `sudo systemctl start --no-block harborvm-setup` through the guest command console. Inspect `bootstrap.log` for completion or failure. |
| Guest commands are unavailable | Check `status`, resume if paused, and wait for the worker heartbeat. The heartbeat runs independently of commands; inspect the guest worker if it remains unavailable. macOS guests require the connection setup and a signed-in guest account. |
| Profile or disk is locked | Use the app that owns it to shut it down cleanly. Do not remove lock files or launch another writer. |
| Codex is missing | Choose its executable in connection settings or use `profile assistant ID /absolute/path/to/codex`. |
| API model list or chat fails | Check the selected base URL/model and service availability. An app launched from Finder may not inherit a terminal's key environment variable. |

Unknown command outcomes are not retried automatically. Inspect the guest before repeating a command that might have changed data. Redact personal URLs, browser state, prompts, and credentials before attaching logs to an issue.
