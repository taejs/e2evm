---
name: guest-e2e
description: Run browser E2E tests in an E2EVM guest when the user requests E2EVM or its isolated browser workspace. Use the guest CLI and return assertions with actual screenshots. Does not select E2EVM for unrelated browser tasks.
---

# Guest E2E

Run the requested browser flow inside the selected VM while the user keeps using their Mac. The external coding agent drives `e2evm`; the app's assistant panel is chat-only and does not execute its replies.

## Establish the target

- Run `e2evm help`, `e2evm profile list`, then `e2evm status ID`. Use the requested profile and the same `E2EVM_HOME` as the desktop. If several profiles could match, resolve the target before changing one.
- Start a stopped target with `e2evm start ID`, or resume a paused target with `e2evm resume ID`, when doing so is within the requested test. Wait for `state: running` and `guest_online: true`. Poll sequentially with a bounded wait; if readiness fails, inspect the error and setup logs instead of launching another VM.
- `start` and `select` preserve the user's current app focus. Use `e2evm app` only when the user wants to bring the workspace window forward; it is not required for guest command tests.
- This browser recipe targets the supplied Ubuntu Linux guest on an Apple Silicon Mac. Inspect the profile's `guest_os` and `setup` choices. macOS guest onboarding is separate and is not covered here; do not apply the Linux package, worker, or browser paths to it.
- Guest readiness precedes browser setup. Check `e2evm exec ID -- test -f /var/lib/harborvm/setup-complete` and required tools. If first-boot setup is still running, let it finish before attempting another package installation.

## Keep execution in the guest

Use `e2evm exec ID -- PROGRAM ARG...` for argument-safe execution. For a shell pipeline or a working directory, use `e2evm run ID 'SHELL-LINE'`; keep guest substitutions inside single quotes so the host does not expand them.

Commands run as guest user `harbor`, with home `/home/harbor` and initial directory `/home/harbor`. Use `/home/harbor/Workspace` for project work. The worker has no interactive stdin; pass a short script as an argument or save it in the guest first. One request is limited to 64 KiB and 60 seconds, and output to 256 KiB. Prefer sequential commands; a queued request can expire while another command runs.

The local repository and host `localhost` are **not** automatically available inside the guest. There is no built-in project sync, `copy`, SSH login, port forwarding, or host-browser fallback. Read [environment.md](references/environment.md) when bringing in a project, reaching a development server, or installing dependencies. If the target is unavailable, explain what is missing instead of testing a different deployment silently.

## Use the browser the task needs

- For the visible persistent browser, use `e2evm exec ID -- e2evm-browser URL`. Execute Python Playwright with `/home/harbor/Workspace/.venv/bin/python` and connect to `http://127.0.0.1:9222` **from inside the guest**. This reuses guest Chrome and its saved login state.
- Open your own test tab, preserve unrelated tabs, and do not close the shared browser. A persistent profile may already contain accounts; use the test account and actions authorized for this task.
- Use guest headless execution if requested or appropriate to the suite. The default installation supplies Python Playwright and Google Chrome; it does not guarantee Playwright's downloaded Chromium, Firefox, WebKit, or an npm Playwright dependency. Install only what the chosen suite requires.
- Prefer semantic locators and assertions. For a visual-only control, the CLI also provides guest-scoped input and screenshots; inspect current help and the screenshot before choosing coordinates. Do not use global host input or open a host browser to get around a guest failure.

The [browser smoke recipe](references/browser-smoke.md) shows a complete assertion, a guest screenshot returned as a host artifact, and optional native workspace capture. Adapt its assertions to the actual test.

## Finish with evidence

Read the command's JSON result, including `ok`, `exit_code`, `timed_out`, and `truncated`. CLI failure or `ok: false` is a failure even if the output includes a success-looking line. When an outcome is unknown, inspect state before repeating a mutation; do not blindly replay a form submission or package install.

Every browser E2E report must include actual screenshots:

- Capture each requested flow's final state, and capture a failure state before closing its tab or cleaning up. This applies to headed and headless tests. Use pixels from the tested guest browser; do not substitute an illustration or an unrelated run.
- Transfer captures to fresh host artifact files, open them with the client's image viewer, and check that they show the relevant state clearly. A guest-only path, blank image, or screenshot command without an inspected image is incomplete evidence.
- Embed or attach the inspected images in the final reply using the client's supported image format. For Markdown clients, use `![description](/absolute/host/path/page.jpg)` with the actual absolute host path. Include a short caption explaining what each image proves. Listing paths or saying "screenshot saved" does not deliver the image.
- If capture, transfer, inspection, or image delivery fails, report the test outcome separately and mark screenshot evidence **blocked** with the reason. Do not claim the overall task is complete. Include a diagnostic capture if available, label it as such, and do not present it as a completed browser flow.

Alongside the images, return the target profile, URL and source revision, checks performed, passed/failed/blocked result, and relevant command output. A screenshot alone does not prove assertions passed. Remove only disposable state created by your test, and leave the user's VM available unless the task calls for shutdown. Testing authority does not authorize publishing, purchases, production data changes, or signing into unrelated accounts. Inspect captures for private data before sharing them outside the user's requested audience.
