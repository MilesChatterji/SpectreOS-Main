#!/usr/bin/env bash
# Get NixOS install date and age in days

install_date=$(stat -c %W /nix/store 2>/dev/null || \
              stat -c %W /etc/nixos 2>/dev/null || \
              stat -c %W / 2>/dev/null || \
              echo 0)

current=$(date +%s)
days=$(( (current - install_date) / 86400 ))

if [ "$install_date" != "0" ] && [ "$install_date" != "" ]; then
  date_str=$(date -d "@$install_date" +%Y-%m-%d 2>/dev/null || echo "Unknown")
else
  date_str="Unknown"
fi

echo "$date_str ($days days)"
