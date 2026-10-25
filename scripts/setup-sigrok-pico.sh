#!/usr/bin/env bash
# Build libsigrok and sigrok-cli with the raspberrypi-pico driver.
#
# The released sigrok-cli 0.7.2 and PulseView 0.4.2 predate the driver, and
# Homebrew's HEAD libsigrok formula currently fails on a Python-bindings patch.
# This builds both tools from git into a private prefix without the C++, Python,
# or Java bindings.
#
# Usage:
#   scripts/setup-sigrok-pico.sh [PREFIX]
#
# Default PREFIX is $HOME/opt/sigrok-pico. Set WORK_DIR to choose where the
# source trees are cloned; the default is a fresh temporary directory.

set -euo pipefail

PREFIX="${1:-$HOME/opt/sigrok-pico}"
WORK_DIR="${WORK_DIR:-$(mktemp -d)}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"

LIB_SIGROK_URL="https://github.com/sigrokproject/libsigrok.git"
SIGROK_CLI_URL="https://github.com/sigrokproject/sigrok-cli.git"

fail() {
    echo "error: $*" >&2
    exit 1
}

for tool in git autoconf automake pkg-config make; do
    command -v "$tool" >/dev/null 2>&1 || fail "missing tool: $tool"
done

if ! command -v glibtoolize >/dev/null 2>&1 && ! command -v libtoolize >/dev/null 2>&1; then
    fail "missing libtoolize; on macOS run: brew install libtool"
fi

for module in glib-2.0 libserialport libusb-1.0 libzip hidapi libftdi1; do
    pkg-config --exists "$module" || fail "missing pkg-config module: $module"
done

if ! pkg-config --exists libsigrokdecode; then
    echo "warning: libsigrokdecode is missing; captures work but decoders do not." >&2
    echo "         on macOS run: brew install libsigrokdecode" >&2
fi

mkdir -p "$WORK_DIR" "$PREFIX"

echo "==> libsigrok -> $PREFIX"
if [ ! -d "$WORK_DIR/libsigrok/.git" ]; then
    git clone --depth 1 "$LIB_SIGROK_URL" "$WORK_DIR/libsigrok"
fi
(
    cd "$WORK_DIR/libsigrok"
    ./autogen.sh
    ./configure \
        --prefix="$PREFIX" \
        --disable-cxx \
        --disable-python \
        --disable-java \
        --disable-static
    make -j"$JOBS"
    make install
)

echo "==> sigrok-cli -> $PREFIX"
if [ ! -d "$WORK_DIR/sigrok-cli/.git" ]; then
    git clone --depth 1 "$SIGROK_CLI_URL" "$WORK_DIR/sigrok-cli"
fi
(
    cd "$WORK_DIR/sigrok-cli"
    ./autogen.sh
    PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}" \
        ./configure --prefix="$PREFIX"
    make -j"$JOBS"
    make install
)

echo "==> verifying"
if ! "$PREFIX/bin/sigrok-cli" -L | grep "raspberrypi-pico" >/dev/null; then
    fail "the raspberrypi-pico driver is missing from the build"
fi

if "$PREFIX/bin/sigrok-cli" -L | grep "rgb_led_ws281x" >/dev/null; then
    echo "decoder rgb_led_ws281x: present"
else
    echo "warning: rgb_led_ws281x decoder not found" >&2
fi

echo "Installed: $PREFIX/bin/sigrok-cli"
echo "Add it to PATH with: export PATH=\"$PREFIX/bin:\$PATH\""
