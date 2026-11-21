# Releasing a Project

Releases are triggered by pushing a tag matching `<project>/v*` (for example `logforge-siem/v0.1.0-dev`) in this monorepo, or `v*` after a project is split into its own repository.

## Steps

1. Confirm CI is green on the target commit for Linux x64, macOS arm64, and Windows x64.
2. Update `<project>/CHANGELOG.md` and `<project>/docs/RELEASE_NOTES.md`.
3. Create and push the tag with git:

```bash
git tag <project>/v0.1.0-dev
git push origin <project>/v0.1.0-dev
```

4. Watch the `Release <project>` workflow in GitHub Actions.
5. Verify the pre-release on GitHub has the three expected files (`<project>-linux-x64.tar.gz`, `<project>-macos-arm64.tar.gz`, `<project>-windows-x64.exe`).

## Recovery

If a release needs to be removed and re-tagged:

```bash
gh release delete <project>/v0.1.0-dev --cleanup-tag --yes
```
