# 0004. Fyne for the dashboard, with its look replaced

Status: accepted

## Context

The dashboard must be a native window on Linux, macOS and Windows from one Go code base, be testable without a display, and look like one product rather than like a toolkit demo.

## Decision

The dashboard uses Fyne v2. `internal/gui/skin.go` implements the toolkit's theme interface from `theme.Tokens`, so every color, size and radius comes from the application's themes. Interface icons are original drawings generated in code; the toolkit's stock icons are not referenced, and a test enforces that. Layout pieces the toolkit lacks (the record table columns, surfaces, form and confirmation dialogs, the collapsible sidebar) are small custom widgets in `internal/gui/widgets.go`.

Tests drive the real widgets through Fyne's headless test driver and render screenshots from it.

Packaged builds use the `no_animations` build tag, because the toolkit has no runtime switch for its animations.

## Consequences

- The toolkit does not report the window position, so only the size is saved.
- Glyphs inside stock widgets (check boxes, drop-down arrows) still come from the toolkit and are recolored by the skin.
- Building the dashboard needs OpenGL and windowing headers; the no-GUI build does not.
- Accessibility support is whatever the toolkit provides; it has not been checked with a screen reader.
