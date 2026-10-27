#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
  echo 'E2EVM requires Apple Silicon and macOS 14 or newer.' >&2
  exit 1
fi
cargo build --release
mkdir -p "$ROOT/target/swift-cache"
APP="$ROOT/dist/E2EVM.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/guest"
xcrun swiftc -module-cache-path "$ROOT/target/swift-cache" -swift-version 5 -O -target arm64-apple-macos14.0 \
  -framework AppKit -framework SwiftUI -framework Virtualization \
  platform/macos/*.swift -o "$APP/Contents/MacOS/E2EVMDesktop"
cp target/release/e2evm "$APP/Contents/MacOS/e2evm"
cp platform/macos/Info.plist "$APP/Contents/Info.plist"
cp assets/brand/E2EVM.icns "$APP/Contents/Resources/E2EVM.icns"
cp guest/worker.py guest/setup.sh guest/setup-macos.sh guest/welcome.html "$APP/Contents/Resources/guest/"
codesign --force --sign - "$APP/Contents/MacOS/e2evm"
codesign --force --sign - --entitlements platform/macos/E2EVM.entitlements "$APP"
codesign --verify --strict "$APP"
printf 'Built %s\n' "$APP"
