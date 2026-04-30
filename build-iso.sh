#!/usr/bin/env bash
# Build the SpectreOS VM installer ISO.
# Run from the repo root as a regular user (no sudo needed).
#
# Output: ./result/iso/spectreos-vm-installer.iso

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Building SpectreOS VM installer ISO..."
echo "    This will take a while on the first run."
echo ""

nix-build '<nixpkgs/nixos>' \
  -A config.system.build.isoImage \
  -I nixos-config="$SCRIPT_DIR/hosts/iso/iso.nix"

echo ""
echo "==> Done."
echo ""
echo "    ISO: $SCRIPT_DIR/result/iso/spectreos-vm-installer.iso"
