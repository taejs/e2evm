---
name: e2evm-setup
description: Build or locate E2EVM, prepare or reuse a dedicated Linux test VM, and verify its browser connection when the user requests E2EVM setup. Use guest-e2e for application tests after setup.
---

# E2EVM setup

Make the requested isolated browser workspace ready for the user's coding agent.
The external agent runs the CLI; E2EVM's assistant panel is chat-only. No model
API key is needed for VM setup or the guest command transport.

## Locate the app and choose a target

Inspect `command -v e2evm`, `e2evm help`, and `e2evm profile list` before building
or creating anything. Use the user's existing checkout, app, profile ID, and
`E2EVM_HOME` when supplied. Inspect a candidate with `e2evm profile show ID` and
`e2evm status ID`. Reuse only a dedicated test guest; do not repurpose a production
VM or change another application's disk. Resolve an ambiguous target first.

This skill prepares Ubuntu Linux on **Apple Silicon, macOS 14+**. macOS guests
have separate manual onboarding; do not silently substitute Linux for a requested
macOS guest or apply Linux worker paths to it.

If a source build is needed, locate the E2EVM checkout containing `Cargo.toml`
(package `e2evm`) and `scripts/build-macos.sh`; do not assume the current directory
is that checkout. Read its `docs/install.md` if present. When the skill was copied
away from the repository, use the self-contained steps below and the
[published installation guide](https://github.com/taejs/e2evm/blob/feat/vm-workspace/docs/install.md).
Source is [taejs/e2evm](https://github.com/taejs/e2evm). If no checkout exists, clone
it into a new workspace location within the user's setup request. Do not overwrite
an existing clone.

Host prerequisites: Rust/Cargo 1.85+, Xcode Command Line Tools with `xcrun swiftc`,
Node.js 22.15+ with `node:zlib.zstdDecompressSync`, and Python 3 for the smoke
helper. macOS supplies `curl`, `tar`, `gzip`, `hdiutil`, and `codesign`. Inspect
installed versions first. Missing host build tools are a setup prerequisite;
install them only within the user's authorized host setup. Guest application
dependencies belong inside the guest. Do not invent a Homebrew cask, notarized
download, or automatic updater for this source release.

From the verified repository root:

```sh
sh scripts/build-macos.sh
export PATH="$PWD/dist/E2EVM.app/Contents/MacOS:$PATH"
e2evm help
```

The build produces and ad-hoc signs `dist/E2EVM.app`. Shell `PATH` changes above
are temporary; do not rewrite the user's shell configuration unless requested.
`start` launches the companion app without taking the user's current app focus.
Use `e2evm app` only when the user wants its window brought forward.

## Prepare only an unprepared dedicated profile

For a new profile, choose an unused ID. This example uses 2 CPUs, 4 GiB RAM,
and the default 24 GiB sparse disk. Check available host resources and the user's
budget; existing profile resources remain unchanged. Preparation downloads Ubuntu
artifacts, verifies their checksums, and writes a fresh guest disk. First boot
also needs internet access and disk space for the selected packages.

```sh
e2evm profile create e2e --os linux --name "E2E workspace" --cpus 2 --memory 4096
e2evm prepare e2e
e2evm start e2e
e2evm status e2e
```

Substitute the selected ID. Skip creation for an existing profile and skip
`prepare` when `ready` is true. Resume a paused guest with `e2evm resume ID`.
Do not stop a running guest to repeat setup. If preparation fails or its outcome
is unknown, inspect the profile and error before retrying. Do not delete its disk,
staging directory, or recovery journal to force a retry.

Setup options default to enabled: `desktop,browser,python,node,playwright`.
Creation accepts `--without node`, a comma-separated list, or `--without all`.
Disabling desktop also disables browser and Playwright; disabling browser or
Python also disables Playwright. Preserve the user's disabled choices. The
browser smoke below needs desktop, browser, Python, and Playwright; if one was
deliberately omitted, explain that constraint and report only the checks possible.

## Verify readiness and deliver evidence

Poll `status` sequentially with a task deadline and a sensible interval. Require
`state: running` and `guest_online: true`, then check:

```sh
e2evm exec e2e -- test -f /var/lib/harborvm/setup-complete
e2evm exec e2e -- /home/harbor/Workspace/.venv/bin/python -m playwright --version
e2evm exec e2e -- /bin/sh -lc '
if command -v e2evm-browser >/dev/null 2>&1; then
  exec e2evm-browser
elif [ -x /usr/local/bin/harbor-browser ]; then
  exec /usr/local/bin/harbor-browser
else
  printf "%s\n" "No guest browser launcher found; inspect setup choices and logs" >&2
  exit 1
fi'
```

New guests provide `e2evm-browser`. Older prepared guests may have only
`/usr/local/bin/harbor-browser`; the command above checks inside the selected
guest and reuses that existing launcher if the new alias is absent. If Chrome
is already available on guest CDP port 9222, the helper can attach directly.
A missing new alias alone is not a reason to reinstall packages, rerun setup,
or migrate the VM. If neither launcher exists, inspect setup choices and logs;
do not fall back to a host browser.

The worker can be online while first-boot packages are still installing. Wait
for completion before starting another install. If setup stalls, inspect
`e2evm exec ID -- tail -n 80 /mnt/harbor/bootstrap.log` and
`e2evm exec ID -- systemctl status harborvm-setup.service --no-pager`.
For a browser connection failure, inspect
`/home/harbor/.local/share/harborvm/browser.log` inside the guest; retry connection
within a bounded wait, without launching a second browser profile.

Run the bundled host helper using the actual installed skill path:

```sh
python3 /absolute/path/to/e2evm-setup/scripts/verify-browser.py --profile e2e
```

`--cli /absolute/path/to/e2evm` handles a CLI not on PATH. `--output-dir NEW_DIR`
chooses a fresh host evidence directory; existing directories are refused. The
helper runs Python Playwright **inside the guest**, opens its own tab with a local
HTML fixture, asserts its title and heading, returns an actual screenshot, and
closes only that tab. It preserves unrelated tabs and never falls back to a host
browser. A fixture pass verifies integration, not the user's application or
internet access. Individual guest requests have a 60-second limit, no interactive
stdin, a 64 KiB request limit, and 256 KiB output limit.

Read the returned JSON and exit status. Open `host_screenshot` with the client's
image viewer, inspect it, and attach or embed the actual image in the reply with
the two assertion results. Paths alone do not deliver evidence. If capture,
inspection, or attachment fails, report that blocker separately and do not claim
complete setup verification. Leave the VM running unless shutdown was requested.

Hand off the exact CLI path, profile ID, custom data root if any, enabled setup
choices, and any remaining dependencies. Use `guest-e2e` if installed for the
requested application flow. Source files and host localhost are not automatically
shared with the VM; project transfer and app installation are separate work.
