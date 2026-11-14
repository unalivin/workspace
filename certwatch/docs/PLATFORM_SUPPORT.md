# Platform support

## Targets

| Platform | Release file | Built and tested by CI on |
|---|---|---|
| Linux x86-64 | `certwatch-linux-x64.tar.gz` | `ubuntu-latest` |
| macOS on Apple silicon | `certwatch-macos-arm64.tar.gz` | `macos-latest` |
| Windows x86-64 | `certwatch-windows-x64.exe` | `windows-latest` |

Other platforms that Go and the window toolkit support may work when built from source. They are not tested.

## What the automated checks cover

On every push, CI runs on each of the three runners: formatting, `go vet`, `staticcheck`, the tests of the dashboard build and of the no-GUI build, the tests under the race detector, both builds, `govulncheck`, a headless rendering of the dashboard screens and the packaging step. The shell script checks and the checks of generated files run on Linux.

A platform counts as verified for a commit only when the CI run of that commit is green on that runner. The status of the latest run is on the Actions page of the repository; this page does not repeat it.

| Area | How it is tested | Not covered |
|---|---|---|
| Service layer, database, commands | Unit and integration tests on all three runners | |
| TLS checks | Tests start local TLS servers with certificates from a test authority: trusted, untrusted, expired, self-signed, wrong name, weak key, old protocol versions, a stalled handshake and a server that does not speak TLS | Servers on the internet; validation by the trust store of the operating system (the tests pass their own roots) |
| Terminal UI | Tests drive the model with key presses and compare the rendered screen | A real terminal emulator; mouse input is not used |
| Dashboard | Tests drive the real widgets through the toolkit's headless test driver and render screenshots | A real window on a real display, real pointer and keyboard input, high-DPI scaling, screen readers |
| Launcher decision | Table-driven tests for desktop, terminal, headless, CI and piped sessions on each operating system | The native chooser on a real desktop |
| Paths and permissions | Tests with injected environments for Linux, macOS and Windows; real file modes on the runner | |
| Start at sign-in | Install, uninstall and status against a temporary home directory for all three layouts | That the operating system starts the entry at a real sign-in |
| System notifications | The command line of the helper is tested with an injected runner | That the desktop displays the notification |
| Opening the browser and a terminal window | The command lines are tested with an injected runner | That the helper programs start on a real desktop |
| Application icon | PNG, ICO and ICNS files are generated and parsed back by tests | How each operating system renders them |
| Release packaging | CI runs the packaging step and checks the file names | Starting the packaged application on a clean machine |

## Known differences and limits

- **Chain validation**: with the default trust store, Linux validates inside the application. macOS and Windows hand validation to the operating system, which uses its own trust store and may fetch a missing intermediate certificate or revocation data. The same server can therefore validate on one system and fail on another. This path is not covered by the automated tests.
- **Trusted roots on Linux** come from the certificate bundle of the distribution. A minimal container without that bundle trusts nothing until `checks.trust_file` is set.
- **TLS 1.0 and 1.1 probing** uses the Go TLS library, which still offers these versions to a server when asked. SSL 3.0 and older cannot be probed.
- **Window position** is not saved or restored on any platform; the window toolkit does not report it. Size and sidebar state are saved.
- **Animations**: packaged builds are compiled without the toolkit's animations. The reduced motion setting controls the fade of the notice bar.
- **Windows console**: the release executable is a window application. Started from a console it attaches to that console for output; the prompt returns before the output is complete. Started by double-click with a remembered Terminal choice it opens its own console window. This console handling has no automated test.
- **macOS terminal relaunch**: Open terminal UI in the dashboard asks Terminal.app to run the application through `osascript`. Other terminal applications are not used.
- **Linux terminal relaunch**: tries `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `alacritty`, `kitty`, `foot` and `xterm` in this order.
- **Linux notifications** need `notify-send`. macOS uses `osascript`, Windows uses PowerShell.
- **Wayland and X11**: the dashboard is built with support for both. Neither is exercised with a real compositor by the tests.
- **Unsigned builds**: see [INSTALLATION.md](INSTALLATION.md).
