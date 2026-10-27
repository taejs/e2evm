#!/bin/sh
# Run in the macOS guest's Terminal after completing Setup Assistant.
set -eu
[ "$(uname -s)" = Darwin ] && [ "$(/usr/sbin/sysctl -n kern.hv_vmm_present 2>/dev/null || true)" = 1 ] || {
  echo 'This installer runs only inside a macOS virtual machine, never on the host.' >&2; exit 1;
}
[ "$(id -u)" != 0 ] || { echo 'Run as your signed-in guest user, without sudo. The installer asks for sudo to mount shared folders and install its mount service.' >&2; exit 1; }
KIT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
[ -f "$KIT/worker.py" ] || { echo 'The guest connection kit is incomplete.' >&2; exit 1; }
option() {
  value=1
  if [ -f "$KIT/setup-options.env" ]; then
    parsed=$(/usr/bin/awk -F= -v key="$1" '$1 == key { print $2 }' "$KIT/setup-options.env")
    [ -z "$parsed" ] || value=$parsed
  fi
  case "$value" in 0|1) printf '%s' "$value" ;; *) echo 'Invalid guest setup option.' >&2; exit 1 ;; esac
}
SETUP_BROWSER=$(option SETUP_BROWSER)
SETUP_PYTHON=$(option SETUP_PYTHON)
SETUP_NODE=$(option SETUP_NODE)
SETUP_PLAYWRIGHT=$(option SETUP_PLAYWRIGHT)
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
PYTHON=''
# /usr/bin/python3 may only be an Xcode installer stub. Do not invoke it.
for candidate in /opt/homebrew/bin/python3 /usr/local/bin/python3 /Library/Frameworks/Python.framework/Versions/Current/bin/python3; do
  if [ -x "$candidate" ] && "$candidate" -c 'import sys; sys.exit(sys.version_info < (3, 9))' 2>/dev/null; then PYTHON=$candidate; break; fi
done
if [ -z "$PYTHON" ]; then
  if command -v brew >/dev/null 2>&1; then
    echo 'Installing Python for the guest command connection (required even when optional Python development tools are off).'
    brew install python
    PYTHON=$(command -v python3)
  else
    cat >&2 <<'HELP'
The guest command connection requires Python 3.9 or later inside this VM.
In this guest's browser, install Python from https://www.python.org/downloads/macos/.
For optional Chrome and Node.js installation, install Homebrew from https://brew.sh
inside this guest. Complete its requested developer-tools setup, then rerun:
  /bin/sh /Volumes/HarborKit/setup-macos.sh
Nothing needs to be installed on your host Mac.
HELP
    exit 1
  fi
fi
if { [ "$SETUP_BROWSER" = 1 ] && [ ! -d '/Applications/Google Chrome.app' ]; } || { [ "$SETUP_NODE" = 1 ] && ! command -v node >/dev/null 2>&1; }; then
  command -v brew >/dev/null 2>&1 || {
    echo 'The selected Chrome/Node.js options need Homebrew inside this VM. Install it from https://brew.sh in the guest, then rerun this installer.' >&2
    exit 1
  }
fi
[ "$SETUP_BROWSER" = 0 ] || [ -d '/Applications/Google Chrome.app' ] || brew install --cask google-chrome
if [ "$SETUP_NODE" = 1 ] && ! command -v node >/dev/null 2>&1; then brew install node; fi
mkdir -p "$HOME/Workspace" "$HOME/Library/Application Support/HarborVM" "$HOME/Library/LaunchAgents" "$HOME/.local/bin"
if [ "$SETUP_PYTHON" = 1 ] || [ "$SETUP_PLAYWRIGHT" = 1 ]; then
  "$PYTHON" -m venv "$HOME/Workspace/.venv"
fi
if [ "$SETUP_PLAYWRIGHT" = 1 ]; then
  "$HOME/Workspace/.venv/bin/python" -m pip install 'playwright==1.64.1'
fi
WORKER="$HOME/Library/Application Support/HarborVM/worker.py"
install -m 600 "$KIT/worker.py" "$WORKER"
if [ "$SETUP_BROWSER" = 1 ]; then
  install -m 644 "$KIT/welcome.html" "$HOME/Library/Application Support/HarborVM/welcome.html"
  cat > "$HOME/.local/bin/harbor-browser" <<'BROWSER'
#!/bin/sh
set -eu
[ "$(/usr/sbin/sysctl -n kern.hv_vmm_present 2>/dev/null || true)" = 1 ] || exit 1
mkdir -p "$HOME/Library/Application Support/HarborVM/Chrome"
if [ "$#" = 0 ]; then set -- "file://$HOME/Library/Application Support/HarborVM/welcome.html"; fi
nohup '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  --user-data-dir="$HOME/Library/Application Support/HarborVM/Chrome" \
  --remote-debugging-address=127.0.0.1 --remote-debugging-port=9222 \
  --no-first-run --no-default-browser-check --start-maximized "$@" \
  </dev/null >"$HOME/Library/Application Support/HarborVM/browser.log" 2>&1 &
BROWSER
  chmod 755 "$HOME/.local/bin/harbor-browser"
  cat > "$HOME/.local/bin/e2evm-browser" <<'BROWSER_ALIAS'
#!/bin/sh
exec "$HOME/.local/bin/harbor-browser" "$@"
BROWSER_ALIAS
  chmod 755 "$HOME/.local/bin/e2evm-browser"
fi
TASK_TEMP=$(mktemp -d)
trap 'rm -rf "$TASK_TEMP"' EXIT HUP INT TERM
printf '#!/bin/sh\nGUEST_UID=%s\nGUEST_GID=%s\n' "$(id -u)" "$(id -g)" > "$TASK_TEMP/mount-shares.sh"
cat >> "$TASK_TEMP/mount-shares.sh" <<'MOUNTS'
set -eu
[ "$(/usr/sbin/sysctl -n kern.hv_vmm_present 2>/dev/null || true)" = 1 ] || exit 1
mkdir -p /Volumes/HarborVM /Volumes/HarborKit
/sbin/mount | /usr/bin/grep -F ' on /Volumes/HarborVM (' >/dev/null || /sbin/mount_virtiofs -u "$GUEST_UID" -g "$GUEST_GID" HarborVM /Volumes/HarborVM
/sbin/mount | /usr/bin/grep -F ' on /Volumes/HarborKit (' >/dev/null || /sbin/mount_virtiofs -r -u "$GUEST_UID" -g "$GUEST_GID" HarborKit /Volumes/HarborKit
MOUNTS
"$PYTHON" - "$TASK_TEMP" "$PYTHON" "$WORKER" "$HOME" <<'PLISTS'
import pathlib, plistlib, sys
folder, python, worker, home = sys.argv[1:]
mount = {
    'Label': 'io.github.taejs.harborvm.mounts',
    'ProgramArguments': ['/bin/sh', '/Library/Application Support/HarborVM/mount-shares.sh'],
    'RunAtLoad': True, 'StartInterval': 30,
    'StandardOutPath': '/var/log/harborvm-mounts.log',
    'StandardErrorPath': '/var/log/harborvm-mounts.log',
}
agent = {
    'Label': 'io.github.taejs.harborvm.worker',
    'ProgramArguments': [python, worker, '/Volumes/HarborVM'],
    'RunAtLoad': True, 'KeepAlive': True, 'ThrottleInterval': 5,
    'WorkingDirectory': home,
    'EnvironmentVariables': {'HOME': home, 'PATH': home + '/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'},
    'StandardOutPath': home + '/Library/Application Support/HarborVM/worker.log',
    'StandardErrorPath': home + '/Library/Application Support/HarborVM/worker.log',
}
for name, data in [('mounts.plist', mount), ('worker.plist', agent)]:
    pathlib.Path(folder, name).write_bytes(plistlib.dumps(data))
PLISTS
sudo mkdir -p '/Library/Application Support/HarborVM'
sudo install -o root -g wheel -m 755 "$TASK_TEMP/mount-shares.sh" '/Library/Application Support/HarborVM/mount-shares.sh'
sudo install -o root -g wheel -m 644 "$TASK_TEMP/mounts.plist" /Library/LaunchDaemons/io.github.taejs.harborvm.mounts.plist
sudo /bin/sh '/Library/Application Support/HarborVM/mount-shares.sh'
[ -w /Volumes/HarborVM ] || {
  echo 'The guest mailbox is mounted with different account permissions. Close commands using this share, then in this guest run:' >&2
  echo '  sudo /sbin/umount /Volumes/HarborVM' >&2
  echo '  sudo /bin/sh "/Library/Application Support/HarborVM/mount-shares.sh"' >&2
  echo 'Then rerun this installer. Existing mounts are never forcibly replaced.' >&2
  exit 1
}
sudo launchctl bootout system/io.github.taejs.harborvm.mounts 2>/dev/null || true
sudo launchctl bootstrap system /Library/LaunchDaemons/io.github.taejs.harborvm.mounts.plist
install -m 600 "$TASK_TEMP/worker.plist" "$HOME/Library/LaunchAgents/io.github.taejs.harborvm.worker.plist"
launchctl bootout "gui/$(id -u)/io.github.taejs.harborvm.worker" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/io.github.taejs.harborvm.worker.plist"
if [ "$SETUP_BROWSER" = 1 ]; then "$HOME/.local/bin/e2evm-browser"; fi
printf 'Guest connection installed for %s. It starts when you sign in to this guest.\n' "$(id -un)"
printf 'macOS includes its own desktop; optional browser and development tools follow this profile’s selections.\n'
