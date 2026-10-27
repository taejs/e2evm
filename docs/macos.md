# macOS guests

HarborVM can prepare a fresh macOS VM using Apple's Virtualization framework and show its desktop in the workspace. Installation, Apple's first-run setup, and the guest command connection are separate steps. A visible macOS desktop does **not** mean `exec` is ready.

This requires an Apple Silicon Mac running macOS 14 or later. Apple decides which restore images and virtual hardware configurations your Mac supports. The helper checks the selected image and validates the VM before installing. See Apple's [macOS VM documentation](https://developer.apple.com/documentation/virtualization/running-macos-in-a-virtual-machine-on-apple-silicon).

## Create and install

Build the app using the [README](../README.md), then make its bundled CLI available in the host Terminal:

```sh
export PATH="$PWD/dist/HarborVM.app/Contents/MacOS:$PATH"
harborvm profile create macos --os macos --name "macOS testing"
harborvm prepare macos
```

The default macOS profile has 2 CPUs, 4096 MiB RAM, and a 64 GiB sparse disk. Use `--cpus`, `--memory`, and `--disk` at creation to request more resources. A newer restore image can require more than these defaults; preparation reports its minimums. A sparse disk grows as the guest writes data, so leave enough free host storage for the operating system, development tools, and projects.

Without an image argument, preparation asks Apple for the latest supported restore image and downloads it directly over HTTPS. This is a large download followed by a full operating-system installation. Progress appears in the preparation output.

Alternatively, use an Apple IPSW already downloaded to this Mac. Supply an absolute path:

```sh
harborvm prepare macos --restore-image /absolute/path/to/macOS.ipsw
```

Choose one preparation method for a new profile. This creates a fresh disk; it does not import another application's VM, accounts, or writable disk. After installation finishes, a durable journal records the prepared files before they move into place. If that final step is interrupted, rerun `harborvm prepare macos`: matching files resume without reinstalling macOS or requiring the original IPSW. An incomplete installation without that journal, changed files, or an unrelated existing disk is retained and refused. Inspect the error and `profiles/macos/preparing` under the data root; use a new profile ID for a fresh attempt rather than deleting a disk.

## Choose the optional tools

All optional setup components are selected by default. The creation sheet lets you turn them off. The CLI supports comma-separated components:

```sh
harborvm profile create macos-minimal --os macos --without all
harborvm profile create macos-web --os macos --without node
```

| Component | macOS behavior |
| --- | --- |
| Desktop | macOS already includes its desktop and Safari. Turning this setup option off does not remove Apple's bundled applications. |
| Browser | Installs Chrome if needed and provides `harbor-browser`, with a separate persistent Chrome profile. |
| Python | Creates `~/Workspace/.venv` for development. The command connection still requires a Python runtime when this option is off. |
| Node.js | Installs Node.js through an existing guest Homebrew installation if Node is absent. |
| Playwright | Installs Python Playwright in the development environment. It uses the selected Chrome setup. |

Disabling desktop also disables the bundled browser and Playwright setup. Disabling browser or Python development tools disables Playwright. Disabling Node.js does not disable Python Playwright. These choices control initial setup; they do not uninstall tools from an existing guest.

## Complete Apple's first-run setup

From the host Terminal:

```sh
harborvm app
harborvm start macos
```

Select the macOS machine in the workspace. Complete Apple's Setup Assistant inside that guest window, review its terms, and create your own guest account. HarborVM does not supply an account, accept Apple's terms for you, or copy your host credentials. Keep the guest signed in when using its graphical browser and command connection.

At this point the VM can be running while `guest_online` remains false. That is expected until the next step succeeds.

## Connect the guest command tools

The following steps run **inside the macOS guest's Terminal**, not in the host Terminal or HarborVM's command console. The console cannot run commands until this connection is installed.

The command worker needs Python 3.9 or later. The bootstrap reuses Python installed through Homebrew or the official Python installer. If Homebrew already exists but Python is missing, it installs Python as a core connection dependency. It deliberately avoids `/usr/bin/python3`, which can trigger Apple's developer-tools installation prompt.

For the default Chrome and Node.js options, install [Homebrew](https://brew.sh) inside the guest and finish any developer-tools setup it requests. If those options are off, a standalone installation from [Python's macOS downloads](https://www.python.org/downloads/macos/) is sufficient. These prerequisites belong inside the VM; the bootstrap does not install them on the host.

Mount the read-only kit using your guest account's ownership mapping:

```sh
sudo mkdir -p /Volumes/HarborKit
sudo /sbin/mount_virtiofs -r -u "$(id -u)" -g "$(id -g)" HarborKit /Volumes/HarborKit
/bin/sh /Volumes/HarborKit/setup-macos.sh
```

If the kit is already mounted, skip the mount command. Run the setup script as your signed-in guest user, without `sudo` in front of it. It checks that it is inside a macOS VM before changing anything. It asks for the **guest** administrator password only for the shared-folder mount service.

The bootstrap honors this profile's saved setup choices, installs the worker under your guest home, mounts the command mailbox at `/Volumes/HarborVM`, and starts a user LaunchAgent. A root LaunchDaemon remounts the two shared folders after reboot, using the account's recorded UID and GID. The kit remains read-only. The worker itself runs as your ordinary guest account and starts again when that account signs in.

From the **host** Terminal, verify the connection:

```sh
harborvm status macos
harborvm exec macos -- uname -s
harborvm exec macos -- whoami
```

Expect `guest_online: true`, `Darwin`, and your guest account name. The command worker does not use the Linux guest's `harbor` account. See [usage](usage.md) for command timeouts, quoting, and output limits.

## Run a visible browser test

If Browser and Playwright were selected, launch the guest's Chrome session from the host:

```sh
harborvm exec macos -- harbor-browser https://example.com
harborvm run macos 'cd ~/Workspace && .venv/bin/python -m playwright --version'
```

A Playwright script running **inside the guest** can connect to that browser at `http://127.0.0.1:9222`. This address refers to the guest, not the host. Keep the debugging port guest-local. Chrome's profile is stored at `~/Library/Application Support/HarborVM/Chrome` inside the VM and survives a normal guest restart.

The guest must be signed in for its desktop browser. After reboot, launch `harbor-browser` again when needed; the worker restarts at login, but the bootstrap does not install a browser auto-launch agent. Safari is part of macOS, but this bootstrap's ready-to-use automation path is Chrome with Playwright.

## Troubleshooting and recovery

| Symptom | Action |
| --- | --- |
| Restore image is unsupported | Use an image supported by this host, or omit `--restore-image` so Apple selects one. |
| Restore image needs more resources | Create a fresh profile with the CPU and memory minimums shown in the error. |
| macOS is visible, but guest commands are unavailable | Sign in to the guest and finish its bootstrap. Check `~/Library/Application Support/HarborVM/worker.log` in the guest. |
| Python or Homebrew is missing | Install the named prerequisite inside the guest, then rerun the setup script. |
| Shares do not return after reboot | Inspect `/var/log/harborvm-mounts.log` inside the guest. The mount service retries every 30 seconds; the user worker retries while unavailable. |
| Mailbox is mounted with another account's permissions | Close programs using `/Volumes/HarborVM`, then follow the bootstrap's unmount/remount instructions. It never forcibly replaces an existing mount. Use the account chosen for this guest connection. |
| A tool was disabled | Install it inside the guest using its normal package manager, or create a new profile with the desired options. Disabled setup does not remove macOS's built-in desktop or Safari. |

Use `harborvm stop macos` for a graceful shutdown. Wait until status reports stopped before backing up or moving a profile. Keep `disk.img`, `hardware-model.bin`, `machine-identifier.bin`, and `auxiliary-storage` together; the disk alone is not a complete macOS VM. Do not open a writable disk in two VM applications at once.

Actual installation and runtime verification are recorded separately in [validation](validation.md). A successful build or configuration check alone is not evidence that macOS setup, login, and guest commands have all completed.

## Apple's licensing terms

This project distributes its own source code and guest connection tools, not macOS restore images, installed disks, firmware, or Apple accounts. Users obtain their own restore image from Apple and complete their own setup.

Section 2B(iii) of Apple's [macOS 27 license](https://www.apple.com/legal/sla/docs/macOS27.pdf) and [macOS Tahoe 26 license](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf) permits up to two additional virtual copies or instances on each qualifying Apple-branded computer you own or control that already runs the Apple software, subject to the applicable license. Listed purposes include software development and testing during development; those purposes are not limited to personal noncommercial projects.

Section 2J restricts non-Apple hardware and redistribution, with stated exceptions elsewhere in the agreement. Service-bureau and similar uses have additional restrictions; developer-service leasing has separate conditions in section 3. See the [macOS 27 agreement](https://www.apple.com/legal/sla/docs/macOS27.pdf) or the [Tahoe agreement](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf) for the complete terms.

The intended use here is local development and testing on your own compatible Mac. That is a product scope, not a blanket legal guarantee. Check the license that applies to the actual macOS version and acquisition method you use, including any separate organizational agreement.
