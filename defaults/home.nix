# SpectreOS default home-manager configuration.
# Hardware-agnostic and user-agnostic — host-specific items (Shadow client,
# NVIDIA desktop files, PX13-only services) are NOT included here.
#
# Consumed by the installer wrapper at ~/.config/home-manager/home.nix, which
# sets home.username and home.homeDirectory for the specific user, then
# imports this file.

{ config, pkgs, ... }:

{
  home.stateVersion = "25.11";

  nixpkgs.config.allowUnfree = true;

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
