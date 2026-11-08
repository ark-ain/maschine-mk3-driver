#!/bin/sh
# Installs the "Maschine Mk3 (Linux)" Bitwig extension for the current user.
# A compiled MaschineMk3Linux.bwextension is included; pass --rebuild to compile it again
# from the sources (needs: sudo dnf install maven java-21-openjdk-devel).
# Usually called by ../install.sh.
set -e
cd "$(dirname "$0")"
if [ "${1:-}" = "--rebuild" ] || [ ! -f MaschineMk3Linux.bwextension ]; then
    mvn -q -B package
    cp target/MaschineMk3Linux.jar MaschineMk3Linux.bwextension
fi
DEST="$HOME/Bitwig Studio/Extensions"
mkdir -p "$DEST"
cp MaschineMk3Linux.bwextension "$DEST/MaschineMk3Linux.bwextension"
echo "Installed: $DEST/MaschineMk3Linux.bwextension"
echo "Restart Bitwig (or disable/enable the 'Maschine Mk3 (Linux)' controller)."
