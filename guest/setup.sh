#!/bin/sh
set -eu
[ "$(uname -s)" = Linux ] && [ "$(id -u)" = 0 ] || { echo 'Run only inside the Linux guest as root.' >&2; exit 1; }
mkdir -p /mnt/harbor /mnt/harbor-kit
mountpoint -q /mnt/harbor || mount -t virtiofs HarborVM /mnt/harbor
mountpoint -q /mnt/harbor-kit || mount -t virtiofs HarborKit /mnt/harbor-kit
install -m 755 /mnt/harbor-kit/worker.py /usr/local/bin/harborvm-worker
mkdir -p /var/lib/harborvm /usr/local/share/harborvm
cat > /etc/systemd/system/harborvm-setup.service <<'SETUP'
[Unit]
Description=Finish HarborVM browser workstation setup
Wants=network-online.target
After=network-online.target
RequiresMountsFor=/mnt/harbor /mnt/harbor-kit
ConditionPathExists=!/var/lib/harborvm/setup-complete
[Service]
Type=oneshot
ExecStart=/bin/sh /mnt/harbor-kit/setup.sh
StandardOutput=append:/mnt/harbor/bootstrap.log
StandardError=append:/mnt/harbor/bootstrap.log
[Install]
WantedBy=multi-user.target
SETUP
cat > /etc/systemd/system/harborvm-worker.service <<'SERVICE'
[Unit]
Description=HarborVM guest command worker
After=local-fs.target
RequiresMountsFor=/mnt/harbor
[Service]
ExecStart=/usr/bin/python3 /usr/local/bin/harborvm-worker /mnt/harbor
Restart=always
RestartSec=2
[Install]
WantedBy=multi-user.target
SERVICE
systemctl daemon-reload
systemctl enable harborvm-setup
systemctl enable --now harborvm-worker
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends xserver-xorg openbox lightdm xterm dbus-x11 \
  curl ca-certificates fonts-noto-core fonts-noto-cjk python3-venv python3-pip nodejs npm
if ! command -v google-chrome >/dev/null; then
  mkdir -p /var/cache/harborvm
  curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --location \
    https://dl.google.com/linux/direct/google-chrome-stable_current_arm64.deb \
    -o /var/cache/harborvm/chrome.deb.partial
  mv /var/cache/harborvm/chrome.deb.partial /var/cache/harborvm/chrome.deb
  apt-get install -y /var/cache/harborvm/chrome.deb
fi
install -m 644 /mnt/harbor-kit/welcome.html /usr/local/share/harborvm/welcome.html
install -d -o harbor -g harbor /home/harbor/Workspace /home/harbor/.local \
  /home/harbor/.local/share /home/harbor/.local/share/applications /home/harbor/.local/share/harborvm
runuser -u harbor -- python3 -m venv /home/harbor/Workspace/.venv
runuser -u harbor -- /home/harbor/Workspace/.venv/bin/pip install 'playwright==1.64.1'
cat > /usr/local/bin/harbor-browser <<'BROWSER'
#!/bin/sh
set -eu
if [ "$(id -u)" = 0 ]; then echo 'Run the browser as the ordinary guest user.' >&2; exit 1; fi
export DISPLAY="${DISPLAY:-:0}"
if [ "$#" = 0 ]; then set -- file:///usr/local/share/harborvm/welcome.html; fi
mkdir -p "$HOME/.local/share/harborvm/chrome"
nohup google-chrome --user-data-dir="$HOME/.local/share/harborvm/chrome" \
  --remote-debugging-address=127.0.0.1 --remote-debugging-port=9222 \
  --no-first-run --no-default-browser-check --ozone-platform=x11 --start-maximized "$@" \
  </dev/null >"$HOME/.local/share/harborvm/browser.log" 2>&1 &
BROWSER
chmod 755 /usr/local/bin/harbor-browser
mkdir -p /etc/lightdm/lightdm.conf.d /home/harbor/.config/openbox
cat > /etc/lightdm/lightdm.conf.d/50-harborvm.conf <<'LIGHTDM'
[Seat:*]
autologin-user=harbor
autologin-user-timeout=0
user-session=openbox
LIGHTDM
cat > /home/harbor/.config/openbox/autostart <<'DESKTOP'
xsetroot -solid '#17252b'
harbor-browser
DESKTOP
chown -R harbor:harbor /home/harbor/.config
systemctl enable lightdm
systemctl set-default graphical.target
systemctl restart lightdm
touch /var/lib/harborvm/setup-complete
printf 'Desktop ready\n' > /mnt/harbor/desktop-ready
