# SpectreOS base configuration
# Hardware-agnostic. Host-specific modules live in hosts/<hostname>/.
# The installer will generate and inject hardware-configuration.nix per machine.

{ config, pkgs, lib, ... }:
let
  # Custom SpectreOS Plymouth theme
  spectreos-plymouth-theme = pkgs.runCommand "spectreos-plymouth-theme" {
    splashImage = builtins.path {
      path = ./assets/logo.png;
      name = "spectreos-logo.png";
    };
    progressBox = builtins.path {
      path = ./assets/progress_box.png;
      name = "progress_box.png";
    };
    progressBar = builtins.path {
      path = ./assets/progress_bar.png;
      name = "progress_bar.png";
    };
  } ''
    mkdir -p $out/share/plymouth/themes/spectreos

    cp $splashImage $out/share/plymouth/themes/spectreos/logo.png
    cp $progressBox $out/share/plymouth/themes/spectreos/progress_box.png
    cp $progressBar $out/share/plymouth/themes/spectreos/progress_bar.png

    cat > $out/share/plymouth/themes/spectreos/spectreos.plymouth <<EOF
    [Plymouth Theme]
    Name=SpectreOS
    Description=SpectreOS Boot Splash
    ModuleName=script

    [script]
    ImageDir=$out/share/plymouth/themes/spectreos
    ScriptFile=$out/share/plymouth/themes/spectreos/spectreos.script
    EOF

    cat > $out/share/plymouth/themes/spectreos/spectreos.script <<'SCRIPT'
    # SpectreOS Plymouth Theme Script

    Window.SetBackgroundTopColor(0.00, 0.00, 0.00);
    Window.SetBackgroundBottomColor(0.00, 0.00, 0.00);

    logo.image = Image("logo.png");
    logo.sprite = Sprite(logo.image);

    progress_box.image = Image("progress_box.png");
    progress_box.sprite = Sprite(progress_box.image);

    progress_bar.original_image = Image("progress_bar.png");
    progress_bar.image = progress_bar.original_image;
    progress_bar.sprite = Sprite(progress_bar.image);

    logo.sprite.SetOpacity(1);
    progress_box.sprite.SetOpacity(1);
    progress_bar.sprite.SetOpacity(1);

    fun refresh_callback() {
      screen_width = Window.GetWidth();
      screen_height = Window.GetHeight();

      if (screen_width > 0 && screen_height > 0) {
        logo_width = logo.image.GetWidth();
        logo_height = logo.image.GetHeight();
        logo_x = Window.GetX() + (screen_width - logo_width) / 2;
        logo_y = Window.GetY() + (screen_height - logo_height) / 2 - 100;
        logo.sprite.SetPosition(logo_x, logo_y, 1000);
        logo.sprite.SetOpacity(1);

        progress_box_width = progress_box.image.GetWidth();
        progress_box_height = progress_box.image.GetHeight();
        progress_box_x = Window.GetX() + (screen_width - progress_box_width) / 2;
        progress_box_y = Window.GetY() + screen_height - progress_box_height - 100;

        progress_box.x = progress_box_x;
        progress_box.y = progress_box_y;

        progress_box.sprite.SetPosition(progress_box_x, progress_box_y, 2000);
        progress_box.sprite.SetOpacity(1);

        current_bar_width = progress_bar.image.GetWidth();
        current_bar_height = progress_bar.image.GetHeight();
        box_padding_x = (progress_box_width - current_bar_width) / 2;
        box_padding_y = (progress_box_height - current_bar_height) / 2;
        progress_bar_x = progress_box_x + box_padding_x;
        progress_bar_y = progress_box_y + box_padding_y;
        progress_bar.sprite.SetPosition(progress_bar_x, progress_bar_y, 3000);
        progress_bar.sprite.SetOpacity(1);
      } else {
        logo.sprite.SetPosition(100, 100, 1000);
        logo.sprite.SetOpacity(1);
        progress_box.sprite.SetPosition(100, 600, 2000);
        progress_box.sprite.SetOpacity(1);
        progress_bar.sprite.SetPosition(100, 650, 3000);
        progress_bar.sprite.SetOpacity(1);
      }
    }

    Plymouth.SetRefreshFunction(refresh_callback);
    refresh_callback();

    fun OnDisplayInit() {
      refresh_callback();
    }

    fun OnBootProgress(duration, progress) {
      new_width = Math.Int(progress_bar.original_image.GetWidth() * progress);

      if (new_width < 1) {
        new_width = 1;
      }

      progress_bar.image = progress_bar.original_image.Scale(new_width, progress_bar.original_image.GetHeight());
      progress_bar.sprite.SetImage(progress_bar.image);

      screen_width = Window.GetWidth();
      screen_height = Window.GetHeight();
      if (screen_width > 0 && screen_height > 0) {
        progress_box_width = progress_box.image.GetWidth();
        progress_box_height = progress_box.image.GetHeight();
        progress_box_x = Window.GetX() + (screen_width - progress_box_width) / 2;
        progress_box_y = Window.GetY() + screen_height - progress_box_height - 100;

        current_bar_width = progress_bar.image.GetWidth();
        current_bar_height = progress_bar.image.GetHeight();
        box_padding_x = (progress_box_width - current_bar_width) / 2;
        box_padding_y = (progress_box_height - current_bar_height) / 2;
        progress_bar_x = progress_box_x + box_padding_x;
        progress_bar_y = progress_box_y + box_padding_y;
        progress_bar.sprite.SetPosition(progress_bar_x, progress_bar_y, 3000);
      }
    }

    Plymouth.SetBootProgressFunction(OnBootProgress);

    fun OnQuit() {
      logo.sprite.SetOpacity(0);
      progress_box.sprite.SetOpacity(0);
      progress_bar.sprite.SetOpacity(0);
    }
    SCRIPT
  '';
in

{
  imports = [
    # hardware-configuration.nix is host-specific and generated by the installer.
    # For PX13: ./hosts/px13/hardware-configuration.nix
    # TODO: installer will inject the correct path here per machine.

    # Niri WM + Noctalia Shell
    ./modules/desktop/niri.nix

    # --- PX13-specific modules (commented out for base config) ---
    # ./hosts/px13/asus-dialpad.nix
    # ./hosts/px13/gpu-offload.nix
  ];

  # Bootloader
  boot.loader.systemd-boot.enable = true;
  boot.loader.systemd-boot.consoleMode = "auto";
  boot.loader.efi.canTouchEfiVariables = true;

  # SpectreOS Plymouth boot splash
  boot.plymouth = {
    enable = true;
    theme = "spectreos";
    themePackages = [ spectreos-plymouth-theme ];
  };

  # Kernel 7 — default on NixOS 25.11 is 6.12; we track 7 early.
  boot.kernelPackages = pkgs.linuxKernel.packages.linux_7_0;

  # Firmware updates
  services.fwupd.enable = true;
  # hardware.enableAllFirmware = true;  # PX13-specific (includes AMD/Qualcomm blobs)

  # Power management
  services.power-profiles-daemon.enable = true;

  services.logind = {
    settings = {
      Login = {
        HandlePowerKey = "suspend";
        HandleSuspendKey = "suspend";
        HandleHibernateKey = "hibernate";
        HandleLidSwitch = "suspend";
        HandleLidSwitchExternalPower = "ignore";
      };
    };
  };

  # Flatpak for third-party software
  services.flatpak.enable = true;

  # Virtualisation (KVM/QEMU)
  virtualisation.libvirtd = {
    enable = true;
    qemu.package = pkgs.qemu_kvm;
  };

  # Generic hostname — the installer will set this per machine.
  # PX13: networking.hostName = "PX13";
  networking.hostName = "spectreos";

  networking.networkmanager.enable = true;

  time.timeZone = "America/Los_Angeles";

  i18n.defaultLocale = "en_US.UTF-8";
  i18n.extraLocaleSettings = {
    LC_ADDRESS = "en_US.UTF-8";
    LC_IDENTIFICATION = "en_US.UTF-8";
    LC_MEASUREMENT = "en_US.UTF-8";
    LC_MONETARY = "en_US.UTF-8";
    LC_NAME = "en_US.UTF-8";
    LC_NUMERIC = "en_US.UTF-8";
    LC_PAPER = "en_US.UTF-8";
    LC_TELEPHONE = "en_US.UTF-8";
    LC_TIME = "en_US.UTF-8";
  };

  # greetd — lightweight Wayland-native display manager
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.greetd.tuigreet}/bin/tuigreet --time --remember --greeting 'SpectreOS' --cmd ${pkgs.niri}/bin/niri-session";
        user = "greeter";
      };
    };
  };

  # XDG desktop portals — file pickers, screen sharing, etc. for Wayland apps
  xdg.portal = {
    enable = true;
    extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
    config.common.default = [ "gtk" ];
  };

  # Keyboard layout — read by libxkbcommon and Wayland compositors
  services.xserver.xkb = {
    layout = "us";
    variant = "";
  };

  services.printing.enable = true;

  # Color management (ICC profiles)
  services.colord.enable = true;

  # Audio via PipeWire
  services.pulseaudio.enable = false;
  security.rtkit.enable = true;
  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };

  # Default user account — the installer will replace this with the user's chosen name.
  # PX13: extraGroups includes "uinput" "input" "i2c" for ASUS DialPad hardware access.
  users.users.miles = {
    isNormalUser = true;
    description = "Miles Chatterji";
    extraGroups = [
      "networkmanager"
      "wheel"
      "video"
      "render"
      "libvirtd"
      # "uinput" "input" "i2c"  # PX13: required for ASUS DialPad
    ];
    packages = with pkgs; [];
  };

  programs.firefox.enable = true;

  programs.zsh.enable = true;
  users.defaultUserShell = pkgs.zsh;

  nixpkgs.config.allowUnfree = true;

  nix.settings.experimental-features = [ "nix-command" "flakes" ];

  environment.systemPackages = with pkgs; [
    neovim   # TTY/recovery editor
    git      # TTY/recovery version control
    wget     # TTY/recovery downloads
    fwupd    # Firmware updates
    busybox  # System utilities
    lshw     # Hardware info
    colord   # ICC color profile management
    # nvtopPackages.full  # PX13: NVIDIA/AMD GPU monitor

    # GNOME apps used standalone outside the full GNOME session
    # TODO: evaluate moving any of these to home-manager
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
  ];

  # --- PX13-specific: NVIDIA gpu-screen-recorder capability wrapper ---
  # Requires gpu-offload.nix (NVIDIA drivers). Re-enable in hosts/px13/.
  # security.wrappers.gsr-kms-server = {
  #   source = "${pkgs.gpu-screen-recorder}/bin/gsr-kms-server";
  #   owner = "root";
  #   group = "root";
  #   capabilities = "cap_sys_admin+ep";
  # };

  networking.firewall = {
    allowedTCPPorts = [];
    allowedUDPPorts = [];
    # PX13/user-specific: Dropbox uses ports 17500 TCP+UDP
    # allowedTCPPorts = [ 17500 ];
    # allowedUDPPorts = [ 17500 ];
  };

  # SpectreOS identity — overrides NixOS defaults in /etc/os-release and /etc/lsb-release.
  # ID_LIKE=nixos preserves NixOS tooling compatibility.
  environment.etc."os-release".text = lib.mkForce ''
    NAME="SpectreOS"
    PRETTY_NAME="SpectreOS 0.1 (Beta)"
    ID=spectreos
    ID_LIKE=nixos
    VERSION="0.1"
    VERSION_ID="0.1"
    VERSION_CODENAME=beta
    LOGO="nix-snowflake"
    ANSI_COLOR="0;38;2;126;186;228"
  '';

  environment.etc."lsb-release".text = lib.mkForce ''
    DISTRIB_ID=SpectreOS
    DISTRIB_RELEASE=0.1
    DISTRIB_CODENAME=beta
    DISTRIB_DESCRIPTION="SpectreOS 0.1 (Beta)"
    LSB_VERSION=0.1
  '';

  system.stateVersion = "25.11";
}
