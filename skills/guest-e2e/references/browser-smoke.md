# Assert a page in the guest browser

Run these snippets in the **host terminal**, with the bundled `e2evm` CLI on `PATH`. Replace profile `e2e` with the selected guest, which must have completed browser setup. The default local HTML fixture verifies the browser connection, assertions, and evidence transfer without depending on a public website. To test an application, replace its URL, title, and heading with the requested target; passing this fixture does not test that application or network access.

```sh
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

New guests provide `e2evm-browser`; older prepared guests may only have `/usr/local/bin/harbor-browser`. The snippet checks inside the selected guest and reuses the old launcher if the new alias is absent. If Chrome already exposes guest CDP port 9222, skip launching and attach directly. Do not reinstall packages, rerun setup, or migrate the VM just to add the new alias. If neither launcher exists, inspect setup choices and logs instead of using a host browser.

Chrome may take a moment to expose its debugging endpoint. If connection is refused, check guest setup and browser logs, then retry the connection within a bounded wait. Do not launch a second browser profile to conceal a startup failure.

This host Python snippet invokes a short Playwright script **in the guest**, asserts its title and heading, and returns a small screenshot through command output even when an assertion fails. It creates a fresh host artifact directory, so it does not overwrite earlier evidence. It needs only host Python's standard library, not host Playwright. Saving the image is followed by agent inspection and attachment; this command cannot display it in the user's chat by itself.

```python
import base64
import json
from pathlib import Path
import subprocess
import tempfile

profile = "e2e"
url = "data:text/html,<title>Guest E2E smoke</title><h1>Guest E2E smoke</h1>"
expected_title = "Guest E2E smoke"
expected_heading = "Guest E2E smoke"
artifacts = Path(tempfile.mkdtemp(prefix="guest-e2e-"))

guest_script = r'''
import base64, json, sys
from playwright.sync_api import sync_playwright, expect

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222", timeout=10000)
    page = browser.contexts[0].new_page()
    evidence = {"status": "failed", "assertions": 0}
    try:
        page.set_default_timeout(8000)
        page.goto(sys.argv[1], wait_until="domcontentloaded", timeout=15000)
        expect(page).to_have_title(sys.argv[2])
        evidence["assertions"] += 1
        expect(page.get_by_role("heading", name=sys.argv[3], exact=True)).to_be_visible()
        evidence["assertions"] += 1
        evidence["status"] = "passed"
    except Exception as error:
        evidence["error"] = f"{type(error).__name__}: {error}"[:2000]
    finally:
        try:
            evidence.update(title=page.title(), url=page.url)
            shot = page.screenshot(type="jpeg", quality=35, timeout=8000)
            if not shot or len(shot) > 160000:
                raise RuntimeError("Screenshot empty or over reply budget; capture a smaller relevant region")
            evidence["screenshot_base64"] = base64.b64encode(shot).decode()
        except Exception as error:
            evidence["capture_error"] = f"{type(error).__name__}: {error}"[:1000]
        try:
            page.close()
        except Exception as error:
            evidence["cleanup_error"] = f"{type(error).__name__}: {error}"[:1000]
        print(json.dumps(evidence))
'''

result = subprocess.run(
    ["e2evm", "exec", profile, "--",
     "/home/harbor/Workspace/.venv/bin/python", "-c", guest_script,
     url, expected_title, expected_heading],
    capture_output=True, text=True,
)
(artifacts / "reply.json").write_text(result.stdout, encoding="utf-8")
(artifacts / "cli-stderr.txt").write_text(result.stderr, encoding="utf-8")
if result.returncode:
    raise SystemExit(f"Guest smoke failed; inspect {artifacts}")
reply = json.loads(result.stdout)
if not reply.get("ok") or reply.get("exit_code") != 0 or reply.get("timed_out") or reply.get("truncated"):
    raise SystemExit(f"Incomplete guest evidence; inspect {artifacts}")
evidence = json.loads(reply["output"])
encoded = evidence.pop("screenshot_base64", None)
image = None
if encoded:
    try:
        shot = base64.b64decode(encoded, validate=True)
        if not shot:
            raise ValueError("Screenshot is empty")
        image = artifacts / "page.jpg"
        image.write_bytes(shot)
    except (ValueError, OSError) as error:
        evidence["capture_error"] = str(error)
        image = None
evidence.update(profile=profile, host_screenshot=str(image) if image else None)
(artifacts / "checks.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
print(json.dumps(evidence, indent=2))
if image is None:
    raise SystemExit(f"Screenshot evidence blocked; inspect {artifacts}. Do not report completion.")
if evidence.get("status") != "passed" or evidence.get("assertions") != 2:
    raise SystemExit(f"Guest smoke failed; inspect and attach {image}")
print(f"Passed 2 assertions. Open and inspect {image}, then attach it in the final reply.")
```

The screenshot is captured before the test tab closes, on both success and assertion failure. It reflects the guest page at the time of that outcome. Failed assertions still produce `page.jpg` and `checks.json` before the host command exits unsuccessfully. A browser connection or capture failure may prevent an image; report that evidence blocker without claiming completion. The script leaves the existing browser and unrelated tabs open. For an authenticated flow, use an authorized test account and adapt assertions and cleanup to the task.

Open the returned **host** image with the client's image viewer. Check that it is readable and shows the relevant result, then embed or attach the actual image in the final reply with a caption and the assertion result. In a Markdown client, use `![Guest test result](/absolute/host/path/page.jpg)` with the returned path. Returning a path alone or leaving the image only inside the guest does not satisfy delivery. If the client cannot display an attachment, report that limitation as incomplete delivery.

Attaching over CDP reuses Chrome's default context. It has fewer guarantees than a browser launched by Playwright; if the suite needs unsupported functionality, run its own browser inside the guest. See [Playwright's CDP documentation](https://playwright.dev/python/docs/api/class-browsertype#browser-type-connect-over-cdp).

For a native view capture, select the running guest while the desired page is visible and choose a **new** filename:

```sh
e2evm select e2e
e2evm screenshot e2e e2e-guest.png
e2evm screenshot e2e e2e-workspace.png --app
```

These paths are on the host. Inspect native captures for missing or blank content; a capture error or incomplete render is not proof of application behavior. A path passed to Playwright's `page.screenshot(path=...)` instead lives inside the guest unless it is explicitly transferred.

## Headless alternative

For a fresh headless session inside the guest, Python Playwright can launch the installed Chrome with `p.chromium.launch(channel="chrome", headless=True)` and create a new context. This does not reuse the persistent desktop session's login state. Close browsers you launch yourself. A project using npm Playwright may require its own package and browsers; discover the project's setup instead of assuming the Python environment satisfies it.
