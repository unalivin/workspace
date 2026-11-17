#if defined(__APPLE__)
#define GL_SILENCE_DEPRECATION
#endif

#include <GLFW/glfw3.h>
#include <imgui.h>
#include <imgui_impl_glfw.h>
#include <imgui_impl_opengl3.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "core/brand.hpp"
#include "core/error.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "gui/appearance.hpp"
#include "gui/chooser.hpp"
#include "gui/gui.hpp"
#include "gui/skin.hpp"
#include "gui/ui.hpp"
#include "gui/view.hpp"

namespace app::gui {

namespace {

constexpr int icon_side = 256;
constexpr double active_seconds = 0.6;
constexpr double typing_seconds = 0.25;

std::string& last_problem() {
  static std::string problem;
  return problem;
}

void remember_problem(int, const char* description) {
  last_problem() = description == nullptr ? "" : description;
}

AppError failure() {
  const std::string reason = last_problem().empty() ? std::string("no display is available") : last_problem();
  return AppError::io(text::concat("The window could not be opened: ", reason, ". Start ", meta::slug,
                                   " --ui tui in a terminal instead."));
}

struct Signals {
  bool woken = true;
  bool focused = false;
};

Signals* signals_of(GLFWwindow* window) {
  return static_cast<Signals*>(glfwGetWindowUserPointer(window));
}

void wake(GLFWwindow* window) {
  if (Signals* signals = signals_of(window)) {
    signals->woken = true;
  }
}

void on_cursor(GLFWwindow* window, double, double) {
  wake(window);
}

void on_button(GLFWwindow* window, int, int, int) {
  wake(window);
}

void on_scroll(GLFWwindow* window, double, double) {
  wake(window);
}

void on_key(GLFWwindow* window, int, int, int, int) {
  wake(window);
}

void on_char(GLFWwindow* window, unsigned int) {
  wake(window);
}

void on_enter(GLFWwindow* window, int) {
  wake(window);
}

void on_size(GLFWwindow* window, int, int) {
  wake(window);
}

void on_refresh(GLFWwindow* window) {
  wake(window);
}

void on_focus(GLFWwindow* window, int focused) {
  if (Signals* signals = signals_of(window)) {
    signals->woken = true;
    signals->focused = signals->focused || focused == GLFW_TRUE;
  }
}

class Library {
 public:
  Library() {
    last_problem().clear();
    glfwSetErrorCallback(remember_problem);
    if (glfwInit() != GLFW_TRUE) {
      throw failure();
    }
  }
  Library(const Library&) = delete;
  Library& operator=(const Library&) = delete;
  ~Library() {
    glfwTerminate();
  }
};

struct Shape {
  std::string title;
  float width = 0;
  float height = 0;
  bool resizable = true;
  float least_width = 0;
  float least_height = 0;
};

class Surface {
 public:
  explicit Surface(const Shape& shape) {
#if defined(__APPLE__)
    const char* shaders = "#version 150";
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 2);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    glfwWindowHint(GLFW_OPENGL_FORWARD_COMPAT, GLFW_TRUE);
#else
    const char* shaders = "#version 130";
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 0);
#endif
    glfwWindowHint(GLFW_RESIZABLE, shape.resizable ? GLFW_TRUE : GLFW_FALSE);
    glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
    const std::string identity(meta::app_id);
    glfwWindowHintString(GLFW_WAYLAND_APP_ID, identity.c_str());
    glfwWindowHintString(GLFW_X11_CLASS_NAME, identity.c_str());
    glfwWindowHintString(GLFW_X11_INSTANCE_NAME, identity.c_str());
    const float factor = ImGui_ImplGlfw_GetContentScaleForMonitor(glfwGetPrimaryMonitor());
    const int width = static_cast<int>(std::lround(shape.width * factor));
    const int height = static_cast<int>(std::lround(shape.height * factor));
    window_ = glfwCreateWindow(width, height, shape.title.c_str(), nullptr, nullptr);
    if (window_ == nullptr) {
      throw failure();
    }
    glfwSetWindowUserPointer(window_, &signals_);
    glfwSetCursorPosCallback(window_, on_cursor);
    glfwSetMouseButtonCallback(window_, on_button);
    glfwSetScrollCallback(window_, on_scroll);
    glfwSetKeyCallback(window_, on_key);
    glfwSetCharCallback(window_, on_char);
    glfwSetCursorEnterCallback(window_, on_enter);
    glfwSetWindowFocusCallback(window_, on_focus);
    glfwSetFramebufferSizeCallback(window_, on_size);
    glfwSetWindowRefreshCallback(window_, on_refresh);
    if (shape.least_width > 0.0F) {
      glfwSetWindowSizeLimits(window_, static_cast<int>(std::lround(shape.least_width * factor)),
                              static_cast<int>(std::lround(shape.least_height * factor)), GLFW_DONT_CARE,
                              GLFW_DONT_CARE);
    }
    place(width, height);
    decorate();
    glfwMakeContextCurrent(window_);
    glfwSwapInterval(1);
    context_ = ImGui::CreateContext();
    ImGui::SetCurrentContext(context_);
    ImGuiIO& io = ImGui::GetIO();
    io.IniFilename = nullptr;
    io.LogFilename = nullptr;
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    faces_ = install_fonts();
    if (!ImGui_ImplGlfw_InitForOpenGL(window_, true) || !ImGui_ImplOpenGL3_Init(shaders)) {
      release();
      last_problem() = "OpenGL 3 is not available";
      throw failure();
    }
    glfwShowWindow(window_);
  }
  Surface(const Surface&) = delete;
  Surface& operator=(const Surface&) = delete;
  ~Surface() {
    ImGui::SetCurrentContext(context_);
    ImGui_ImplOpenGL3_Shutdown();
    ImGui_ImplGlfw_Shutdown();
    release();
  }

  [[nodiscard]] const Faces& faces() const {
    return faces_;
  }

  [[nodiscard]] float scale() const {
    return ImGui_ImplGlfw_GetContentScaleForWindow(window_);
  }

  [[nodiscard]] bool closing() const {
    return glfwWindowShouldClose(window_) == GLFW_TRUE;
  }

  [[nodiscard]] bool take_focus() {
    return std::exchange(signals_.focused, false);
  }

  [[nodiscard]] std::optional<WindowSize> restored_size() const {
    if (glfwGetWindowAttrib(window_, GLFW_MAXIMIZED) == GLFW_TRUE ||
        glfwGetWindowAttrib(window_, GLFW_ICONIFIED) == GLFW_TRUE || glfwGetWindowMonitor(window_) != nullptr) {
      return std::nullopt;
    }
    int width = 0;
    int height = 0;
    glfwGetWindowSize(window_, &width, &height);
    if (width <= 0 || height <= 0) {
      return std::nullopt;
    }
    const float factor = std::max(scale(), 0.5F);
    return WindowSize{static_cast<float>(width) / factor, static_cast<float>(height) / factor};
  }

  void wait(double seconds) {
    if (std::exchange(signals_.woken, false)) {
      active_until_ = glfwGetTime() + active_seconds;
    }
    if (glfwGetTime() < active_until_ || seconds <= 0.0) {
      glfwPollEvents();
    } else {
      glfwWaitEventsTimeout(seconds);
    }
  }

  void begin() {
    ImGui_ImplOpenGL3_NewFrame();
    ImGui_ImplGlfw_NewFrame();
    ImGui::NewFrame();
  }

  void end(theme::Color clear) {
    ImGui::Render();
    int width = 0;
    int height = 0;
    glfwGetFramebufferSize(window_, &width, &height);
    const ImVec4 fill = paint(clear);
    glViewport(0, 0, width, height);
    glClearColor(fill.x, fill.y, fill.z, 1.0F);
    glClear(GL_COLOR_BUFFER_BIT);
    ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());
    glfwSwapBuffers(window_);
  }

 private:
  void place(int width, int height) {
    GLFWmonitor* monitor = glfwGetPrimaryMonitor();
    if (monitor == nullptr) {
      return;
    }
    int left = 0;
    int top = 0;
    int area_width = 0;
    int area_height = 0;
    glfwGetMonitorWorkarea(monitor, &left, &top, &area_width, &area_height);
    if (area_width > 0 && area_height > 0) {
      glfwSetWindowPos(window_, left + std::max((area_width - width) / 2, 0),
                       top + std::max((area_height - height) / 2, 0));
    }
  }

  void decorate() {
    std::vector<std::uint8_t> pixels = brand::icon(icon_side);
    GLFWimage image;
    image.width = icon_side;
    image.height = icon_side;
    image.pixels = pixels.data();
    glfwSetWindowIcon(window_, 1, &image);
  }

  void release() {
    if (context_ != nullptr) {
      ImGui::DestroyContext(context_);
      context_ = nullptr;
    }
    if (window_ != nullptr) {
      glfwDestroyWindow(window_);
      window_ = nullptr;
    }
  }

  GLFWwindow* window_ = nullptr;
  ImGuiContext* context_ = nullptr;
  Faces faces_;
  Signals signals_;
  double active_until_ = 0.0;
};

launcher::PromptResult quit_result() {
  launcher::PromptResult result;
  result.quit = true;
  return result;
}

}

std::string run(service::App& app, Options options) {
  const WindowSize size = window_size(app.settings().effective());
  const Library library;
  Surface surface({std::string(meta::app_name), size.width, size.height, true, min_width, min_height});
  if (const std::optional<bool> dark = system_dark(options.env)) {
    options.system_dark = *dark;
  }
  const platform::Env env = options.env;
  Dashboard board(app, std::move(options));
  View view(board, surface.faces(), surface.scale());
  std::optional<WindowSize> kept;
  double ticked = glfwGetTime();
  while (true) {
    const double now = glfwGetTime();
    const double interval = board.interval();
    const bool animating = view.notice_wash(ImGui::GetTime()) > 0;
    const double idle = ImGui::GetIO().WantTextInput ? typing_seconds : std::max(interval - (now - ticked), 0.01);
    surface.wait(animating ? 0.0 : idle);
    const bool closing = surface.closing();
    if (const std::optional<WindowSize> current = surface.restored_size()) {
      kept = current;
    }
    if (surface.take_focus()) {
      if (const std::optional<bool> dark = system_dark(env)) {
        board.follow_system(*dark);
      }
    }
    if (glfwGetTime() - ticked >= interval) {
      ticked = glfwGetTime();
      board.tick();
    }
    view.rescale(surface.scale());
    view.prepare();
    surface.begin();
    view.frame();
    surface.end(view.tokens().palette.background);
    if (closing && !board.finished()) {
      board.close("");
    }
    if (board.finished()) {
      if (kept) {
        board.keep_window_size(kept->width, kept->height);
      }
      break;
    }
  }
  return board.next();
}

launcher::PromptResult choose(std::string_view default_mode) {
  const Library library;
  Surface surface({chooser::title(), chooser::width, chooser::height, false, 0.0F, 0.0F});
  const platform::Env env = platform::Env::system();
  std::optional<theme::Tokens> tokens;
  Icons icons;
  std::vector<Seen> seen;
  bool remember = false;
  bool focused = false;
  bool dark = system_dark(env).value_or(false);
  float scale = 0.0F;
  while (true) {
    surface.wait(typing_seconds * 4.0);
    const bool closing = surface.closing();
    if (surface.take_focus()) {
      dark = system_dark(env).value_or(dark);
    }
    if (!tokens || tokens->dark != dark || scale != surface.scale()) {
      scale = surface.scale();
      tokens = scaled(chooser::tokens(dark), scale);
      apply(*tokens);
    }
    surface.begin();
    seen.clear();
    const Ui ui(*tokens, surface.faces(), icons, seen, scale);
    const chooser::Choice choice = chooser::show(ui, default_mode, remember, !std::exchange(focused, true));
    surface.end(tokens->palette.background);
    if (choice.kind == chooser::ChoiceKind::Mode) {
      launcher::PromptResult result;
      result.mode = std::string(choice.mode);
      result.remember = remember;
      return result;
    }
    if (choice.kind == chooser::ChoiceKind::Quit || closing) {
      return quit_result();
    }
  }
}

}
