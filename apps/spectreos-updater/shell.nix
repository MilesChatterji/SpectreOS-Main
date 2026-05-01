{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    pkg-config
  ];
  buildInputs = with pkgs; [
    gtk4
    glib
  ];
}
