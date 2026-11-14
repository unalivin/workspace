# Releasing

A release is a pre-release called "dev beta". The only thing that publishes one is pushing a tag: `certwatch/v*` in the workspace (for example `certwatch/v0.1.0-dev`), plain `v*` in a repository that contains only this project. There is no draft release and no manually started release workflow.

## What the workflow does

`Release certwatch` (`.github/workflows/certwatch-release.yml` in the workspace, `release.yml` in a split repository):

1. On Linux x64, macOS arm64 and Windows x64 runners: runs the tests of the dashboard build, builds and tests the no-GUI variant (it is not attached to the release), then runs `scripts/package.sh`, which builds the dashboard application with the `fyne` packaging tool.
2. In a separate job with `contents: write`, runs `scripts/release-verify.sh`: the tag points at the workflow commit, the tag version equals the version in the source, no release with this tag exists, and exactly the three files are present.
3. Publishes exactly three files as a pre-release titled `CertWatch v0.1.0-dev (dev beta)` with the description from `docs/RELEASE_NOTES.md`:

| File | Content |
|---|---|
| `certwatch-linux-x64.tar.gz` | the application, desktop entry, icon, install `Makefile`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.txt`, `USAGE.md` |
| `certwatch-macos-arm64.tar.gz` | `CertWatch.app`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.txt`, `USAGE.md` |
| `certwatch-windows-x64.exe` | one executable; the license texts and the usage guide are built in |

No checksum file is attached and the workflow never overwrites an existing release.

## Maintainer steps

These work from a desktop or from a phone with Termux (`pkg install git gh`).

1. Confirm that CI is green on the commit you want to release:

   ```bash
   gh run list --workflow "CI certwatch" --limit 3
   ```

2. Update `CHANGELOG.md` and `docs/RELEASE_NOTES.md`. For a new version also change `Version` in `internal/meta/meta.go`. Commit and push.
3. Create the tag with git, not with the GitHub release form:

   ```bash
   git tag certwatch/v0.1.0-dev
   git push origin certwatch/v0.1.0-dev
   ```

4. Watch the run:

   ```bash
   gh run watch
   ```

5. Verify the pre-release:

   ```bash
   gh release view certwatch/v0.1.0-dev
   ```

   It must be marked as a pre-release, have the title `CertWatch v0.1.0-dev (dev beta)` and list exactly three files.

6. If something is wrong, delete the release together with its tag, fix the problem and start again at step 1:

   ```bash
   gh release delete certwatch/v0.1.0-dev --cleanup-tag --yes
   ```

## Checking a download

GitHub shows the SHA-256 digest of each file on the release page. Compare it with the digest of the downloaded file:

```bash
sha256sum certwatch-linux-x64.tar.gz
shasum -a 256 certwatch-macos-arm64.tar.gz
```

```powershell
Get-FileHash certwatch-windows-x64.exe -Algorithm SHA256
```

The files are not signed and not notarized.

## Building the files locally

```bash
make package                 # the file for this operating system, in dist/
scripts/package.sh --dry-run # print the commands
```

`scripts/package.sh` needs the `fyne` tool (`scripts/dev-setup.sh --install`) and the dashboard build requirements.
