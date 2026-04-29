# PX13 user account.
# uinput, input, i2c groups are required for ASUS DialPad hardware access (asus-dialpad.nix).

{ pkgs, ... }:

{
  users.users.miles = {
    isNormalUser = true;
    description = "Miles Chatterji";
    extraGroups = [
      "networkmanager"
      "wheel"
      "video"
      "render"
      "libvirtd"
      "uinput"
      "input"
      "i2c"
    ];
    packages = with pkgs; [];
  };
}
