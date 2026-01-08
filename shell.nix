let
  sources = import ./npins;
  pkgs = import sources.nixpkgs {
    overlays = [ (import sources.rust-overlay) ];
  };

  pkgs-cross-mingw = import pkgs.path {
    crossSystem = {
      config = "x86_64-w64-mingw32";
    };
  };

  mingw_w64_cc = pkgs-cross-mingw.stdenv.cc;
  mingw_w64 = pkgs-cross-mingw.windows.mingw_w64;
  mingw_w64_pthreads_w_static = pkgs-cross-mingw.windows.pthreads.overrideAttrs (oldAttrs: {
    configureFlags = (oldAttrs.configureFlags or [ ]) ++ [
      "--enable-static"
    ];
  });
in
pkgs.mkShell {
  packages = [
    pkgs.bacon
    pkgs.godot_4_5
    pkgs.godot_4_5-export-templates-bin
    mingw_w64_cc
    (pkgs.rust-bin.stable.latest.default.override {
      targets = [ "x86_64-pc-windows-gnu" ];
    })
    pkgs.wineWowPackages.stable
  ];

  WINDOWS_RUSTFLAGS = (
    map (a: ''-L ${a}/lib'') [
      mingw_w64
      mingw_w64_pthreads_w_static
    ]
  );
}
