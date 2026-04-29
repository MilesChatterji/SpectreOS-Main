# Niri Wayland compositor + Noctalia Shell
# Hardware-agnostic base. PX13-specific blocks (AMD wrapper, MST, color profile) are commented out.

{ config, pkgs, ... }:

let
  unstable = import <unstable> { config.allowUnfree = true; };

  # Save/restore display brightness around idle dimming
  brightness-save-restore = pkgs.writeScriptBin "brightness-save-restore" ''
    #!${pkgs.bash}/bin/bash
    BRIGHTNESS_FILE="$HOME/.cache/niri-brightness-backup"
    case "$1" in
      save)
        CURRENT=$(brightnessctl --class=backlight get 2>/dev/null || echo "50")
        echo "$CURRENT" > "$BRIGHTNESS_FILE"
        ;;
      restore)
        if [ -f "$BRIGHTNESS_FILE" ]; then
          SAVED=$(cat "$BRIGHTNESS_FILE")
          brightnessctl --class=backlight set "$SAVED" 2>/dev/null || true
          rm -f "$BRIGHTNESS_FILE"
        fi
        ;;
      *)
        echo "Usage: brightness-save-restore save|restore" >&2
        exit 1
        ;;
    esac
  '';

  # Auto brightness from ambient light sensor.
  # Dynamically discovers the HID-SENSOR-200041 sensor path.
  # Silently exits if no sensor is present — safe on hardware without one.
  # Keyboard backlight lines use || true and are no-ops on non-ASUS hardware.
  auto-brightness-sensor = pkgs.writeScriptBin "auto-brightness-sensor" ''
    #!${pkgs.bash}/bin/bash

    RAW_FILE=""
    SCALE_FILE=""

    for raw_path in $(find /sys/devices -path "*HID-SENSOR-200041*" -name "in_illuminance_raw" 2>/dev/null); do
      if [ -r "''$raw_path" ]; then
        RAW_FILE="''$raw_path"
        SENSOR_DIR=$(dirname "''$raw_path")
        if [ -r "''$SENSOR_DIR/in_illuminance_scale" ]; then
          SCALE_FILE="''$SENSOR_DIR/in_illuminance_scale"
          break
        fi
      fi
    done

    if [ -z "''$RAW_FILE" ]; then
      RAW_FILE=$(find /sys/devices -name "in_illuminance_raw" 2>/dev/null | head -1)
      if [ -n "''$RAW_FILE" ]; then
        SENSOR_DIR=$(dirname "''$RAW_FILE")
        SCALE_FILE="''$SENSOR_DIR/in_illuminance_scale"
      fi
    fi

    if [ -z "''$RAW_FILE" ] || [ ! -r "''$RAW_FILE" ]; then
      exit 0
    fi

    MANUAL_BRIGHTNESS_FILE="''$HOME/.cache/manual-brightness-time"
    LAST_AUTO_BRIGHTNESS_FILE="''$HOME/.cache/last-auto-brightness"
    COOLDOWN=30

    AUTO_BRIGHTNESS_DISABLED_FILE="/tmp/auto-brightness-disabled"
    if [ -f "''$AUTO_BRIGHTNESS_DISABLED_FILE" ]; then
      exit 0
    fi

    CURRENT_RAW=$(${pkgs.brightnessctl}/bin/brightnessctl --class=backlight get 2>/dev/null || echo "0")
    CURRENT_MAX=$(${pkgs.brightnessctl}/bin/brightnessctl --class=backlight max 2>/dev/null || echo "100")
    CURRENT_PERCENT=$((CURRENT_RAW * 100 / CURRENT_MAX))

    if [ -f "''$MANUAL_BRIGHTNESS_FILE" ]; then
      MANUAL_TIME=$(stat -c %Y "''$MANUAL_BRIGHTNESS_FILE" 2>/dev/null || echo "0")
      NOW=$(date +%s)
      if [ $((NOW - MANUAL_TIME)) -lt ''$COOLDOWN ]; then
        exit 0
      fi
      rm -f "''$MANUAL_BRIGHTNESS_FILE" 2>/dev/null || true
    fi

    RAW=$(cat "''$RAW_FILE" 2>/dev/null || echo "0")
    SCALE=$(cat "''$SCALE_FILE" 2>/dev/null || echo "0.1")
    LUX=$(echo "''$RAW * ''$SCALE" | ${pkgs.bc}/bin/bc -l)

    if (( $(echo "''$LUX < 1" | ${pkgs.bc}/bin/bc -l) )); then
      TARGET_BRIGHTNESS=20
      TARGET_KBD_BRIGHTNESS=1
    elif (( $(echo "''$LUX < 10" | ${pkgs.bc}/bin/bc -l) )); then
      TARGET_BRIGHTNESS=35
      TARGET_KBD_BRIGHTNESS=1
    elif (( $(echo "''$LUX < 50" | ${pkgs.bc}/bin/bc -l) )); then
      TARGET_BRIGHTNESS=50
      TARGET_KBD_BRIGHTNESS=1
    elif (( $(echo "''$LUX < 200" | ${pkgs.bc}/bin/bc -l) )); then
      TARGET_BRIGHTNESS=70
      TARGET_KBD_BRIGHTNESS=0
    elif (( $(echo "''$LUX < 500" | ${pkgs.bc}/bin/bc -l) )); then
      TARGET_BRIGHTNESS=85
      TARGET_KBD_BRIGHTNESS=0
    else
      TARGET_BRIGHTNESS=100
      TARGET_KBD_BRIGHTNESS=0
    fi

    DIFF=$((TARGET_BRIGHTNESS - CURRENT_PERCENT))
    if [ ''${DIFF#-} -lt 5 ]; then
      exit 0
    fi

    ${pkgs.brightnessctl}/bin/brightnessctl --class=backlight set "''$TARGET_BRIGHTNESS%"
    echo "''$TARGET_BRIGHTNESS" > "''$LAST_AUTO_BRIGHTNESS_FILE" 2>/dev/null || true

    # PX13: keyboard backlight (asus::kbd_backlight). No-op on other hardware.
    ${pkgs.brightnessctl}/bin/brightnessctl --class=leds --device=asus::kbd_backlight set "''$TARGET_KBD_BRIGHTNESS" 2>/dev/null || true
  '';

  # Wrapper for manual brightness changes — marks the change so auto-brightness
  # skips overriding it for the 30-second cooldown period.
  brightnessctl-manual = pkgs.writeScriptBin "brightnessctl-manual" ''
    #!${pkgs.bash}/bin/bash
    ${pkgs.brightnessctl}/bin/brightnessctl "$@"
    touch "$HOME/.cache/manual-brightness-time" 2>/dev/null || true
    if echo "$*" | grep -q "backlight"; then
      CURRENT_RAW=$(${pkgs.brightnessctl}/bin/brightnessctl --class=backlight get 2>/dev/null || echo "0")
      CURRENT_MAX=$(${pkgs.brightnessctl}/bin/brightnessctl --class=backlight max 2>/dev/null || echo "100")
      CURRENT_PERCENT=$((CURRENT_RAW * 100 / CURRENT_MAX))
      echo "$CURRENT_PERCENT" > "$HOME/.cache/last-auto-brightness" 2>/dev/null || true
    fi
  '';

  # swayidle: handles auto-dim, lock, and suspend on idle.
  # Uses Noctalia's built-in lock screen via IPC.
  # PX13: kbd_backlight save/restore lines are no-ops on non-ASUS hardware (|| true).
  swayidle-start = pkgs.writeShellScript "swayidle-start" ''
    ${pkgs.swayidle}/bin/swayidle -w \
      timeout 180 '${brightness-save-restore}/bin/brightness-save-restore save && touch /tmp/auto-brightness-disabled && ${pkgs.brightnessctl}/bin/brightnessctl --class=backlight set 10% && KBD_BRIGHTNESS=$(${pkgs.brightnessctl}/bin/brightnessctl --class=leds --device=asus::kbd_backlight get 2>/dev/null || echo "0") && echo "$KBD_BRIGHTNESS" > "$HOME/.cache/niri-kbd-brightness-backup" && ${pkgs.brightnessctl}/bin/brightnessctl --class=leds --device=asus::kbd_backlight set 0 2>/dev/null || true' \
        resume '${brightness-save-restore}/bin/brightness-save-restore restore && rm -f /tmp/auto-brightness-disabled && if [ -f "$HOME/.cache/niri-kbd-brightness-backup" ]; then KBD_BRIGHTNESS=$(cat "$HOME/.cache/niri-kbd-brightness-backup"); ${pkgs.brightnessctl}/bin/brightnessctl --class=leds --device=asus::kbd_backlight set "$KBD_BRIGHTNESS" 2>/dev/null || true; rm -f "$HOME/.cache/niri-kbd-brightness-backup"; fi' \
      timeout 300 '${unstable.noctalia-shell}/bin/noctalia-shell ipc call lockScreen lock' \
      timeout 900 '${brightness-save-restore}/bin/brightness-save-restore save && ${pkgs.systemd}/bin/systemctl suspend' \
        before-sleep '${unstable.noctalia-shell}/bin/noctalia-shell ipc call lockScreen lock'
  '';

  # --- PX13-specific: AMD iGPU wrapper for Niri ---
  # Detects AMD card/render node at runtime and forces Niri to use it exclusively,
  # preventing NVIDIA from initializing and blocking runtime power-down.
  # Re-enable in hosts/px13/ once host-specific layering is in place.
  #
  # niri-amd-wrapper = pkgs.writeScriptBin "niri-session-amd" '' ... '';
  # niri-amd-desktop = pkgs.writeText "niri-amd.desktop" '' ... '';
  # niri-amd-session = pkgs.runCommand "niri-amd-session" { ... } '' ... '';

  # --- PX13-specific: DP-MST output auto-enable ---
  # Enables daisy-chained DisplayPort monitors that Niri detects but doesn't enable by default.
  # Re-enable in hosts/px13/.
  #
  # enable-mst-outputs = pkgs.writeScriptBin "enable-mst-outputs" '' ... '';

  # --- PX13-specific: ASUS DCI P3 ICC color profile ---
  # Applies the ASUS display color profile via colord for accurate DaVinci Resolve color.
  # Re-enable in hosts/px13/.
  #
  # apply-color-profile = pkgs.writeScriptBin "apply-color-profile" '' ... '';

in
{
  environment.systemPackages = with pkgs; [
    niri
    xwayland-satellite
    unstable.noctalia-shell  # 4.7.6 — noctalia-qs bundled, replaces custom derivation
    brightness-save-restore
    brightnessctl-manual
    auto-brightness-sensor
    brightnessctl
    bc
    wlsunset
    wlr-randr
    swayidle
    cava          # audio visualiser — not bundled in nixpkgs noctalia-shell wrapper
    matugen       # material theming — not bundled in nixpkgs noctalia-shell wrapper
    # PX13: niri-amd-wrapper, enable-mst-outputs, apply-color-profile
  ] ++ pkgs.lib.optionals (pkgs.stdenv.hostPlatform.system == "x86_64-linux") [
    gpu-screen-recorder  # screen recording — not bundled in nixpkgs noctalia-shell wrapper
  ];

  # --- PX13-specific: replace stock Niri session with AMD-optimized one ---
  # services.displayManager.sessionPackages = [ niri-amd-session ];

  systemd.user.services.noctalia-shell = {
    description = "Noctalia Shell - Wayland desktop shell";
    wantedBy = [ "graphical-session.target" ];
    after = [ "graphical-session.target" ];
    serviceConfig = {
      # nixpkgs noctalia-shell 4.7.6 is a compiled wrapper that already embeds its
      # runtimeDeps (brightnessctl, cliphist, ddcutil, wlsunset, wl-clipboard, wlr-randr).
      # System PATH covers nmcli and anything else from the broader system.
      ExecStart = "${unstable.noctalia-shell}/bin/noctalia-shell";
      Restart = "on-failure";
      Environment = [
        "NOCTALIA_SETTINGS_FALLBACK=%h/.config/noctalia/gui-settings.json"
        "__GLX_VENDOR_LIBRARY_NAME=mesa"
        "DRI_PRIME=0"
        "__NV_PRIME_RENDER_OFFLOAD=0"
        "__VK_LAYER_NV_optimus="
        "PATH=/run/wrappers/bin:/run/current-system/sw/bin:/run/current-system/sw/sbin:/usr/bin:/usr/sbin:/bin:/sbin"
      ];
      PassEnvironment = [ "PATH" ];
    };
  };

  systemd.user.services.wlsunset = {
    description = "wlsunset - Nightlight (blue light filter) for Wayland";
    wantedBy = [ "graphical-session.target" ];
    after = [ "graphical-session.target" ];
    serviceConfig = {
      Type = "simple";
      ExecStart = "${pkgs.wlsunset}/bin/wlsunset";
      Restart = "on-failure";
      RestartSec = 5;
      PassEnvironment = [ "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" "XDG_SESSION_TYPE" ];
    };
  };

  systemd.user.timers.auto-brightness-sensor = {
    description = "Auto brightness sensor timer";
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnActiveSec = "3s";
      OnUnitActiveSec = "3s";
      AccuracySec = "1s";
    };
  };

  systemd.user.services.auto-brightness-sensor = {
    description = "Auto brightness based on ambient light sensor";
    serviceConfig = {
      ExecStart = "${auto-brightness-sensor}/bin/auto-brightness-sensor";
      Type = "oneshot";
      PassEnvironment = [ "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" "XDG_SESSION_TYPE" ];
      Environment = [
        "PATH=/run/current-system/sw/bin:/run/current-system/sw/sbin:/usr/bin:/usr/sbin:/bin:/sbin"
      ];
    };
  };

  systemd.user.services.swayidle = {
    description = "swayidle - Wayland idle management daemon";
    wantedBy = [ "graphical-session.target" ];
    after = [ "graphical-session.target" ];
    serviceConfig = {
      Type = "simple";
      ExecStart = "${swayidle-start}";
      Restart = "on-failure";
      RestartSec = 5;
      PassEnvironment = [ "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" "XDG_SESSION_TYPE" ];
      Environment = [
        "PATH=/run/current-system/sw/bin:/run/current-system/sw/sbin:/usr/bin:/usr/sbin:/bin:/sbin"
      ];
    };
  };

  # --- PX13-specific: DP-MST services ---
  # systemd.user.services.enable-mst-outputs = { ... };
  # systemd.user.paths.enable-mst-outputs-on-hotplug = { ... };
  # systemd.user.services.enable-mst-outputs-on-hotplug = { ... };

  # --- PX13-specific: ASUS DCI P3 color profile service ---
  # systemd.user.services.apply-color-profile = { ... };
}
