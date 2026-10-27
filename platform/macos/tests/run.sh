#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
TEMP=$(mktemp -d "${TMPDIR:-/tmp}/harborvm-native-tests.XXXXXX")
trap 'rm -rf "$TEMP"' EXIT HUP INT TERM
xcrun swiftc -swift-version 5 -module-cache-path "$TEMP/modules" \
  "$ROOT/platform/macos/NativeSafety.swift" \
  "$ROOT/platform/macos/GuestKeyboard.swift" \
  "$ROOT/platform/macos/tests/SafetyTests.swift" -o "$TEMP/safety-tests"
"$TEMP/safety-tests"
