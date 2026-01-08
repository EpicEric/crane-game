let
  sources = import ./npins;
  pkgs = import sources.nixpkgs {
    overlays = [ (import sources.rust-overlay) ];
  };

  inherit (pkgs) lib stdenv;

  pkgs-cross-mingw = import pkgs.path {
    crossSystem = {
      config = "x86_64-w64-mingw32";
    };
  };

  inherit (pkgs-cross-mingw.windows) mingw_w64;
  mingw_w64_cc = pkgs-cross-mingw.stdenv.cc;
  mingw_w64_pthreads_w_static = pkgs-cross-mingw.windows.pthreads.overrideAttrs (oldAttrs: {
    configureFlags = (oldAttrs.configureFlags or [ ]) ++ [
      "--enable-static"
    ];
  });

  rust = pkgs.rust-bin.stable.latest.default;

  rustCc = pkgs.rust-bin.stable.latest.default.override {
    targets = [ "x86_64-pc-windows-gnu" ];
  };

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./godot
      ./rust
    ];
  };

  commonArgs = {
    version = "0";

    inherit src;

    cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
      inherit src;
      sourceRoot = "${src.name}/rust";
      hash = "sha256-kRZdndknulrZFeb7EDwNrpctjaT/UreFwZREOxv7e08=";
    };

    cargoRoot = "rust";

    postPatch = ''
      export HOME=$(mktemp -d)
      mkdir -p ~/.local/share/godot
      ln -s ${pkgs.godot_4_5-export-templates-bin}/share/godot/export_templates ~/.local/share/godot/export_templates
    '';
  };

  commonArgsWin = commonArgs // {
    RUSTFLAGS = (
      map (a: "-L ${a}/lib") [
        mingw_w64
        mingw_w64_pthreads_w_static
      ]
    );
  };
in
{
  linux-release = stdenv.mkDerivation (
    commonArgs
    // {
      pname = "crane-game-linux-release";

      buildInputs = [
        pkgs.rustPlatform.cargoSetupHook
        rust
      ];

      buildPhase = ''
        ${rust}/bin/cargo build --release --manifest-path ./rust/Cargo.toml
        mkdir ./export_linux
        ${pkgs.godot_4_5}/bin/godot --headless --path ./godot --export-release "Linux" ../export_linux/CraneGame_linux_x86_64
      '';

      installPhase = ''
        mkdir $out
        cp ./export_linux/* $out
      '';
    }
  );

  windows-release = stdenv.mkDerivation (
    commonArgsWin
    // {
      pname = "crane-game-windows-release";

      buildInputs = [
        pkgs.rustPlatform.cargoSetupHook
        mingw_w64_cc
        rustCc
      ];

      buildPhase = ''
        ${rustCc}/bin/cargo build --target x86_64-pc-windows-gnu --release --manifest-path ./rust/Cargo.toml
        mkdir ./export_windows
        ${pkgs.godot_4_5}/bin/godot --headless --path ./godot --export-release "Windows Desktop" ../export_windows/CraneGame_win.exe
      '';

      installPhase = ''
        mkdir $out
        cp ./export_windows/* $out
      '';
    }
  );
}
