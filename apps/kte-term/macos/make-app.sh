#!/bin/sh
# Builds "KTE Terminal.app" in target/release/.
#
#   apps/kte-term/macos/make-app.sh
#   open "target/release/KTE Terminal.app"
set -eu
cd "$(dirname "$0")/../../.."

cargo build -p kte-term --release

app="target/release/KTE Terminal.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/kte-term "$app/Contents/MacOS/kte-term"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>KTE Terminal</string>
  <key>CFBundleDisplayName</key><string>KTE Terminal</string>
  <key>CFBundleIdentifier</key><string>io.github.touchde.kte-term</string>
  <key>CFBundleExecutable</key><string>kte-term</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST

echo "$app"
