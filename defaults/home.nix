# SpectreOS default home-manager configuration.
# Hardware-agnostic and user-agnostic — host-specific items (Shadow client,
# NVIDIA desktop files, PX13-only services) are NOT included here.
#
# Consumed by the installer wrapper at ~/.config/home-manager/home.nix, which
# sets home.username and home.homeDirectory for the specific user, then
# imports this file.

{ config, pkgs, ... }:

let
  # Same nixpkgs-unstable pin as modules/desktop/niri.nix.
  # To update: nix-prefetch-url --unpack https://github.com/NixOS/nixpkgs/archive/<commit>.tar.gz
  unstableSrc = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/6368eda62c9775c38ef7f714b2555a741c20c72d.tar.gz";
    sha256 = "0lhfh8fcsaifwzs388sg6cy0j2galj8ssfmk6wb0pc8alwdpi868";
  };
  unstable = import unstableSrc { config.allowUnfree = true; };
in

{
  home.stateVersion = "25.11";

  nixpkgs.config.allowUnfree = true;

  home.packages = with pkgs; [
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
    zoom-us
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
    # unstable.code-cursor      # Large; install manually if needed in a VM
    # unstable.claude-code       # Large; install manually if needed in a VM
    virt-manager

    # Creative
    # unstable.davinci-resolve-studio  # GPU-dependent; not suitable for VM use
    # alsa-plugins                     # ALSA → PipeWire bridge (only needed for DaVinci Resolve)
    unstable.darktable
    gimp3
    audacity
    easyeffects

    # Utilities
    unstable.omnissa-horizon-client
    dropbox-cli

    # Games
    # steam                      # Performance-dependent; install manually if needed in a VM

    # Fonts
    nerd-fonts.jetbrains-mono

    # GTK theming
    ayu-theme-gtk
    tela-icon-theme
  ];

  home.file = {
    # Route ALSA applications through PipeWire's PulseAudio compatibility layer.
    ".asoundrc".text = ''
      pcm.!default {
        type pulse
      }
      ctl.!default {
        type pulse
      }
    '';
  };

  home.sessionVariables = {
    EDITOR = "nvim";
  };

  programs.home-manager.enable = true;

  gtk = {
    enable = true;
    theme = {
      name = "Ayu-Dark";
      package = pkgs.ayu-theme-gtk;
    };
    iconTheme = {
      name = "Tela-orange";
      package = pkgs.tela-icon-theme;
    };
    gtk3.extraConfig.gtk-application-prefer-dark-theme = true;
    gtk4.extraConfig.gtk-application-prefer-dark-theme = true;
  };

  systemd.user.services.dropbox = {
    Unit = {
      Description = "Dropbox";
      After = [ "graphical-session.target" ];
    };
    Install = {
      WantedBy = [ "graphical-session.target" ];
    };
    Service = {
      ExecStart = "${pkgs.dropbox-cli}/bin/dropbox";
      ExecReload = "${pkgs.coreutils}/bin/kill -HUP $MAINPID";
      KillMode = "control-group";
      Restart = "on-failure";
      PrivateTmp = true;
      ProtectSystem = "full";
      Nice = 10;
      Environment = [
        "QT_PLUGIN_PATH=/run/current-system/sw/${pkgs.qt5.qtbase.qtPluginPrefix}"
        "QML2_IMPORT_PATH=/run/current-system/sw/${pkgs.qt5.qtbase.qtQmlPrefix}"
      ];
    };
  };
}
