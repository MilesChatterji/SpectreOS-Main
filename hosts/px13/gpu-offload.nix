# GPU configuration for PX13 (AMD Ryzen AI 9 HX 370 / Radeon 890M + NVIDIA discrete)
#
# Phase 1 (current): iGPU only — AMD 890M active, NVIDIA fully commented out.
#   Test VM installation with a single virtualised GPU first.
#
# Phase 2: Re-enable NVIDIA block below after a clean base install is confirmed,
#   then test GPU offload with a virtualised NVIDIA to validate the wrapper approach
#   before enabling on real hardware.
#
# NVIDIA driver note (2025-02-07): driver was pulled from nixos-unstable (580.126.18+)
# because stable 25.11 had 580.119.02 which does not build on kernel 6.19/7.
# When stable has 580.126.18+, revert package to:
#   config.boot.kernelPackages.nvidiaPackages.production

{ config, pkgs, ... }:

# --- NVIDIA: let block (re-enable for Phase 2) ---
# let
#   unstableSrc = builtins.fetchTarball {
#     url = "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz";
#   };
#   unstable = import unstableSrc {
#     config.allowUnfree = true;
#     inherit (config.nixpkgs) system;
#   };
#   ourKernel = config.boot.kernelPackages.kernel;
#   nvidiaX11Path = unstableSrc + "/pkgs/os-specific/linux/nvidia-x11";
#   unstableWithOurKernel = unstable // {
#     kernel = ourKernel;
#     callPackage = fn: args:
#       let
#         inject = if builtins.isFunction fn && builtins.functionArgs fn ? kernel then { kernel = ourKernel; } else { };
#         passOurCallPackage = if fn == nvidiaX11Path then { callPackage = unstableWithOurKernel.callPackage; } else { };
#         args' = args // inject // passOurCallPackage;
#       in
#       unstable.callPackage fn args';
#   };
#   unstableNvidiaPackages = unstableWithOurKernel.callPackage (unstableSrc + "/pkgs/os-specific/linux/nvidia-x11") { };
#
#   # nvidia-offload: run an app on NVIDIA GPU. Usage: nvidia-offload <app>
#   nvidia-offload = pkgs.writeScriptBin "nvidia-offload" ''
#     #!${pkgs.bash}/bin/bash
#     if [ -n "$XDG_RUNTIME_DIR" ]; then
#       export XDG_RUNTIME_DIR
#       export PIPEWIRE_RUNTIME_DIR="$XDG_RUNTIME_DIR"
#       export PULSE_RUNTIME_PATH="$XDG_RUNTIME_DIR/pulse"
#       export PULSE_SERVER="unix:$XDG_RUNTIME_DIR/pulse/native"
#     fi
#     export ALSA_PCM_NAME=pulse
#     export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"
#     if [ -z "$LD_LIBRARY_PATH" ]; then
#       export LD_LIBRARY_PATH="${pkgs.alsa-plugins}/lib"
#     else
#       export LD_LIBRARY_PATH="${pkgs.alsa-plugins}/lib:$LD_LIBRARY_PATH"
#     fi
#     export __NV_PRIME_RENDER_OFFLOAD=1
#     export __GLX_VENDOR_LIBRARY_NAME=nvidia
#     export __VK_LAYER_NV_optimus=NVIDIA_only
#     export DRI_PRIME=1
#     export GBM_BACKEND=nvidia-drm
#     unset __EGL_VENDOR_LIBRARY_FILENAMES
#     unset VK_ICD_FILENAMES
#     exec "$@"
#   '';
#
#   # amd-only: force an app to use AMD iGPU. Useful for Electron apps.
#   amd-only = pkgs.writeScriptBin "amd-only" ''
#     #!${pkgs.bash}/bin/bash
#     export __GLX_VENDOR_LIBRARY_NAME=mesa
#     export DRI_PRIME=0
#     export __NV_PRIME_RENDER_OFFLOAD=0
#     export __VK_LAYER_NV_optimus=
#     export MESA_LOADER_DRIVER_OVERRIDE=radeonsi
#     export LIBGL_ALWAYS_SOFTWARE=0
#     export ELECTRON_DISABLE_SANDBOX=1
#     export ELECTRON_USE_ANGLE=0
#     export CHROMIUM_FLAGS="--use-gl=egl --disable-gpu-sandbox"
#     exec "$@"
#   '';
# in

{
  # Mesa / Vulkan / DRI — needed for AMD iGPU and any hardware acceleration.
  # Declared here rather than base configuration.nix since this is the PX13 GPU module.
  hardware.graphics = {
    enable = true;
    enable32Bit = true;
  };

  # Session defaults: AMD iGPU first, NVIDIA only via explicit wrapper.
  # Safe to keep active even without NVIDIA drivers loaded.
  environment.sessionVariables = {
    __GLX_VENDOR_LIBRARY_NAME = "mesa";
    DRI_PRIME = "0";
    __NV_PRIME_RENDER_OFFLOAD = "0";
    __VK_LAYER_NV_optimus = "";
  };

  # --- NVIDIA: re-enable below for Phase 2 ---

  # services.xserver.videoDrivers = [ "nvidia" ];

  # hardware.nvidia = {
  #   open = false;
  #   modesetting.enable = false;  # Keep false: forces Niri onto AMD iGPU
  #   powerManagement.enable = true;
  #   nvidiaSettings = true;
  #   package = unstableNvidiaPackages.production;
  # };

  # environment.systemPackages = [ nvidia-offload amd-only ];

  # RTD3 runtime power management for NVIDIA (replaces finegrained option
  # which requires explicit PCI bus IDs via hardware.nvidia.prime).
  # boot.extraModprobeConfig = ''
  #   options nvidia NVreg_DynamicPowerManagement=0x02
  # '';

  # udev rules: allow NVIDIA PCI devices to runtime-suspend when idle.
  # services.udev.extraRules = ''
  #   ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x030000", TEST=="power/control", ATTR{power/control}="auto"
  #   ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x030200", TEST=="power/control", ATTR{power/control}="auto"
  #   ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x040300", TEST=="power/control", ATTR{power/control}="auto"
  #   ACTION=="add", SUBSYSTEM=="pci", ATTR{vendor}=="0x10de", ATTR{class}=="0x0c0330", TEST=="power/control", ATTR{power/control}="auto"
  # '';
}
