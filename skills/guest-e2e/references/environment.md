# Guest environment and dependencies

## Reach the application

| Where the app runs | What the guest can use |
| --- | --- |
| Public or authorized staging URL | Open that URL from the guest. Confirm it is the intended deployment. |
| Inside this guest | Use guest `http://127.0.0.1:PORT`; launch the development server inside the guest. |
| On the Mac | Host `localhost` is a different loopback interface. There is no automatic forwarding. Use an explicitly configured address reachable from the guest, or run the project inside the guest. Do not broaden a host server's network binding without authorization. |

Guest networking uses NAT. Do not assume a fixed host gateway IP or expose Chrome's debugging port beyond guest loopback. Assistant provider endpoints configured in the native app run from the **host**; they are separate from guest browser URLs.

## Bring in source

The guest shares its command mailbox with the host, not the user's entire working tree. There is no project-sync command. Choose an existing approved way to get the source:

- Clone a public or otherwise authorized repository **inside the guest**, then check out the intended revision. A fresh clone does not contain uncommitted host edits. Do not copy host credentials, browser cookies, SSH keys, or an entire home directory to make a clone work.
- For a small script, `harborvm exec ID -- python3 -c 'SCRIPT'` avoids transfer. To save a short text file, pass its contents as a separate argument to a guest Python writer. The serialized request must remain below 64 KiB; this is not a bulk project-transfer feature.
- If local unpublished changes or a large project need transfer, identify the approved transfer method with the user. Report this as a setup dependency rather than claiming the remote revision proves the local change.

Inspect the project instructions and lockfile inside the guest before installing or running its suite.

## Check before installing

These are **host terminal commands** that inspect the guest. Substitute the selected profile for `e2e`:

```sh
harborvm exec e2e -- /bin/sh -lc 'command -v google-chrome; command -v node; command -v npm; command -v git'
harborvm exec e2e -- /home/harbor/Workspace/.venv/bin/python -m playwright --version
harborvm exec e2e -- test -f /var/lib/harborvm/setup-complete
```

The first command lists available executables; its exit status alone does not establish that all four exist. Git is not part of the default desktop package list. The supplied Python virtual environment contains Playwright. Use the project's own environment when it declares a different version. Prefer its locked npm install over unrelated global installs; use a guest virtual environment for Python dependencies. Respect setup options the user deliberately disabled.

Do dependency work inside the guest through its existing tools: `sudo apt-get` for Ubuntu system packages, project-local `npm ci` for a committed lockfile, and a virtual environment's `python -m pip` for Python. Guest `sudo` is available without a password. Do not install tools on the Mac merely because they are missing in the guest.

## Jobs longer than 60 seconds

The CLI has no timeout override or interactive terminal. Package installation and full E2E suites often exceed its 60-second limit. Launch a named job with guest systemd, then inspect its state and journal in separate requests. A successful launch is not a successful job.

For example, after confirming Git is needed and no other package installation is active, choose a fresh unit name and run:

```sh
harborvm exec e2e -- sudo systemd-run --no-block --expand-environment=no --unit=harbor-e2e-deps-01 --property=Type=oneshot --property=RemainAfterExit=yes /bin/sh -lc 'apt-get update && apt-get install -y git'
harborvm exec e2e -- systemctl show harbor-e2e-deps-01.service --property=ActiveState,SubState,Result,ExecMainCode,ExecMainStatus
harborvm exec e2e -- sudo journalctl -u harbor-e2e-deps-01.service --no-pager -n 80
```

`--no-block` returns after queuing the job instead of waiting for the oneshot to finish. `--expand-environment=no` leaves dollar expansion to the guest shell rather than systemd. See the [systemd-run manual](https://www.freedesktop.org/software/systemd/man/latest/systemd-run.html).

`activating` means still running. With this oneshot configuration, `active` / `exited`, `Result=success`, and `ExecMainStatus=0` indicate completion. `failed`, a nonzero status, or missing completion evidence must be investigated. Do not delete package-manager lock files. Reuse the same unit name to inspect an uncertain launch rather than start a duplicate.

For a project test or install, add `--uid=harbor --gid=harbor`, `--setenv=HOME=/home/harbor`, `--setenv=DISPLAY=:0`, and `--working-directory=/home/harbor/Workspace/PROJECT` to `sudo systemd-run`. Pass the actual project command and arguments after its options. Keep project package scripts running as `harbor`, not root. Choose a new unit name for each new job, retain exit evidence, and stop only jobs created by this task when cleanup is needed. Poll with a sensible delay and a task deadline; do not leave a failing job in an unlimited retry loop.
