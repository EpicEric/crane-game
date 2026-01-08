let
  sources = import ./npins;
  pkgs = import sources.nixpkgs {
    overlays = [ (import sources.rust-overlay) ];
  };
in
pkgs.mkShell {
  packages = [
    pkgs.bacon
    pkgs.godot_4_5
    pkgs.rust-bin.stable.latest.default
    pkgs.steam-run-free
    pkgs.unzip
    pkgs.wineWowPackages.stable
    pkgs.zip
  ];
}
