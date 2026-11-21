# Languages and Toolchains

Each project uses one primary language, plus SQLite (SQL) for persistence and Bash for developer scripts.

## Go (14 projects)

- Toolchain: Go `1.27.4`
- Desktop GUI: Fyne
- Terminal UI: Bubble Tea
- SQLite: `modernc.org/sqlite` or cgo driver with FTS5
- No-GUI build: `-tags nogui`

## Rust (8 projects)

- Toolchain: Stable Rust (`1.83.0` pinned in `rust-toolchain.toml`)
- Desktop GUI: `egui` with `eframe`
- Terminal UI: `ratatui` with `crossterm`
- SQLite: `rusqlite` with `bundled` and FTS5
- No-GUI build: `--no-default-features`

## C++ (3 projects)

- Toolchain: C++20, CMake with Ninja
- Desktop GUI: Dear ImGui with GLFW and OpenGL 3
- Terminal UI: FTXUI
- SQLite: SQLite amalgamation with FTS5
- No-GUI build: `-DPROJECT_GUI=OFF`
