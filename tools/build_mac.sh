#!/bin/bash
# Build "OM Disk Analyzer.app" and a drag-to-Applications .dmg on a Mac (Rust version).
#
#   bash tools/build_mac.sh            # for this Mac's chip
#   bash tools/build_mac.sh arm64      # Apple Silicon (M1 and later)
#   bash tools/build_mac.sh x86_64     # Intel Macs
#
# Needs Rust (https://rustup.rs). GitHub's Mac computers have it already; see
# .github/workflows/build.yml. The .app is signed ad-hoc (no Apple Developer account), so on
# another Mac the first start is "right-click > Open" (see docs/INSTALLING.md).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
ARCH="${1:-$(uname -m)}"
case "$ARCH" in
  arm64|aarch64) ARCH=arm64;  TARGET=aarch64-apple-darwin; CHIP="Apple Silicon" ;;
  x86_64|intel)  ARCH=x86_64; TARGET=x86_64-apple-darwin;  CHIP="Intel" ;;
  *) echo "unknown chip '$ARCH' (use arm64 or x86_64)"; exit 2 ;;
esac

NAME="OM Disk Analyzer"
FULL=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)      # 2.0.0
VERSION=${FULL%.0}                                               # 2.0   (same rule as package.ps1)
OUT="dist/mac-$ARCH"
APP="$OUT/$NAME.app"

rustup target add "$TARGET" 2>/dev/null || true
cargo build --release --target "$TARGET"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/$TARGET/release/om-disk-analyzer" "$APP/Contents/MacOS/om-disk-analyzer"
cp assets/om.icns "$APP/Contents/Resources/om.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>com.omindustries.diskanalyzer</string>
  <key>CFBundleExecutable</key><string>om-disk-analyzer</string>
  <key>CFBundleIconFile</key><string>om</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$FULL</string>
  <key>CFBundleVersion</key><string>$FULL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>© 2026 OM Industries Inc. All rights reserved.</string>
  <key>NSDesktopFolderUsageDescription</key><string>OM Disk Analyzer reads folder sizes so it can show what uses your space.</string>
  <key>NSDocumentsFolderUsageDescription</key><string>OM Disk Analyzer reads folder sizes so it can show what uses your space.</string>
  <key>NSDownloadsFolderUsageDescription</key><string>OM Disk Analyzer reads folder sizes so it can show what uses your space.</string>
  <key>NSRemovableVolumesUsageDescription</key><string>OM Disk Analyzer reads folder sizes so it can show what uses your space.</string>
  <key>NSNetworkVolumesUsageDescription</key><string>OM Disk Analyzer reads folder sizes so it can show what uses your space.</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"     # ad-hoc signature: Apple Silicon refuses unsigned programs
codesign --verify --deep --strict "$APP"

STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="dist/$NAME $VERSION macOS $CHIP.dmg"
rm -f "$DMG"
for try in 1 2 3; do   # hdiutil is sometimes "busy" on build machines
  hdiutil create -volname "$NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" && break
  sleep 5
done
[ -f "$DMG" ]
rm -rf "$STAGE"
echo "built: $DMG"
