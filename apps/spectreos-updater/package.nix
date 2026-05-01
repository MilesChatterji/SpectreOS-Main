{ lib, rustPlatform, pkg-config, wrapGAppsHook4, gtk4, glib }:

rustPlatform.buildRustPackage {
  pname = "spectreos-updater";
  version = "0.1.0";

  src = builtins.filterSource
    (path: type: !(type == "directory" && builtins.baseNameOf path == "target"))
    ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ pkg-config wrapGAppsHook4 ];
  buildInputs = [ gtk4 glib ];

  postInstall = ''
    install -Dm644 spectreos-updater.desktop \
      $out/share/applications/spectreos-updater.desktop
  '';

  meta.description = "SpectreOS graphical package manager";
  meta.mainProgram = "spectreos-updater";
}
