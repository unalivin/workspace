# Installation

## Downloads

Pre-releases are on the [releases page](https://github.com/unalivin/workspace/releases). Each one has exactly three files:

| File | Platform | Content |
|---|---|---|
| `templateapp-linux-x64.tar.gz` | Linux x86-64 | The binary, a desktop entry, the icon, an install `Makefile`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.txt`, `USAGE.md` |
| `templateapp-macos-arm64.tar.gz` | macOS on Apple silicon | `Template App.app`, `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.txt`, `USAGE.md` |
| `templateapp-windows-x64.exe` | Windows x86-64 | One executable. `templateapp licenses` and `templateapp guide` print the license texts and the usage guide that are built into it. |

There is no installer and no automatic update.

## Check the digest

GitHub shows the SHA-256 digest of every file on the release page. Compute the digest of your download and compare the two:

```bash
sha256sum templateapp-linux-x64.tar.gz
shasum -a 256 templateapp-macos-arm64.tar.gz
```

```powershell
Get-FileHash templateapp-windows-x64.exe -Algorithm SHA256
```

## Open an unsigned build

The files are not signed and not notarized, so the operating system warns before the first start.

- **macOS**: right-click `Template App.app`, choose Open, then Open again in the dialog.
- **Windows**: in the SmartScreen dialog choose More info, then Run anyway.
- **Linux**: if the binary lost its executable bit, run `chmod +x usr/local/bin/templateapp` inside the unpacked folder.

## Linux

```bash
tar -xzf templateapp-linux-x64.tar.gz
cd templateapp-linux-x64
make user-install
```

`make user-install` copies the binary to `~/.local/bin`, the desktop entry to `~/.local/share/applications` and the icon to `~/.local/share/icons`. It needs no administrator rights. `sudo make install` installs for every user, and `make user-uninstall` or `sudo make uninstall` removes the files again.

To try the application without installing, run `usr/local/bin/templateapp` from the unpacked folder.

The dashboard needs a desktop session with OpenGL. On a server or over SSH the application opens the terminal UI instead.

## macOS

Unpack the archive and move `Template App.app` to Applications. For the commands, call the binary inside the bundle:

```bash
"/Applications/Template App.app/Contents/MacOS/templateapp" --help
```

## Windows

Put `templateapp-windows-x64.exe` where you want to keep it and start it. From PowerShell or the command prompt the same file runs the commands:

```powershell
.\templateapp-windows-x64.exe records list
```

The executable is built as a window application, so the prompt returns before the output is complete. Add `| more` or redirect to a file when a script needs to wait for the output.

## Where your data is stored

| | Database | Saved database path, interface choice and notifier position |
|---|---|---|
| Linux | `~/.local/share/templateapp/templateapp.db` | `~/.config/templateapp/` |
| macOS | `~/Library/Application Support/templateapp/templateapp.db` | `~/Library/Application Support/templateapp/` |
| Windows | `%LocalAppData%\templateapp\templateapp.db` | `%AppData%\templateapp\` |

On Linux `XDG_DATA_HOME` and `XDG_CONFIG_HOME` are honored. `templateapp db path` prints the database in use. Change the location in Settings, Data, Database location or with `templateapp db move`.

## Remove the application

Delete the files you installed, then delete the two locations in the table above. If you turned on Start at sign-in, turn it off first (Settings, Notifications, or `templateapp notifier uninstall`).

## Build from source

Requirements: Go 1.27.4 with cgo and a C compiler. The dashboard also needs OpenGL and windowing headers:

- **Debian or Ubuntu**: `gcc pkg-config libgl1-mesa-dev xorg-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev xz-utils`
- **macOS**: the Xcode command line tools
- **Windows**: a MinGW-w64 `gcc` on `PATH`, for example from MSYS2

```bash
scripts/dev-setup.sh            # report what is missing
scripts/dev-setup.sh --install  # install it (apt packages use sudo)
make build                      # dist/bin/templateapp with the dashboard
make build-nogui                # dist/bin/templateapp-nogui: terminal UI and commands only
make package                    # the release file for this operating system in dist/
```

The no-GUI build needs only Go and a C compiler. It is meant for servers: `--ui gui` reports that this build has no dashboard.
