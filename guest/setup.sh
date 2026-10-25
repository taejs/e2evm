#!/bin/sh
set -eu
mkdir -p /mnt/harbor /mnt/harbor-kit
mountpoint -q /mnt/harbor || mount -t virtiofs HarborVM /mnt/harbor
mountpoint -q /mnt/harbor-kit || mount -t virtiofs HarborKit /mnt/harbor-kit
install -m 755 /mnt/harbor-kit/worker.py /usr/local/bin/harborvm-worker
cat > /etc/systemd/system/harborvm-worker.service <<'SERVICE'
[Unit]
Description=HarborVM guest command worker
After=local-fs.target
[Service]
ExecStart=/usr/bin/python3 /usr/local/bin/harborvm-worker /mnt/harbor
Restart=always
RestartSec=2
[Install]
WantedBy=multi-user.target
SERVICE
systemctl daemon-reload
systemctl enable --now harborvm-worker
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends xserver-xorg openbox lightdm xterm dbus-x11
mkdir -p /etc/lightdm/lightdm.conf.d /home/harbor/.config/openbox
cat > /etc/lightdm/lightdm.conf.d/50-harborvm.conf <<'LIGHTDM'
[Seat:*]
autologin-user=harbor
autologin-user-timeout=0
user-session=openbox
LIGHTDM
cat > /home/harbor/.config/openbox/autostart <<'DESKTOP'
xsetroot -solid '#17252b'
xterm -geometry 100x28+90+90 -bg '#101b21' -fg '#d4e8e5' -fa 'Monospace' -fs 13 -title 'HarborVM · Linux workspace' &
DESKTOP
chown -R harbor:harbor /home/harbor/.config
systemctl enable lightdm
systemctl set-default graphical.target
systemctl restart lightdm
printf 'Desktop ready\n' > /mnt/harbor/desktop-ready
