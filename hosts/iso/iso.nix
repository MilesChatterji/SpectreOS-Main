# SpectreOS VM Installer ISO
#
# Builds a bootable ISO that auto-launches the SpectreOS VM installer.
# The installer clones the SpectreOS repo from GitHub at install time,
# so this ISO is always thin and installs the latest version.
#
# Build:
#   nix-build '<nixpkgs/nixos>' -A config.system.build.isoImage \
#     -I nixos-config=hosts/iso/iso.nix
#
# Output: ./result/iso/*.iso
#
# For an offline/production ISO (repo baked in), see hosts/iso/iso-offline.nix.

{ config, pkgs, lib, ... }:

let
  # Root's shell is replaced with this installer launcher.
  # When root logs in (on console or SSH), this script runs.
  # Ctrl+C drops to a real bash shell; success reboots automatically.
  installerShell = pkgs.writeShellScript "spectreos-installer-shell" ''
    echo ""
    echo "  ╔══════════════════════════════════════════════════════════╗"
    echo "  ║                                                          ║"
    echo "  ║                  S P E C T R E  O S                     ║"
    echo "  ║                  VM Installer  (Beta)                   ║"
    echo "  ║                                                          ║"
    echo "  ╚══════════════════════════════════════════════════════════╝"
    echo ""
    echo "  This installer will partition your disk, install SpectreOS,"
    echo "  and reboot automatically when complete."
    echo ""
    echo "  SSH access is available during install:"
    echo "    ssh root@<vm-ip>   password: spectreos"
    echo ""
    echo "  Press Enter to begin, or Ctrl+C to drop to a shell."
    read -r
    bash /etc/spectreos-install.sh
    # Installer exited (error or cancelled) — drop to a real shell.
    exec ${pkgs.bash}/bin/bash --login
  '';
in

{
  imports = [
    <nixpkgs/nixos/modules/installer/cd-dvd/installation-cd-minimal.nix>
  ];

  # Match the kernel version used by the installed system.
  boot.kernelPackages = pkgs.linuxKernel.packages.linux_7_0;

  # ZFS does not support kernel 7 yet; the minimal ISO base enables it by
  # default which causes the build to fail. Disable it — we install on ext4.
  boot.supportedFilesystems.zfs = lib.mkForce false;

  # Tools required by install.sh beyond what the minimal ISO provides.
  environment.systemPackages = with pkgs; [
    git      # clone SpectreOS repo during install
    python3  # patch noctalia settings.json with per-user paths
    parted   # disk partitioning (may already be present; explicit for safety)
    openssl  # password hashing
  ];

  # Networking — disable wpa_supplicant (conflicts with NetworkManager on the ISO)
  networking.wireless.enable = lib.mkForce false;
  networking.networkmanager.enable = true;
  networking.hostName = "spectreos-installer";

  # SSH — lets the install be observed or driven remotely.
  # Root password is intentionally simple; this is a live installer environment only.
  services.openssh = {
    enable = true;
    settings.PermitRootLogin = lib.mkForce "yes";
    settings.PasswordAuthentication = true;
  };
  users.users.root.password = lib.mkForce "spectreos";

  # Bake install.sh into the ISO filesystem.
  # The script itself clones the full repo from GitHub during the install run,
  # so only this small launcher needs to be on the ISO.
  environment.etc."spectreos-install.sh" = {
    source = ../vm/install.sh;
    mode = "0755";
  };

  # Replace root's shell with the installer launcher.
  # This fires on both console auto-login and SSH — no login shell init
  # hooks needed, no race with systemd session setup.
  users.users.root.shell = installerShell;

  # Auto-login root on TTY1.
  services.getty.autologinUser = lib.mkForce "root";

  # ISO metadata
  isoImage.isoName = "spectreos-vm-installer.iso";
  isoImage.volumeID = "SPECTREOS_VM";

  system.stateVersion = "25.11";
}
