#!/bin/bash
# Build "OM Disk Analyzer.app" and a drag-to-Applications .dmg installer on macOS.
#
#   bash tools/build_mac.sh            # for this Mac's chip
#   bash tools/build_mac.sh arm64      # Apple Silicon (M1/M2/M3/M4...)
#   bash tools/build_mac.sh x86_64     # Intel Macs
#
# Needs Python 3.12+ from python.org (it includes Tk) or the one from GitHub's setup-python.
# A chip type other than this Mac's needs a "universal2" Python from python.org.
set -euo pipefail
cd "$(dirname "$0")/.."
APP_DIR="$PWD"
ARCH="${1:-$(uname -m)}"
NAME="OM Disk Analyzer"
VERSION=$(grep '^VERSION = ' omdisk.py | head -1 | cut -d'"' -f2)
OUT="dist/mac-$ARCH"

python3 -m pip install --quiet --upgrade pyinstaller
python3 -m PyInstaller --noconfirm --clean --windowed --name "$NAME" \
  --icon "$APP_DIR/assets/om.icns" --add-data "$APP_DIR/assets:assets" \
  --osx-bundle-identifier com.omindustries.diskanalyzer --target-arch "$ARCH" \
  --distpath "$OUT" --workpath "$APP_DIR/.build" --specpath "$APP_DIR/.build" "$APP_DIR/main.py"
rm -rf "$APP_DIR/.build"

# sharper text on Retina screens, and the version in Finder
PLIST="$OUT/$NAME.app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :NSHighResolutionCapable bool true" "$PLIST" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$PLIST" 2>/dev/null || true
codesign --force --deep --sign - "$OUT/$NAME.app"   # ad-hoc signature so Apple Silicon will start it

STAGE=$(mktemp -d)
cp -R "$OUT/$NAME.app" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="dist/$NAME $VERSION macOS $([ "$ARCH" = arm64 ] && echo 'Apple Silicon' || echo 'Intel').dmg"
for try in 1 2 3; do  # hdiutil is sometimes "busy" on build machines
  hdiutil create -volname "$NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" && break
  sleep 5
done
[ -f "$DMG" ]
rm -rf "$STAGE"
echo "built: $DMG"
