#!/bin/bash
# Apply our Pingora patches to the pingora/ submodule.
#
# Usage:
#   ./patches/apply.sh          # apply all patches
#   ./patches/apply.sh --check  # dry-run to verify patches apply cleanly
#
# After applying, rebuild with:
#   cargo build --release
set -e

cd "$(dirname "$0")/.."
PINGORA_DIR="pingora"

if [ ! -d "$PINGORA_DIR/pingora-proxy" ]; then
    echo "Error: $PINGORA_DIR/ not found or not initialized."
    echo "Run: git submodule update --init"
    exit 1
fi

CHECK_FLAG=""
if [ "$1" = "--check" ]; then
    CHECK_FLAG="--dry-run"
    echo "Dry-run mode — checking if patches apply cleanly..."
fi

echo "Applying patches to $PINGORA_DIR/..."
for patchfile in patches/*.patch; do
    echo "  $(basename $patchfile)"
    patch -p1 $CHECK_FLAG -d "$PINGORA_DIR" < "$patchfile"
done

if [ -z "$CHECK_FLAG" ]; then
    echo "Done. Rebuild with: cargo build --release"
else
    echo "All patches apply cleanly."
fi
