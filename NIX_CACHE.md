# Nix Cache Setup

This fork publishes prebuilt Nix outputs the same way as `adampoit/opencode`:

- build on Linux and Darwin in `.github/workflows/fork-ci.yml`
- save the resulting store paths with `adampoit/static-nix-cache/save`
- publish signed cache metadata and nar files to GitHub Pages with `adampoit/static-nix-cache/deploy`

## What gets published

The workflow builds the default package from `./nix` for the current system:

```bash
nix build ./nix#packages.<system>.default
```

Published cache URLs:

- `https://adampoit.github.io/wezterm/linux`
- `https://adampoit.github.io/wezterm/darwin`

## Required repository configuration

Add these before expecting cache publication to work:

- GitHub secret: `NIX_CACHE_SIGNING_KEY`
- GitHub variable: `NIX_CACHE_PUBLIC_KEY`

The public key is added to `nix.conf` during CI so subsequent builds can substitute from the fork cache.

## Publish behavior

- Pull requests to `main` build both Nix packages but do not publish
- Pushes to `sync/integration` build both Nix packages but do not publish
- Pushes to `main` build both Nix packages and publish the cache

That mirrors the `opencode` flow where integration branches are validated first and the promoted branch is what gets published.
