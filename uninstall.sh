#!/bin/sh
# Removes everything install.sh installed (driver, system files and the Bitwig extension).
# The project folder itself (sources, target/) is left untouched.
set -u
systemctl --user stop maschine-mk3 2>/dev/null
sudo rm -f /usr/local/bin/maschine-mk3 \
    /etc/udev/rules.d/70-maschine-mk3.rules \
    /etc/systemd/user/maschine-mk3.service \
    /etc/modules-load.d/snd-virmidi.conf \
    /etc/modprobe.d/snd-virmidi.conf
sudo udevadm control --reload
systemctl --user daemon-reload
rm -f "$HOME/Bitwig Studio/Extensions/MaschineMk3Linux.bwextension"
if lsmod | grep -q snd_virmidi; then
    sudo modprobe -r snd-virmidi || echo "snd-virmidi is still in use (is Bitwig open?): close Bitwig and run 'sudo modprobe -r snd-virmidi', or reboot."
fi
echo "Uninstalled."
