# Language profiles

Every project has one primary language. SQL and Bash are supporting languages in every project. Python is never part of a product.

| Language | Projects | Template | State of the template |
|---|---|---|---|
| Go | 14 | `templates/go` | Present. Checked by the workflow `CI template-go` on Linux, macOS and Windows. |
| Rust | 8 | `templates/rust` | Present. Checked by the workflow `CI template-rust` on Linux, macOS and Windows. |
| C++ | 3 | `templates/cpp` | Not written yet |

`scripts/new-project.sh` refuses a language whose template folder does not exist.

## Go

| | |
|---|---|
| Toolchain | Go `1.27.4`, cgo with a C compiler |
| Layout | `cmd/`, `internal/`, `migrations/`, `tests/`, `testdata/`, `scripts/` |
| Dashboard | Fyne `v2.8.2`, with the toolkit look replaced by the template's design tokens |
| Terminal UI | Bubble Tea `v1.3.10` with Lip Gloss `v1.1.0` |
| SQLite | `github.com/mattn/go-sqlite3` `v1.14.53`, built with the `sqlite_fts5` tag |
| Build with dashboard | `make build` (tags `sqlite_fts5 no_animations`) |
| Build without dashboard | `make build-nogui` (tags `sqlite_fts5 nogui`) |
| Format and lint | `gofmt`, `go vet`, `staticcheck` `v0.8.2` |
| Tests | `go test`, `go test -race`, native fuzzing through `make fuzz` |
| Supply chain | `govulncheck` `v1.9.0`; license review by `make notices` |
| Packaging | `fyne package` from `fyne.io/tools/cmd/fyne` `v1.7.4`, wrapped by `scripts/package.sh` |
| Scripts | `shellcheck` `0.11.0.1`, `shfmt` `4.2.0`, `bats` `1.13.0` |

System packages for the dashboard build:

- Debian or Ubuntu: `gcc pkg-config libgl1-mesa-dev xorg-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev xz-utils`
- macOS: the Xcode command line tools
- Windows: a MinGW-w64 `gcc` on `PATH`

`templates/go/scripts/dev-setup.sh` reports what is missing and `--install` installs it. The versions above are defined in `templates/go/go.mod` and `templates/go/scripts/lib.sh`; change them there.

## Rust

| | |
|---|---|
| Toolchain | Rust `1.100.0`, pinned in `rust-toolchain.toml`; edition 2024 |
| Layout | Cargo workspace: `crates/<name>-core`, `-app`, `-tui`, `-gui`; `migrations/`, `testdata/`, `scripts/`, and `fuzz/` as its own workspace |
| Dashboard | egui and eframe `0.37.0`, with the toolkit look replaced by the template's design tokens |
| Terminal UI | ratatui `0.30.2` with crossterm `0.29.0` |
| SQLite | `rusqlite` `0.43.0` with the `bundled` feature, which includes FTS5 |
| Build with dashboard | `make build` |
| Build without dashboard | `make build-nogui` (the core, terminal and application crates with `--no-default-features`) |
| Format and lint | `cargo fmt --check`, `cargo clippy` with warnings as errors; `unsafe_code` is forbidden |
| Tests | `cargo test`, `proptest` `1.12.0`, AddressSanitizer through `make race-or-sanitize`, `cargo fuzz` `0.13.2` through `make fuzz` |
| Supply chain | `cargo audit` `0.23.0`, `cargo deny` `0.20.2`; license review by `make notices` |
| Packaging | `cargo bundle` `0.12.0` for the macOS application bundle, wrapped by `scripts/package.sh` |
| Scripts | `shellcheck` `0.11.0.1`, `shfmt` `4.2.0`, `bats` `1.13.0` |

System packages:

- Debian or Ubuntu: a C compiler for the bundled SQLite. The dashboard needs no extra headers to build; to run it needs the usual desktop libraries (X11 or Wayland, `libxkbcommon`, a graphics driver with Vulkan or OpenGL).
- macOS: the Xcode command line tools
- Windows: Build Tools for Visual Studio with the Windows SDK

`templates/rust/scripts/dev-setup.sh` reports what is missing and `--install` installs it. The versions above are defined in `templates/rust/Cargo.toml`, `templates/rust/rust-toolchain.toml` and `templates/rust/scripts/lib.sh`; change them there.

## C++ (planned profile)

| | |
|---|---|
| Toolchain | C++20, CMake with Ninja; GCC, Clang and MSVC |
| Layout | `src/core`, `src/app`, `src/tui`, `src/gui`, `tests/`, `third_party/` |
| Dashboard | Dear ImGui with GLFW and OpenGL 3 |
| Terminal UI | FTXUI |
| SQLite | the vendored amalgamation, compiled with FTS5 |
| Build without dashboard | `-DPROJECT_GUI=OFF` |
| Format and lint | `clang-format --dry-run --Werror`, `clang-tidy`, `cppcheck` |
| Tests | Catch2 or doctest, libFuzzer, ASan and UBSan, TSan for concurrent code |
| Supply chain | pinned dependencies, `osv-scanner`, license review |
| Packaging | CPack |

No version is recorded because the template does not exist yet.
