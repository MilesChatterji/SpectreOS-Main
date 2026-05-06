# SpectreOS home-manager entry point.
# This file is your personal package list — add, remove, or rearrange anything.
# SpectreOS gives you a starting point, not a locked config.
#
# The SpectreOS Package Manager (spectreos-updater) reads and writes the
# marked section at the bottom of home.packages. Everything else is yours.
{ lib, pkgs, ... }:
let
  unstableSrc = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/6368eda62c9775c38ef7f714b2555a741c20c72d.tar.gz";
    sha256 = "0lhfh8fcsaifwzs388sg6cy0j2galj8ssfmk6wb0pc8alwdpi868";
  };
  unstable = import unstableSrc { config.allowUnfree = true; };
in
{
  imports = [ /etc/nixos/spectreos/defaults/home.nix ];
  home.username = "__USERNAME__";
  home.homeDirectory = "/home/__USERNAME__";

  home.packages = with pkgs; [
    # SpectreOS Updater managed packages — do not edit below
    # System info
    fastfetch
    neofetch
    microfetch
    cmatrix
    powertop
    conky
    # Terminal / TUI
    ghostty
    btop
    cava
    yazi
    fzf
    # Launcher
    fuzzel
    # Browsers / communication
    brave
    unstable.spotify
    signal-desktop
    signal-cli
    discord
    vlc
    thunderbird
    teams-for-linux
    # Private productivity
    proton-authenticator
    protonmail-bridge-gui
    proton-pass
    protonvpn-gui
    standardnotes
    # Development
    gh
    rustc
    cargo
    pkg-config
    virt-manager
    # Creative
    unstable.darktable
    gimp3
    audacity
    easyeffects
    # Utilities
    unstable.omnissa-horizon-client
    dropbox-cli
    # Fonts
    nerd-fonts.jetbrains-mono
    # GTK theming
    ayu-theme-gtk
    tela-icon-theme
    # END SpectreOS Updater
  ];
}
