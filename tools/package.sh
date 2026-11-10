#!/usr/bin/env bash
# Package both artifacts:
#  - the source-only .hnmod (tools/package.py, deterministic bytes)
#  - the .hmod via the pin's own cargo-horizon-mod (build + check), from content/
# Run inside a build slot (it compiles the packaging tool):
#   ports/tools/build-slot.sh run --job mg-pack --port ports/mininggadgets \
#       --step package -- env CARGO_INCREMENTAL=0 tools/package.sh
set -euo pipefail

PORT=$(cd "$(dirname "$0")/.." && pwd)
HORIZON=${HORIZON:-${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}/ports/_shared/pins/horizon-1d63e0a}
LOGS=$PORT/target/ladder
mkdir -p "$LOGS"
cd "$PORT"

python3 tools/package.py | tee "$LOGS/package-hnmod.log"
cp "target/mininggadgets-0.1.0.hnmod" "$PORT/mininggadgets-0.1.0.hnmod"

CARGO_TARGET_DIR=$PORT/target/carry-tools
export CARGO_TARGET_DIR
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod build) \
    2>&1 | tee "$LOGS/package-hmod.log"
cp content/target/horizon-mod/mininggadgets.hmod content/mininggadgets.hmod
(cd content && cargo run --locked --offline --quiet \
    --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- horizon-mod check mininggadgets.hmod) \
    2>&1 | tee -a "$LOGS/package-hmod.log"

echo "== artifacts =="
for f in content/mininggadgets.hmod mininggadgets-0.1.0.hnmod; do
    [ -f "$f" ] || { echo "missing $f"; exit 1; }
    size=$(stat -c %s "$f")
    sha=$(sha256sum "$f" | cut -d' ' -f1)
    entries=$(python3 -c "import zipfile,sys; print(len(zipfile.ZipFile(sys.argv[1]).namelist()))" "$f")
    echo "$f size=$size sha256=$sha entries=$entries"
done
