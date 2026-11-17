# 0004. Dear ImGui for the dashboard, with its look replaced

Status: accepted

## Context

The dashboard must be a native window on Linux, macOS and Windows from one C++ code base, be testable without a display, and look like one product rather than like a toolkit demo. It must build with GCC, Clang and MSVC and link statically into one executable.

## Decision

The dashboard uses Dear ImGui, with GLFW as the window host and OpenGL 3 as the graphics backend. `src/gui/skin.cpp` builds the toolkit's style from the design tokens of the `theme` module, so every color, size and radius comes from the application's themes, and installs the embedded Noto Sans faces. Interface icons are original drawings: outlines in `icons.cpp` that are filled by the application's own code. Layout pieces the toolkit lacks (the record table columns, surfaces, form and confirmation dialogs, the collapsible sidebar, the tally) are written in `src/gui`.

The interface state and every action live in a model (`model.cpp`) that does not depend on a window. The drawing code (`view.cpp`, `pages.cpp`) depends on Dear ImGui but not on GLFW or OpenGL; these files are the library `app_gui_view`. Tests call the model, feed pointer and key events to the same drawing code the window uses, and turn the draw lists into pictures with a small software renderer (`render.cpp`). No display, graphics driver or window is involved, so the same tests run on every CI runner. Only `host.cpp` touches GLFW and OpenGL.

## Consequences

- The window host (`host.cpp`), the event loop and the graphics backend run only in a real window. They are not covered by the tests, see [../TESTING.md](../TESTING.md).
- The software renderer is test and screenshot code. It paints what the toolkit asks for, but it is not the renderer users see, so a picture from it does not prove how a graphics driver draws the same frame.
- The window position is not saved; the size is.
- Marks inside toolkit widgets (check boxes, drop-down arrows) are drawn by the toolkit and colored by the skin.
- Building the dashboard on Linux needs the X11 and OpenGL headers, and running it needs X11 (or XWayland) and OpenGL 3, see [../INSTALLATION.md](../INSTALLATION.md).
- The toolkit is immediate mode: the visible page is laid out again for every frame. The window draws a new frame only on input, on the refresh interval and while a notice fades.
- Dear ImGui has no accessibility interface. A screen reader cannot read the dashboard; the terminal UI and the commands are the accessible ways to use the application.
