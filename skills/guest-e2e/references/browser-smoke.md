# Assert a page in the guest browser

Run these snippets in the **host terminal**, with the bundled `e2evm` CLI on `PATH`. Replace profile `e2e` with the selected guest, which must have completed browser setup. The default local HTML fixture verifies the browser connection, assertions, and evidence transfer without depending on a public website. To test an application, replace its URL, title, and heading with the requested target; passing this fixture does not test that application or network access.

```sh
e2evm exec e2e -- e2evm-browser
```

Chrome may take a moment to expose its debugging endpoint. If connection is refused, check guest setup and browser logs, then retry the connection within a bounded wait. Do not launch a second browser profile to conceal a startup failure.

This host Python snippet invokes a short Playwright script **in the guest**, asserts its title and heading, and returns a small screenshot through command output. It creates a fresh host artifact directory, so it does not overwrite earlier evidence. It needs only host Python's standard library, not host Playwright.

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
    try:
        page.set_default_timeout(8000)
        page.goto(sys.argv[1], wait_until="domcontentloaded", timeout=15000)
        expect(page).to_have_title(sys.argv[2])
        expect(page.get_by_role("heading", name=sys.argv[3], exact=True)).to_be_visible()
        shot = page.screenshot(type="jpeg", quality=35, timeout=8000)
        if len(shot) > 160000:
            raise RuntimeError("Screenshot exceeds reply budget; capture a smaller region")
        print(json.dumps({"title": page.title(), "url": page.url,
                          "assertions": 2, "screenshot_base64": base64.b64encode(shot).decode()}))
    finally:
        page.close()
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
if not reply.get("ok") or reply.get("timed_out") or reply.get("truncated"):
    raise SystemExit(f"Incomplete guest evidence; inspect {artifacts}")
evidence = json.loads(reply["output"])
(artifacts / "page.jpg").write_bytes(base64.b64decode(evidence.pop("screenshot_base64"), validate=True))
(artifacts / "checks.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
print(f"Passed {evidence['assertions']} assertions. Host artifacts: {artifacts}")
```

The screenshot is captured before the test tab closes. It reflects the guest page at the time of the assertions. The script leaves the existing browser and unrelated tabs open. For an authenticated flow, use an authorized test account and adapt assertions and cleanup to the task.

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
