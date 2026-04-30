#!/usr/bin/env bash
# SpectreOS first-boot user environment setup.
# Launched automatically by niri on first login when ~/.hm-pending exists.
# Runs home-manager switch, then removes the flag file on success.

FLAG="$HOME/.hm-pending"

# Only run if the pending flag exists (first login only).
[ -f "$FLAG" ] || exit 0

clear
echo ""
echo "  ╔══════════════════════════════════════════════════════════╗"
echo "  ║                                                          ║"
echo "  ║                  S P E C T R E  O S                     ║"
echo "  ║              First-Boot Environment Setup                ║"
echo "  ║                                                          ║"
echo "  ╚══════════════════════════════════════════════════════════╝"
echo ""
echo "  Setting up your user environment. This takes a few minutes"
echo "  depending on your connection speed. Please do not close"
echo "  this window."
echo ""
echo "  ──────────────────────────────────────────────────────────"
echo ""

home-manager switch -b backup --option max-jobs 2 --option cores 2
EXIT=$?

echo ""
echo "  ──────────────────────────────────────────────────────────"
echo ""

if [ "$EXIT" -eq 0 ]; then
    rm -f "$FLAG"

    # Apply wallpaper now — swww-daemon is already running from niri startup.
    WALLPAPER="$HOME/Pictures/SpectreOS/SpectreOSWall.png"
    [ -f "$WALLPAPER" ] && swww img "$WALLPAPER" --transition-type fade 2>/dev/null || true

    echo "  Setup complete. Your SpectreOS environment is ready."
    echo ""
    echo "  Rebooting in 10 seconds for the best experience..."
    echo "  Press Ctrl+C to cancel."
    echo ""
    for i in 10 9 8 7 6 5 4 3 2 1; do
        printf "  %s...\n" "$i"
        sleep 1
    done
    systemctl reboot
else
    echo "  Setup encountered an error (exit code $EXIT)."
    echo ""
    echo "  To retry, open a terminal and run:"
    echo "    home-manager switch"
    echo ""
    echo "  Press Enter to close this window."
    read -r
fi
