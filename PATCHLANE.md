# Patchlane Setup

This fork uses `adampoit/patchlane` to rebuild `sync/integration` from upstream `wezterm/main`, apply fork patches, run CI on the generated branch, and then promote the tested commit onto `main`.

## Required GitHub secret

Create `FORK_SYNC_TOKEN` in the fork repository settings.

Use a PAT or GitHub App token with `contents: write` so the sync workflow can push `sync/integration` and still trigger `Fork CI`. The default `GITHUB_TOKEN` will push, but it will not fan out into the follow-up workflow runs needed for promotion.

## Patch branches

Current patch refs, applied in order:

- `patch/fork-ci` for fork-owned workflows and docs, and to remove upstream workflows that would otherwise fan out on fork pushes
- `patch/vim-copy-mode` from wezterm PR #7682
- `patch/font-thickening` from wezterm PR #7683

Local branches were created from the upstream pull request refs with:

```bash
git fetch upstream \
  "+refs/pull/7682/head:refs/heads/patch/vim-copy-mode" \
  "+refs/pull/7683/head:refs/heads/patch/font-thickening"
```

Push them to the fork before running Patchlane:

```bash
git push -u origin patch/fork-ci patch/vim-copy-mode patch/font-thickening
```

## Manual sync

Run `.github/workflows/sync-upstream.yml` with `dry_run: true` first.

Default behavior:

- starts from upstream `main`
- applies `patch/fork-ci`
- applies `patch/vim-copy-mode`
- applies `patch/font-thickening`
- publishes `sync/integration` when `dry_run` is false
- waits for `Fork CI`
- promotes the tested `sync/integration` SHA onto `main`

`patch/fork-ci` carries the fork-owned workflows and docs, and removes upstream workflow files, so promotion does not drop the automation from `main` or trigger upstream release/test workflows on fork pushes.

`release_selector` is left blank by default because upstream's latest published release is much older than current `main`, and these PR patches were validated against `main`.

## Shipping note

This setup gives the fork a repeatable upstream sync plus a packaged Ubuntu artifact on `sync/integration`. Upstream's existing tag/nightly release-upload jobs are still gated to `wezterm/wezterm`, so publishing GitHub Releases from the fork would be a separate follow-up change.
