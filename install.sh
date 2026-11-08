#!/bin/sh
# Installs the Maschine MK3 Linux driver and the "Maschine Mk3 (Linux)" Bitwig extension.
#
#   ./install.sh                      driver + prebuilt extension
#   ./install.sh --rebuild-extension  also recompile the extension (needs maven and a JDK 21)
#
# Build dependencies (Fedora): sudo dnf install rust cargo alsa-lib-devel libusb1-devel
set -e
cd "$(dirname "$0")"

echo "==> Building the driver"
cargo build --release

echo "==> Installing the driver (asks for your password)"
sudo install -m 755 target/release/maschine-mk3 /usr/local/bin/
sudo install -m 644 packaging/70-maschine-mk3.rules /etc/udev/rules.d/
sudo install -m 644 packaging/maschine-mk3.service /etc/systemd/user/
sudo install -m 644 packaging/modules-load-snd-virmidi.conf /etc/modules-load.d/snd-virmidi.conf
sudo install -m 644 packaging/modprobe-snd-virmidi.conf /etc/modprobe.d/snd-virmidi.conf
sudo udevadm control --reload
systemctl --user daemon-reload
if ! lsmod | grep -q snd_virmidi; then
    sudo modprobe snd-virmidi
fi
# restart the driver if it is already running, so the new version is used
systemctl --user try-restart maschine-mk3 || true

echo "==> Installing the Bitwig extension"
if [ "${1:-}" = "--rebuild-extension" ]; then
    ./bitwig-extension/build.sh --rebuild
else
    ./bitwig-extension/build.sh
fi

echo
echo "Done."
echo "- First install: unplug and replug the Maschine MK3."
echo "- Restart Bitwig, then in Settings > Controllers add"
echo "  Native Instruments > Maschine Mk3 (Linux), with 'Virtual Raw MIDI/10' as input and output."
