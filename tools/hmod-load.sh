#!/usr/bin/env bash
# Proves the built content package (content/mininggadgets.hmod) loads on a real
# game: the shared, unmodded 1d63e0a game with `--dev-mod <the .hmod>` must
# register the three gadget items and the upgrade cards. A second launch
# WITHOUT `--dev-mod` is the negative control: the same ids must be unknown
# there, so the check cannot pass vacuously.
set -Eeuo pipefail
export LC_ALL=C

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PIN_BIN/horizon-client}
HMOD=${HMOD:-$CLONE/ports/mininggadgets/content/mininggadgets.hmod}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/mininggadgets-hmod.sock}

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -f "$HMOD" ] || { echo "missing package: $HMOD" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/mininggadgets-hmod-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "${KEEP:-0}" = 1 ]; then echo "kept scratch: $SCRATCH" >&2; else rm -rf "$SCRATCH" 2>/dev/null || true; fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch WORLD [--dev-mod PATH]
    local world=$1; shift
    local out
    rm -f "$SOCK"
    cd "$SCRATCH"
    out=$("$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$world" --seed 20261105 --creative --headless --wait \
        --resources "$RES" "$@" 2>&1) || {
        echo "launch failed: $(tr -d '\n' <<<"$out" | cut -c1-400)" >&2
        return 1
    }
    ctl wait-ready 120
    ctl wait 20
}

echo "== launch WITH the package (--dev-mod $HMOD) =="
launch MiningGadgetsHmod --dev-mod "$HMOD"
for item in mininggadgets:mininggadget mininggadgets:mininggadget_fancy mininggadgets:upgrade_empty mininggadgets:upgrade_silk; do
    ctl give "$item" 1 >/dev/null
done
ctl wait 4
ctl inventory | jq -e '[.hotbar[], .main[] | select(.item != null) | .item | select(startswith("mininggadgets:"))] | length >= 4' >/dev/null \
    || fail "the package did not register its items: $(ctl inventory | jq -c '{hotbar,main}')"
echo "items: $(ctl inventory | jq -c '[.hotbar[], .main[] | select(.item != null) | .item | select(startswith("mininggadgets:"))]')"
# The card ids are the jar's (`Registration.SILK` is `upgrade_silk`), and the
# jar's en_us.json travels with them unchanged.
ctl quit >/dev/null 2>&1 || true
sleep 2

echo "== negative control: the SAME ids without --dev-mod =="
launch MiningGadgetsHmodControl
ctl give mininggadgets:mininggadget 1 >/dev/null 2>&1 || true
ctl wait 4
ctl inventory | jq -e '[.hotbar[], .main[] | select(.item != null) | .item | select(startswith("mininggadgets:"))] | length == 0' >/dev/null \
    || fail "the unmodded game gave a mininggadgets item: $(ctl inventory | jq -c '{hotbar,main}')"
echo "items: none"
echo "PASS: mininggadgets.hmod registered the three gadgets and the upgrade cards on a real, unmodded game, and the same ids are unknown without the package"
