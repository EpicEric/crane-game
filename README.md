# crane-game

## Build for release with Nix

Use the dev shell with `direnv allow`.

```bash
nix-build . -A linux-release
# steam-run ./result/CraneGame_linux_x86_64
zip export/CraneGame_linux_x86_64 result/*

nix-build . -A windows-release
# wine64 ./result/CraneGame_win.exe
zip export/CraneGame_win result/*
```

## Attributions

"Playing Room" background sprite belongs to Texturify.com
