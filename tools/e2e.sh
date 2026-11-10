#!/usr/bin/env bash
# In-game check for Mining Gadgets slice 1: the laser fires on right click,
# breaks the aimed block with its drop, and pays the gadget's own FE.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=200 (away from the central island, its dragon egg
# and the ambient item entities of a generated level). The player is
# teleported first so the chunks the fills touch are loaded.
#
# Default run:
#  A  a stone block at 202,101,0 aimed at its west face -> it becomes air, it
#     drops one cobblestone, and the buffer falls by exactly 200 FE;
#  B  a chest (a block entity) in the same spot -> it stays, no FE is spent
#     (upstream refuses block entities);
#  C  bedrock (hardness -1) -> it stays, no FE is spent (unbreakable);
#  D  a 3 x 3 stone wall with the `size_1` upgrade and range 3 -> all nine
#     blocks go for 1,800 FE, so the plane really is the aimed face;
#  E  the same wall with 300 FE, below the plane's 1,800 FE reserve -> nothing
#     breaks and the buffer is untouched;
#  F  a flat buffer does nothing at all.
#
# SELFTEST=1 flips A's energy assertion (it must FAIL), proving the scenario
# can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${MG_DEBUG:-}" ] || set -x

CLONE=${CAMPAIGN_ROOT:?set CAMPAIGN_ROOT to a checkout of the ATM10-Horizon campaign tree}
PORT=$CLONE/ports/mininggadgets
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/mininggadgets-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=MiningGadgetsE2E
# Where the arena stands: the End's void, far from the central island.
X0=200

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/mininggadgets-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

# `/mininggadgets info`'s line for the held gadget.
info() {
    say "mininggadgets info"
    ctl wait 12 >/dev/null
    ctl messages 12 | jq -r '.chat[]' | grep -m1 'Mining Gadget:' || true
}
# The held gadget's charge, parsed out of that line.
energy() {
    local line
    line=$(info)
    echo "  $line" >&2
    sed -n 's/.*: \([0-9][0-9]*\)\/[0-9][0-9]* FE.*/\1/p' <<<"$line"
}
expect_energy() { # expect_energy AMOUNT
    local got
    got=$(energy)
    [ -n "$got" ] || fail "the gadget answered no charge at all"
    [ "$got" = "$1" ] || fail "the buffer is $got FE, expected $1"
    echo "  buffer: $got FE"
}
# The block name at a position.
block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }
# Cobblestone the world holds: on the ground plus in the player's inventory.
cobble() {
    local ground held
    ground=$(ctl entities 16 item \
        | jq '[.entities[] | select(.kind == "item" and .item.item == "minecraft:cobblestone") | .item.count] | add // 0')
    held=$(ctl inventory | jq -r '.counts["minecraft:cobblestone"] // 0')
    echo $((ground + held))
}
clear_cobble() {
    ctl command "clear @s minecraft:cobblestone" >/dev/null 2>&1 || true
    ctl command "kill @e[type=minecraft:item]" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
}

launch() {
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$WORLD" --seed 20261105 --creative --headless --wait \
        --resources "$RES" --dev-mod "$PORT/content" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded (a fill in an
# unloaded chunk answers "That position is not loaded").
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 4)) 100 -4 $((X0 + 8)) 100 4 minecraft:stone" >/dev/null
    ctl wait 4 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 4)) 101 -4 $((X0 + 8)) 106 4 minecraft:air" >/dev/null
    ctl wait 4 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    local above
    above=$(block_name "$X0" 101 0)
    [ "$above" = "minecraft:air" ] || fail "the arena air above the floor is '$above', not air"
}

# Aim at a block's west face (the player stands west of it) and fire. The
# click's own reply is quoted in the log: `use-item` exits non-zero when the
# click changed nothing, which is exactly what a refused target does.
fire_at() { # fire_at X Y Z
    local aim click
    aim=$(ctl look-at-block "$1" "$2" "$3" west 2>&1) || fail "cannot aim at $1,$2,$3: $aim"
    ctl wait 2 >/dev/null
    click=$(ctl use-item 2>&1) || true
    echo "  use-item: $(tr -d '\n' <<<"$click" | cut -c1-160)"
    ctl wait 20 >/dev/null
}

echo "== launch (The End arena, content + native bundle) =="
launch
arena

echo "== give the gadget and charge it =="
ctl give mininggadgets:mininggadget 1 >/dev/null
ctl wait 4 >/dev/null
ctl select mininggadgets:mininggadget >/dev/null
ctl wait 4 >/dev/null
ctl inventory | jq -e '[.hotbar[] | select(.item == "mininggadgets:mininggadget")] | length >= 1' >/dev/null \
    || fail "the gadget item did not register: $(ctl inventory | jq -c '{hotbar,main}')"
say "mininggadgets energy 10000"
ctl wait 12 >/dev/null
expect_energy 10000
clear_cobble

echo "== A: one stone block, aimed west face -> air, one drop, 200 FE =="
ctl command "execute in minecraft:the_end run setblock $((X0 + 2)) 101 0 minecraft:stone" >/dev/null
ctl wait 4 >/dev/null
fire_at $((X0 + 2)) 101 0
a_block=$(block_name $((X0 + 2)) 101 0)
[ "$a_block" = "minecraft:air" ] || fail "the aimed block is '$a_block', not air"
a_cobble=$(cobble)
[ "$a_cobble" = 1 ] || fail "the break dropped $a_cobble cobblestone, expected 1"
echo "  block $((X0 + 2)),101,0 -> $a_block; cobblestone $a_cobble"
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting the buffer fell to 9700)"
    expect_energy 9700
else
    expect_energy 9800
fi

echo "== B: a chest (block entity) is not a target =="
# The pin has no hook before a block's own use, so `use_item_on` reaches the
# gadget only after the chest opened its screen (upstream cancels
# `RightClickBlock` first). The gadget must still not break it or spend FE.
ctl command "execute in minecraft:the_end run setblock $((X0 + 2)) 101 2 minecraft:chest" >/dev/null
ctl wait 4 >/dev/null
fire_at $((X0 + 2)) 101 2
b_screen=$(ctl screen | jq -r '.screen // .name // empty')
echo "  the chest's own use ran first: screen=$b_screen"
ctl close >/dev/null 2>&1 || true
ctl wait 4 >/dev/null
b_block=$(block_name $((X0 + 2)) 101 2)
[ "$b_block" = "minecraft:chest" ] || fail "the chest became '$b_block'"
expect_energy 9800
echo "  block $((X0 + 2)),101,2 still $b_block, buffer untouched"

echo "== C: bedrock (hardness -1) is not a target =="
ctl command "execute in minecraft:the_end run setblock $((X0 + 2)) 101 -2 minecraft:bedrock" >/dev/null
ctl wait 4 >/dev/null
fire_at $((X0 + 2)) 101 -2
c_block=$(block_name $((X0 + 2)) 101 -2)
[ "$c_block" = "minecraft:bedrock" ] || fail "bedrock became '$c_block'"
expect_energy 9800
echo "  block $((X0 + 2)),101,-2 still $c_block, buffer untouched"

echo "== D: size_1 + range 3 mines the whole aimed face =="
ctl command "execute in minecraft:the_end run fill $((X0 + 2)) 100 -1 $((X0 + 2)) 102 1 minecraft:stone" >/dev/null
ctl wait 4 >/dev/null
say "mininggadgets upgrade size_1"
ctl wait 12 >/dev/null
say "mininggadgets range 3"
ctl wait 12 >/dev/null
say "mininggadgets energy 10000"
ctl wait 12 >/dev/null
clear_cobble
fire_at $((X0 + 2)) 101 0
d_air=0
for y in 100 101 102; do
    for z in -1 0 1; do
        [ "$(block_name $((X0 + 2)) "$y" "$z")" = "minecraft:air" ] && d_air=$((d_air + 1))
    done
done
[ "$d_air" = 9 ] || fail "only $d_air of the 3 x 3 face became air"
d_cobble=$(cobble)
[ "$d_cobble" = 9 ] || fail "the plane dropped $d_cobble cobblestone, expected 9"
expect_energy 8200
echo "  the 3 x 3 face at x=$((X0 + 2)) is air; cobblestone $d_cobble"

echo "== E: 300 FE is below the plane's 1,800 FE reserve =="
ctl command "execute in minecraft:the_end run fill $((X0 + 2)) 100 -1 $((X0 + 2)) 102 1 minecraft:stone" >/dev/null
ctl wait 4 >/dev/null
say "mininggadgets energy 300"
ctl wait 12 >/dev/null
fire_at $((X0 + 2)) 101 0
e_block=$(block_name $((X0 + 2)) 101 0)
[ "$e_block" = "minecraft:stone" ] || fail "the underfunded shot broke the plane (got '$e_block')"
expect_energy 300
echo "  block $((X0 + 2)),101,0 still $e_block, buffer untouched"

echo "== F: a flat buffer does nothing =="
ctl command "execute in minecraft:the_end run setblock $((X0 + 2)) 101 0 minecraft:dirt" >/dev/null
ctl wait 4 >/dev/null
say "mininggadgets energy 0"
ctl wait 12 >/dev/null
fire_at $((X0 + 2)) 101 0
f_block=$(block_name $((X0 + 2)) 101 0)
[ "$f_block" = "minecraft:dirt" ] || fail "an empty gadget broke the block (got '$f_block')"
expect_energy 0
echo "  block $((X0 + 2)),101,0 still $f_block"

echo "PASS: the laser broke the aimed stone block and dropped its cobblestone for 200 FE, left a chest and bedrock alone, mined a 3 x 3 face for 1,800 FE, refused an underfunded plane and an empty buffer, and every control left the buffer untouched"
