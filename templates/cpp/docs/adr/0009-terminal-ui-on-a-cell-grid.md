# 0009. The terminal UI paints a grid of cells

Status: accepted

## Context

The terminal UI must offer every capability of the dashboard, fill the terminal exactly at every size, and be tested without a terminal. Terminal libraries differ in how they lay out widgets, and a layout that a test cannot read cannot be checked.

## Decision

The terminal UI is a model (`src/tui/model.cpp` and the page files) that receives keys as plain values and paints a grid of cells, each with a character, colors and attributes. Layout, clipping, wrapping by display width and scrolling are the application's own code (`text.cpp`, `widgets.cpp`). FTXUI is used in one file, `screen.cpp`, for what needs a real terminal: the alternate screen, raw keyboard input, the color capability and writing the cells.

## Consequences

- Tests press keys on the model and compare the painted grid, for every screen and at every terminal size, on every CI runner.
- The terminal library can be replaced by changing one file.
- Widgets of FTXUI are not used, so nothing is gained from its component set; forms, tables and dialogs are written in `src/tui`.
- What happens in a real terminal emulator (key encodings, resize events, colors) is not covered by the tests, see [../PLATFORM_SUPPORT.md](../PLATFORM_SUPPORT.md).
