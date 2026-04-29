# Host System Pending Changes
**Date:** 2026-04-29  
**Context:** Changes made in SpectreOS26.05 (broader project) that should eventually be applied to the host system config in ~/Documents/SpectreOS.

---

## 1. Switch from GDM + GNOME session to greetd + Niri-only

**What to change in `configuration.nix`:**

Remove:
```nix
services.xserver.enable = true;
services.displayManager.gdm.enable = true;
services.desktopManager.gnome.enable = true;
```

Add:
```nix
services.greetd = {
  enable = true;
  settings = {
    default_session = {
      command = "${pkgs.greetd.tuigreet}/bin/tuigreet --time --remember --greeting 'SpectreOS' --cmd ${pkgs.niri}/bin/niri-session";
      user = "greeter";
    };
  };
};

xdg.portal = {
  enable = true;
  extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
  config.common.default = [ "gtk" ];
};

# Keep this — read by libxkbcommon and Wayland compositors
services.xserver.xkb = {
  layout = "us";
  variant = "";
};
```

**Why:** GDM + `services.desktopManager.gnome.enable` pulls in the full GNOME shell and session even though Niri has been the primary/only session in use. greetd is lightweight, Wayland-native, and has no session overhead. xdg-desktop-portal-gtk replaces the GNOME portal for file pickers, screen sharing, etc.

**Risk:** Low. Niri has been stable on the host. The only visible change is the login screen (tuigreet TUI instead of GDM). Rollback is easy — previous generation still has GDM.

---

## 2. Add standalone GNOME apps to system packages

**What to add to `environment.systemPackages` in `configuration.nix`:**

```nix
# GNOME apps used standalone — no full GNOME session required
snapshot              # Camera
nautilus              # File browser
gnome-calculator      # Calculator
loupe                 # Image viewer
gnome-text-editor     # Text editor
gnome-disk-utility    # Disk/partition manager
gnome-system-monitor  # Task manager / resource monitor
gnome-logs            # Systemd journal viewer
gnome-characters      # Unicode character picker
gnome-font-viewer     # Font preview
gnome-color-manager   # Includes gcm-picker (on-screen color picker)
```

**Why:** These apps were previously available because `services.desktopManager.gnome.enable = true` pulled them in implicitly. Once GNOME is removed, they need to be declared explicitly or they disappear. All work fine as standalone GTK apps under Niri/Wayland.

**Note:** Some of these may eventually be better placed in the home-manager config rather than system packages. For now, system-level is fine.

---

## Apply order

Do (2) before (1) and rebuild once to confirm the apps are explicitly present before removing the GNOME session that was implicitly providing them. Then do (1) and rebuild again.
