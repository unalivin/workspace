# Releasing

## What a release is

Each project is released on its own as a pre-release called "dev beta". The only thing that publishes a release is pushing a tag that matches `<project>/v*`, for example `certwatch/v0.1.0-dev`. There is no draft release and no manually triggered release workflow.

The workflow `.github/workflows/<project>-release.yml`, named `Release <project>`:

1. runs the tests on Linux x64, macOS arm64 and Windows x64, including the no-GUI variant
2. builds the dashboard application on each runner with the packaging tool of the project's language
3. in a separate job with `contents: write`, checks that the tag points at the workflow commit, that no release with this tag exists and that exactly three files are present
4. publishes exactly three files with the description from `<project>/docs/RELEASE_NOTES.md`:

| File | Platform |
|---|---|
| `<project>-linux-x64.tar.gz` | Linux x64 |
| `<project>-macos-arm64.tar.gz` | macOS arm64 |
| `<project>-windows-x64.exe` | Windows x64 |

No checksum file is attached. GitHub shows the SHA-256 digest of every asset on the release page. Compare it with:

```bash
sha256sum certwatch-linux-x64.tar.gz
shasum -a 256 certwatch-macos-arm64.tar.gz
```

```powershell
Get-FileHash certwatch-windows-x64.exe -Algorithm SHA256
```

The binaries are not signed and not notarised.

## Maintainer steps

These steps work from a desktop or from a phone with Termux.

1. Confirm that the project's CI is green on the commit you want to release.
2. Update `<project>/CHANGELOG.md` and `<project>/docs/RELEASE_NOTES.md`, commit and push.
3. Create the tag with git, not with the GitHub release form:

   ```bash
   git tag certwatch/v0.1.0-dev
   git push origin certwatch/v0.1.0-dev
   ```

4. Watch the run:

   ```bash
   gh run watch
   ```

5. Verify the pre-release: three files, marked as pre-release, title `<App name> v0.1.0-dev (dev beta)`.
6. To recover from a bad release, delete it together with its tag and start again:

   ```bash
   gh release delete certwatch/v0.1.0-dev --cleanup-tag --yes
   ```

The workflow never overwrites an existing release.

## Splitting a project into its own repository

```bash
scripts/split-project.sh --dry-run certwatch
scripts/split-project.sh certwatch
```

The script uses `git subtree split` to extract the folder with its history into a new repository directory (by default `../certwatch-standalone`), generates that project's own `.github/workflows/ci.yml` and `release.yml` from `templates/workflows/`, and rewrites repository links to `<owner>/<project>`. The monorepo is left untouched. In a split repository the release tag is plain `v*`, for example `v0.1.0-dev`.

## Publishing the workspace repository

```bash
scripts/init-main.sh --dry-run
scripts/init-main.sh
scripts/publish-github.sh --dry-run
scripts/publish-github.sh
```

`scripts/init-main.sh` creates the local branch `main` as one single commit named `chore: initial public release` that holds the files of the current commit. It carries no earlier history, checks nothing out and pushes nothing.

The script checks the logged-in `gh` user against the owner, refuses a git email that is not a GitHub noreply address, requires the first commit on `main` to be `chore: initial public release`, creates the public repository, sets the description and exactly seven topics, pushes `main`, enables private vulnerability reporting and pushes a tag when `--tag` is given.
