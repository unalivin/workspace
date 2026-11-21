# Platform Support

LogForge-SIEM targets three desktop platforms:

- Linux x64
- macOS arm64
- Windows x64

## Build and test matrix

Every release builds the GUI binary on all three platforms and runs the test suite on each runner before packaging. The no-GUI build is also compiled and tested in CI.

## Notes

- Binaries are unsigned in `0.1.0-dev`. See `docs/RELEASE_NOTES.md` for how to open unsigned binaries on macOS and Windows.
- Desktop notifications use per-user autostart entries without administrator rights.
