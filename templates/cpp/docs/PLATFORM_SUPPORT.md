# Platform support

## Targets

| Platform | Release file | Built and tested by CI on |
|---|---|---|
| Linux x86-64 | `templateapp-linux-x64.tar.gz` | `ubuntu-latest` |
| macOS on Apple silicon | `templateapp-macos-arm64.tar.gz` | `macos-latest` |
| Windows x86-64 | `templateapp-windows-x64.exe` | `windows-latest` |

| Compiler | Where |
|---|---|
| GCC | every build and test on the Linux runner |
| Clang | the libFuzzer build on the Linux runner; `clang-tidy` reads every file with the Clang front end |
| Apple Clang | every build and test on the macOS runner |
| MSVC | every build and test on the Windows runner |

Other platforms with a C++20 compiler, OpenGL 3 and GLFW support may work when built from source. They are not tested.

## What the automated checks cover

On every push, CI runs on each of the three runners: `clang-format` in check mode, `cppcheck` on every file, the tests of the dashboard build and of the no-GUI build, the sanitizer run (`make race-or-sanitize`), both release builds, the checks of generated files, a headless rendering of the dashboard screens and the packaging step. `clang-tidy` runs on Linux and macOS. The supply chain checks (`make vuln`), the libFuzzer run and the shell script checks run on Linux. Every compiler runs with its warnings as errors.

The sanitizer run uses AddressSanitizer with UndefinedBehaviorSanitizer and then ThreadSanitizer on Linux and macOS. On Windows it runs the tests as an optimized build with assertions and checked containers instead; [TESTING.md](TESTING.md) explains the difference.

A platform counts as verified for a commit only when the CI run of that commit is green on that runner. The status of the latest run is on the Actions page of the repository; this page does not repeat it.

| Area | How it is tested | Not covered |
|---|---|---|
| Service layer, database, commands | Unit and integration tests on all three runners | |
| Terminal UI | Tests drive the model with key presses and compare the rendered screen | A real terminal emulator; mouse input is not used |
| Dashboard | Tests feed pointer and key events to the real interface code without a window and render every main screen with a software renderer that is part of the project | A real window on a real display, the graphics driver, real pointer and keyboard input, high-DPI scaling, screen readers |
| Launcher decision | Table-driven tests for desktop, terminal, headless, CI and piped sessions on each operating system | The native chooser on a real desktop |
| Paths and permissions | Tests with injected environments for Linux, macOS and Windows; real file modes on the runner | |
| Start at sign-in | Install, uninstall and status against a temporary home directory for all three layouts | That the operating system starts the entry at a real sign-in |
| System notifications | The command line of the helper is tested with an injected runner | That the desktop displays the notification |
| Opening the browser and a terminal window | The command lines are tested with an injected runner | That the helper programs start on a real desktop |
| Application icon | PNG, ICO and ICNS files are generated and parsed back by tests | How each operating system renders them |
| Release packaging | CI runs the packaging step and checks the file names | Starting the packaged application on a clean machine |

## Known differences and limits

- **The real window has not been opened by an automated check on any platform.** The dashboard tests and the screenshots use a software renderer; the window host, the event loop and the graphics backend run only when a person starts the application.
- **Window position** is not saved or restored on any platform. Size and sidebar state are saved.
- **Animations**: the fade of the notice bar is the only one; the reduced motion setting turns it off.
- **Windows console**: the executable is a console application, so commands print to the console they were started from. Started by double-click, Windows opens a console window, and the application lets it go when the dashboard opens and no other program shares it. How long the console is visible on a real desktop has not been verified.
- **macOS Quit**: closing the window ends the application. Whether Quit from the application menu (Cmd+Q) returns through the same path, so that the window size is saved, has not been verified.
- **macOS terminal relaunch**: Open terminal UI in the dashboard asks Terminal.app to run the application through `osascript`. Other terminal applications are not used.
- **Linux terminal relaunch**: tries `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `alacritty`, `kitty`, `foot` and `xterm` in this order.
- **Linux notifications** need `notify-send`. macOS uses `osascript`, Windows uses PowerShell.
- **Wayland and X11**: the dashboard is built for X11. On a Wayland desktop it runs through XWayland; without XWayland the window cannot be opened and the application says so. Neither is exercised with a real display server by the tests.
- **Graphics**: the window needs OpenGL 3.0 (3.2 core profile on macOS). macOS has deprecated OpenGL; it still ships with the system.
- **High-DPI**: the window scales the interface by the content scale that the operating system reports and follows a change, for example when the window moves to another monitor. This has not been checked on a real high-DPI screen.
- **Unsigned builds**: see [INSTALLATION.md](INSTALLATION.md).
