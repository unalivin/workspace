# Language profiles

Every project has one primary language. SQL and Bash are supporting languages in every project. Python is never part of a product.

| Language | Projects | Template | State of the template |
|---|---|---|---|
| Go | 14 | `templates/go` | Present. Checked by the workflow `CI template-go` on Linux, macOS and Windows. |
| Rust | 8 | `templates/rust` | Not written yet |
| C++ | 3 | `templates/cpp` | Not written yet |

`scripts/new-project.sh` refuses a language whose template folder does not exist.

## Go

| | |
|---|---|
| Toolchain | Go `1.27.4`, cgo with a C compiler. The specification names `1.27.3`; `1.27.4` is the security patch release of the same line, and `govulncheck` reports standard-library findings for `1.27.3`. |
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

## Rust (planned profile)

| | |
|---|---|
| Toolchain | Stable Rust pinned in `rust-toolchain.toml` |
| Layout | Cargo workspace: `crates/<name>-core`, `-app`, `-tui`, `-gui` |
| Dashboard | egui with eframe |
| Terminal UI | ratatui with crossterm |
| SQLite | `rusqlite` with the `bundled` feature and FTS5 |
| Build without dashboard | `--no-default-features` |
| Format and lint | `cargo fmt --check`, `cargo clippy -- -D warnings` |
| Tests | `cargo test`, `proptest`, `cargo fuzz` for parsers |
| Supply chain | `cargo audit`, `cargo deny` |
| Packaging | `cargo-packager` or `cargo-bundle`, pinned |

No version is recorded because the template does not exist yet.

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
