#!/usr/bin/env python3
"""Verify the selected Linux guest browser; save actual pixels for agent inspection."""
import argparse
import base64
import json
from pathlib import Path
import subprocess
import sys
import tempfile

GUEST_SCRIPT = r'''
import base64, json, sys
from playwright.sync_api import sync_playwright, expect

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp("http://127.0.0.1:9222", timeout=10000)
    page = browser.contexts[0].new_page()
    page.bring_to_front()
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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", required=True, help="existing prepared Linux profile")
    parser.add_argument("--cli", default="e2evm", help="CLI name on PATH or absolute executable path")
    parser.add_argument("--output-dir", type=Path, help="new host artifact directory; never overwritten")
    args = parser.parse_args(argv)
    try:
        if args.output_dir is None:
            artifacts = Path(tempfile.mkdtemp(prefix="e2evm-setup-smoke-"))
        else:
            artifacts = args.output_dir.expanduser().absolute()
            artifacts.mkdir(mode=0o700, parents=True, exist_ok=False)
    except OSError as error:
        parser.exit(1, f"Cannot create fresh artifact directory: {error}\n")
    evidence = {"status": "blocked", "profile": args.profile, "artifacts": str(artifacts)}
    try:
        result = subprocess.run(
            [args.cli, "exec", args.profile, "--",
             "/home/harbor/Workspace/.venv/bin/python", "-c", GUEST_SCRIPT,
             "data:text/html,<title>Guest E2E smoke</title><h1>Guest E2E smoke</h1>",
             "Guest E2E smoke", "Guest E2E smoke"],
            capture_output=True, text=True, timeout=90,
        )
        (artifacts / "reply.json").write_text(result.stdout, encoding="utf-8")
        (artifacts / "cli-stderr.txt").write_text(result.stderr, encoding="utf-8")
        reply = json.loads(result.stdout)
        if (result.returncode or reply.get("ok") is not True or reply.get("exit_code") != 0
                or reply.get("timed_out") or reply.get("truncated")):
            raise ValueError("Guest command failed or evidence was truncated; inspect reply.json")
        payload = json.loads(reply["output"])
        encoded = payload.pop("screenshot_base64", None)
        evidence.update(payload)
        if not encoded:
            raise ValueError("No screenshot returned: " + payload.get("capture_error", "unknown capture failure"))
        shot = base64.b64decode(encoded, validate=True)
        if not shot or len(shot) > 160000 or not shot.startswith(b"\xff\xd8"):
            raise ValueError("Screenshot is empty, oversized, or not JPEG data")
        image = artifacts / "page.jpg"
        image.write_bytes(shot)
        evidence["host_screenshot"] = str(image)
        evidence["delivery"] = "Agent must inspect and attach this image; saving is not delivery"
        if evidence.get("status") != "passed" or evidence.get("assertions") != 2:
            raise ValueError("Browser assertions failed; inspect and attach the diagnostic screenshot")
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.TimeoutExpired) as error:
        evidence.setdefault("error", str(error))
        if evidence.get("status") == "passed":
            evidence["status"] = "blocked"
    (artifacts / "checks.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
    print(json.dumps(evidence, indent=2))
    return 0 if evidence.get("status") == "passed" and not evidence.get("error") else 1


if __name__ == "__main__":
    raise SystemExit(main())
