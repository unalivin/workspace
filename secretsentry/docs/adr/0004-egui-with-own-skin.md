# 0004. egui for the dashboard, with its look replaced

Status: accepted

## Context

The dashboard must be a native window on Linux, macOS and Windows from one Rust code base, be testable without a display, and look like one product rather than like a toolkit demo.

## Decision

The dashboard uses egui, with eframe as the window host. `crates/secretsentry-gui/src/skin.rs` builds the toolkit's style from the design tokens of the `theme` module, so every color, size and radius comes from the application's themes. Interface icons are original drawings: outlines in `icons.rs` that are filled by the application's own code. The emoji and icon fonts of the toolkit are not used for interface icons. Layout pieces the toolkit lacks (the record table columns, surfaces, form and confirmation dialogs, the collapsible sidebar) are written in the dashboard crate.

The interface state and every action live in a model (`model.rs`) that does not depend on a window. Tests call the model, feed pointer and key events to the same drawing code the window uses, and turn the result into pictures with a small software renderer (`render.rs`). No display, graphics driver or window is involved, so the same tests run on every CI runner.

The reduced motion setting sets the toolkit's animation time to zero at run time; one build serves both.

## Consequences

- The window host (`host.rs`), the event loop and the graphics backend run only in a real window. They are not covered by the tests, see [../TESTING.md](../TESTING.md).
- The software renderer is test and screenshot code. It paints what the toolkit asks for, but it is not the renderer users see, so a picture from it does not prove how a graphics driver draws the same frame.
- The window position is not saved; the size is.
- Marks inside toolkit widgets (check boxes, drop-down arrows) are drawn by the toolkit and colored by the skin.
- Building the dashboard needs no windowing or graphics headers. Running it needs a graphics driver, see [../INSTALLATION.md](../INSTALLATION.md).
- The toolkit is immediate mode: the visible page is laid out again for every frame. The dashboard asks for a new frame only on input, on the refresh interval and while a notice fades.
- Accessibility support is whatever the toolkit provides through AccessKit; it has not been checked with a screen reader.
