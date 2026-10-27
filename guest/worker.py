#!/usr/bin/env python3
"""E2EVM guest-only command worker. Never runs on the host."""
import argparse
import json
import os
import platform
from pathlib import Path
import re
import signal
import subprocess
import time
import threading
import uuid

MAX_OUTPUT = 256 * 1024
POST_TIMEOUT_DRAIN_SECONDS = 0.5
ID = re.compile(r"^[a-z0-9-]{1,100}$")


def read_regular(path, limit=65536):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        import stat
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError("Not a regular request")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(limit + 1)
        if len(data) > limit:
            raise ValueError("Request too large")
        return json.loads(data)
    finally:
        os.close(fd)


def write_reply(path, value):
    temp = path.with_name(path.name + "." + uuid.uuid4().hex + ".tmp")
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(value, stream, ensure_ascii=False)
    os.replace(temp, path)


def execute(request):
    if request.get("version") != 1:
        raise ValueError("Unsupported protocol version")
    if request.get("expires_at", 0) < time.time():
        raise ValueError("Request expired; command was not executed")
    argv = request.get("argv")
    if not isinstance(argv, list) or not argv or len(argv) > 256 or not all(isinstance(a, str) and "\0" not in a for a in argv):
        raise ValueError("argv must be a nonempty array of strings")
    timeout = max(1, min(float(request.get("timeout", 60)), 600))
    command = argv
    macos = platform.system() == "Darwin"
    home = str(Path.home()) if macos else "/home/harbor"
    if os.geteuid() == 0:
        if macos:
            raise ValueError("Run the macOS guest worker as the signed-in guest user")
        command = ["/usr/sbin/runuser", "-u", "harbor", "--", *argv]
    import selectors
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, start_new_session=True,
                               env={**os.environ, "DISPLAY": ":0", "HOME": home}, cwd=home)
    output = bytearray()
    truncated = False
    timed_out = False
    deadline = time.monotonic() + timeout
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    try:
        while selector.get_map() or process.poll() is None:
            if time.monotonic() > deadline:
                if timed_out:
                    # A detached descendant can retain stdout after the original
                    # process group is killed. Do not wait indefinitely for EOF.
                    if selector.get_map():
                        truncated = True
                        selector.unregister(process.stdout)
                        process.stdout.close()
                    break
                timed_out = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                deadline = time.monotonic() + POST_TIMEOUT_DRAIN_SECONDS
            for key, _ in selector.select(0.1):
                data = os.read(key.fd, 8192)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                remaining = max(0, MAX_OUTPUT - len(output))
                output.extend(data[:remaining])
                truncated |= len(data) > remaining
        code = process.wait()
    finally:
        selector.close()
        process.stdout.close()
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
    return {"ok": code == 0 and not timed_out, "exit_code": code,
            "output": output.decode("utf-8", errors="replace"), "truncated": truncated,
            "timed_out": timed_out, "target": "guest"}


def keep_heartbeat(mailbox):
    while True:
        try:
            write_reply(mailbox / "heartbeat.json", {"version": 1, "timestamp": time.time(), "target": "guest"})
        except OSError:
            pass
        time.sleep(1)


def serve(mailbox):
    for name in ("commands", "replies"):
        (mailbox / name).mkdir(exist_ok=True)
    threading.Thread(target=keep_heartbeat, args=(mailbox,), daemon=True).start()
    while True:
        for path in sorted((mailbox / "commands").glob("*.json")):
            request_id = path.stem
            if not ID.fullmatch(request_id):
                continue
            reply = mailbox / "replies" / path.name
            try:
                request = read_regular(path)
                path.unlink()  # Consume before execution: never replay on worker restart.
                if reply.exists():
                    continue
                if request.get("id") != request_id:
                    raise ValueError("Request ID mismatch")
                result = execute(request)
            except Exception as error:
                result = {"ok": False, "error": str(error), "target": "guest"}
                try:
                    path.unlink()
                except FileNotFoundError:
                    pass
            result["id"] = request_id
            try:
                write_reply(reply, result)
            except FileExistsError:
                pass
        time.sleep(0.2)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mailbox", type=Path)
    serve(parser.parse_args().mailbox)
