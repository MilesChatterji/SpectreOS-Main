# SpectreOS PX13 host — ASUS ProArt PX13 (AMD Ryzen AI 9 HX 370 / Radeon 890M + NVIDIA)
#
# Entry point for the live host system in ~/Documents/SpectreOS.
# See notes/HOST_SYSTEM_PENDING_CHANGES.md for pending migration steps.

{ config, pkgs, lib, ... }:

{
  imports = [
    ./hardware-configuration.nix
    ../../configuration.nix
    ./gpu-offload.nix
    ./asus-dialpad.nix
    ./user.nix
  ];

  networking.hostName = "PX13";

  # AMD/Qualcomm firmware blobs required for full PX13 hardware support.
  hardware.enableAllFirmware = true;

  environment.systemPackages = with pkgs; [
    nvtopPackages.full  # GPU monitor — AMD iGPU + NVIDIA
  ];

  # NVIDIA gpu-screen-recorder capability wrapper.
  # Requires gpu-offload.nix Phase 2 (NVIDIA drivers). Re-enable then.
  # security.wrappers.gsr-kms-server = {
  #   source = "${pkgs.gpu-screen-recorder}/bin/gsr-kms-server";
  #   owner = "root";
  #   group = "root";
  #   capabilities = "cap_sys_admin+ep";
  # };

  # Dropbox uses ports 17500 TCP+UDP for LAN sync.
  # networking.firewall.allowedTCPPorts = [ 17500 ];
  # networking.firewall.allowedUDPPorts = [ 17500 ];
}
