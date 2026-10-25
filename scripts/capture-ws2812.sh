#!/usr/bin/env bash
# Capture a WS2812 frame with the Pico analyzer and check it against the simulator.
#
# The Pico H runs the sigrok-pico firmware and is clipped to the strip data pin;
# see "Diagnostics with a Pico" in docs/hardware.md. Build the host client once
# with scripts/setup-sigrok-pico.sh.

set -euo pipefail

PORT="${SIGROK_PICO_PORT:-}"
OUTPUT="/tmp/ws2812.sr"
RATE=24000000
CHANNEL="D2"
SAMPLES=380000
TIME=""
SEED=""
COLOR=""
DECODE_ONLY=0
COMPARE=1
TIMING=1
DECODED=""
EXPECTED=""

usage() {
    cat <<'EOF'
Capture a WS2812 frame with the Pico analyzer and check it against the simulator.

Usage:
  scripts/capture-ws2812.sh [options]

Options:
  -p, --port DEVICE    Pico CDC port; defaults to $SIGROK_PICO_PORT or the
                       single matching /dev/cu.usbmodem* or /dev/ttyACM*
  -o, --output FILE    session file to write or read (default /tmp/ws2812.sr)
  -r, --rate HZ        sample rate (default 24000000)
  -c, --channel CH     digital channel (default D2, which is GPIO2)
  -n, --samples N      samples to capture (default 380000)
  -t, --time TIME      expected frame, e.g. "15.07.2026 06:30"
      --seed N         seed for the expected frame
      --color RRGGBB   fixed colour for the expected frame
      --decode-only    decode an existing --output file instead of capturing
      --no-compare     skip the frame comparison
      --no-timing      skip the longest-low analysis
  -h, --help

Without --time the script only captures and decodes. With it, the decoded
pixels must contain the frame that examples/dump_frame.rs renders for the same
time, seed, and colour. A capture can start mid-frame, so the frame is matched
at any offset.

Examples:
  scripts/capture-ws2812.sh
  scripts/capture-ws2812.sh --time "15.07.2026 06:30" --seed 42
  scripts/capture-ws2812.sh --decode-only -o /tmp/ws2812.sr \
      --time "15.07.2026 06:30" --seed 42
EOF
}

fail() {
    echo "error: $*" >&2
    exit 1
}

cleanup() {
    [ -n "$DECODED" ] && rm -f "$DECODED"
    [ -n "$EXPECTED" ] && rm -f "$EXPECTED"
    return 0
}
trap cleanup EXIT

while [ $# -gt 0 ]; do
    case "$1" in
        -p|--port) PORT="$2"; shift 2 ;;
        -o|--output) OUTPUT="$2"; shift 2 ;;
        -r|--rate) RATE="$2"; shift 2 ;;
        -c|--channel) CHANNEL="$2"; shift 2 ;;
        -n|--samples) SAMPLES="$2"; shift 2 ;;
        -t|--time) TIME="$2"; shift 2 ;;
        --seed) SEED="$2"; shift 2 ;;
        --color) COLOR="$2"; shift 2 ;;
        --decode-only) DECODE_ONLY=1; shift ;;
        --no-compare) COMPARE=0; shift ;;
        --no-timing) TIMING=0; shift ;;
        -h|--help) usage; exit 0 ;;
        *) fail "unknown option: $1 (try --help)" ;;
    esac
done

SIGROK_CLI="${SIGROK_CLI:-}"
if [ -z "$SIGROK_CLI" ]; then
    if command -v sigrok-cli >/dev/null 2>&1; then
        SIGROK_CLI="$(command -v sigrok-cli)"
    elif [ -x "$HOME/opt/sigrok-pico/bin/sigrok-cli" ]; then
        SIGROK_CLI="$HOME/opt/sigrok-pico/bin/sigrok-cli"
    else
        fail "sigrok-cli not found; run scripts/setup-sigrok-pico.sh first"
    fi
fi

# `grep -q` would close the pipe early and, with pipefail, turn the resulting
# SIGPIPE into a failure. Capture the list once instead.
SUPPORTED="$("$SIGROK_CLI" -L)"
case "$SUPPORTED" in
    *rgb_led_ws281x*) ;;
    *) fail "this sigrok-cli has no rgb_led_ws281x decoder; run scripts/setup-sigrok-pico.sh" ;;
esac

detect_port() {
    if [ -n "$PORT" ]; then
        echo "$PORT"
        return 0
    fi

    local pattern matches count
    case "$(uname -s)" in
        Darwin) pattern="/dev/cu.usbmodem*" ;;
        *) pattern="/dev/ttyACM*" ;;
    esac

    matches="$(compgen -G "$pattern" || true)"
    count="$(printf '%s' "$matches" | grep -c . || true)"

    if [ "$count" -eq 0 ]; then
        fail "no Pico found at $pattern; pass --port or check '$SIGROK_CLI --list-serial'"
    fi
    if [ "$count" -gt 1 ]; then
        printf 'error: several ports match %s:\n%s\n' "$pattern" "$matches" >&2
        fail "pass --port; the ESP32-C3 also enumerates as a usbmodem"
    fi

    echo "$matches"
}

if [ "$DECODE_ONLY" -eq 0 ]; then
    case "$SUPPORTED" in
        *raspberrypi-pico*) ;;
        *) fail "this sigrok-cli has no raspberrypi-pico driver; run scripts/setup-sigrok-pico.sh" ;;
    esac

    if [ "$SAMPLES" -gt 400000 ]; then
        echo "warning: $SAMPLES samples exceeds the 400k fixed-depth window;" >&2
        echo "         the driver falls back to USB streaming and may overflow" >&2
    fi

    PORT="$(detect_port)"
    echo "==> capturing $SAMPLES samples at $RATE Hz on $CHANNEL from $PORT"

    # No trigger: the driver switches to USB streaming for triggered captures,
    # which overflows at this rate. Untriggered captures use fixed depth.
    "$SIGROK_CLI" \
        -d "raspberrypi-pico:conn=$PORT:serialcomm=115200/flow=0" \
        --config "samplerate=$RATE" \
        --channels "$CHANNEL" \
        --samples "$SAMPLES" \
        -o "$OUTPUT"
else
    [ -f "$OUTPUT" ] || fail "no session file at $OUTPUT; drop --decode-only to capture"
fi

DECODED="$(mktemp)"

echo "==> decoding $OUTPUT"
"$SIGROK_CLI" -i "$OUTPUT" -P rgb_led_ws281x -A rgb_led_ws281x=rgb-vals \
    | grep -oE '#[0-9a-fA-F]{6}' \
    | tr 'A-F' 'a-f' > "$DECODED"

PIXELS="$(wc -l < "$DECODED" | tr -d ' ')"
[ "$PIXELS" -gt 0 ] || fail "the decoder produced no pixels; check the probe and the channel"
echo "decoded $PIXELS pixels"

if [ "$TIMING" -eq 1 ]; then
    echo "==> timing"
    "$SIGROK_CLI" -i "$OUTPUT" -O bits | awk -v rate="$RATE" '
        BEGIN {
            maxlow = 0
            resets = 0
            run = 0
        }
        function note(n) {
            if (n >= rate / 20000) {
                resets++
            } else if (n > maxlow) {
                maxlow = n
            }
        }
        /^META samplerate:/ {
            sub(/^META samplerate: */, "")
            rate = $0 + 0
        }
        /^[A-Za-z0-9_]+:[01 ]+$/ {
            line = $0
            sub(/^[^:]*:/, "", line)
            gsub(/ /, "", line)
            for (i = 1; i <= length(line); i++) {
                if (substr(line, i, 1) == "0") {
                    run++
                } else {
                    if (run) note(run)
                    run = 0
                }
            }
        }
        END {
            if (run) note(run)
            if (rate <= 0) {
                print "no sample rate found in the session file" > "/dev/stderr"
                exit 1
            }
            printf "longest low under 50 us: %.3f us (%d samples)\n", maxlow / rate * 1000000, maxlow
            printf "reset gaps of 50 us or more: %d\n", resets
        }'
fi

STATUS=0
if [ "$COMPARE" -eq 1 ] && [ -n "$TIME" ]; then
    echo "==> comparing against the simulator"
    EXPECTED="$(mktemp)"

    DUMP_ARGS=("$TIME")
    if [ -n "$COLOR" ]; then
        DUMP_ARGS+=(--color "$COLOR")
    fi
    if [ -n "$SEED" ]; then
        DUMP_ARGS+=(--seed "$SEED")
    fi

    cargo run --quiet --release --example dump_frame -- "${DUMP_ARGS[@]}" \
        | awk '$1 ~ /^[0-9]+$/ { print "#" tolower($3) }' > "$EXPECTED"

    if awk '
        NR == FNR { want[++n] = $0; next }
        { got[++m] = $0 }
        END {
            best = 0
            best_at = 0
            for (s = 1; s + n - 1 <= m; s++) {
                k = 0
                while (k < n && got[s + k] == want[k + 1]) k++
                if (k > best) {
                    best = k
                    best_at = s
                }
                if (k == n) {
                    printf "parity: OK, all %d pixels match at offset %d of %d decoded\n", n, s - 1, m
                    exit 0
                }
            }
            if (best_at == 0) {
                if (m < n)
                    printf "parity: FAIL, decoded %d pixels, need a window of %d\n", m, n
                else
                    printf "parity: FAIL, the expected first pixel %s never appears\n", want[1]
                exit 1
            }
            printf "parity: FAIL, best run %d of %d pixels at offset %d\n", best, n, best_at - 1
            for (i = best + 1; i <= n && i <= best + 3 && best_at + i - 1 <= m; i++)
                printf "  pixel %d: expected %s, decoded %s\n", i - 1, want[i], got[best_at + i - 1]
            exit 1
        }
    ' "$EXPECTED" "$DECODED"; then
        STATUS=0
    else
        STATUS=1
    fi
fi

exit "$STATUS"
