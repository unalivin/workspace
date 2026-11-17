#include "notices.hpp"

#include <algorithm>
#include <cctype>
#include <fstream>
#include <map>
#include <optional>
#include <sstream>
#include <utility>

#include "core/error.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

namespace tools::notices {

namespace {

using app::AppError;
namespace text = app::text;

struct Notice {
  std::string component;
  std::string file;
  std::vector<std::string> copyrights;
};

struct Body {
  std::string content;
  std::vector<Notice> notices;
};

bool is_alnum(char letter) {
  return std::isalnum(static_cast<unsigned char>(letter)) != 0;
}

bool is_alpha(char letter) {
  return std::isalpha(static_cast<unsigned char>(letter)) != 0;
}

bool is_local(char letter) {
  return is_alnum(letter) || std::string_view("._%+-").find(letter) != std::string_view::npos;
}

bool is_host(char letter) {
  return is_alnum(letter) || letter == '.' || letter == '-';
}

std::optional<std::size_t> domain_length(std::string_view host) {
  std::optional<std::size_t> best;
  std::size_t label = 0;
  for (std::size_t index = 0; index < host.size(); ++index) {
    if (host[index] != '.') {
      ++label;
      continue;
    }
    if (label == 0) {
      break;
    }
    label = 0;
    std::size_t letters = 0;
    while (index + 1 + letters < host.size() && is_alpha(host[index + 1 + letters])) {
      ++letters;
    }
    if (letters >= 2) {
      best = index + 1 + letters;
    }
  }
  return best;
}

bool is_copyright_line(std::string_view line) {
  if (line.size() >= 200) {
    return false;
  }
  const std::string lower = text::lower(line);
  std::string_view rest = lower;
  if (!rest.starts_with("copyright")) {
    return false;
  }
  rest.remove_prefix(9);
  while (!rest.empty() && (rest.front() == ' ' || rest.front() == '\t')) {
    rest.remove_prefix(1);
  }
  return rest.starts_with("(c)") || rest.starts_with("\xc2\xa9") ||
         (!rest.empty() && std::isdigit(static_cast<unsigned char>(rest.front())) != 0);
}

std::string trim_end(std::string_view line) {
  while (!line.empty() && (line.back() == ' ' || line.back() == '\t' || line.back() == '\r')) {
    line.remove_suffix(1);
  }
  return std::string(line);
}

std::pair<std::vector<std::string>, std::string> split_copyrights(std::string_view raw) {
  std::vector<std::string> copyrights;
  std::vector<std::string> body;
  for (const std::string& original : text::split(text::replace_all(raw, "\r\n", "\n"), '\n')) {
    const std::string cleaned = omit_addresses(trim_end(original));
    const std::string trimmed(text::trim(cleaned));
    if (is_copyright_line(trimmed)) {
      if (std::find(copyrights.begin(), copyrights.end(), trimmed) == copyrights.end()) {
        copyrights.push_back(trimmed);
      }
      continue;
    }
    body.push_back(cleaned);
  }
  while (!body.empty() && body.front().empty()) {
    body.erase(body.begin());
  }
  while (!body.empty() && body.back().empty()) {
    body.pop_back();
  }
  std::string compact;
  int blank = 0;
  for (const std::string& line : body) {
    blank = text::trim(line).empty() ? blank + 1 : 0;
    if (blank < 2) {
      compact.append(line);
      compact.push_back('\n');
    }
  }
  return {copyrights, compact};
}

std::string fence(std::string_view body) {
  std::size_t longest = 2;
  std::size_t current = 0;
  for (const char letter : body) {
    current = letter == '`' ? current + 1 : 0;
    longest = std::max(longest, current);
  }
  return std::string(longest + 1, '`');
}

std::string escaped(std::string_view cell) {
  return text::replace_all(cell, "|", "\\|");
}

std::string titled(const Component& component) {
  return text::concat(component.name, " ", component.version);
}

}

std::string read_text(const std::filesystem::path& path) {
  std::ifstream file(path, std::ios::binary);
  if (!file) {
    throw AppError::io(text::concat(app::files::shown(path), " cannot be read"));
  }
  std::ostringstream content;
  content << file.rdbuf();
  return text::replace_all(content.str(), "\r\n", "\n");
}

void write_text(const std::filesystem::path& path, std::string_view content) {
  std::ofstream file(path, std::ios::binary | std::ios::trunc);
  file.write(content.data(), static_cast<std::streamsize>(content.size()));
  file.flush();
  if (!file) {
    throw AppError::io(text::concat(app::files::shown(path), " cannot be written"));
  }
}

std::string omit_addresses(std::string_view line) {
  std::string result;
  std::size_t copied = 0;
  std::size_t index = 0;
  while (index < line.size()) {
    if (line[index] == '@') {
      std::size_t start = index;
      while (start > copied && is_local(line[start - 1])) {
        --start;
      }
      std::size_t reach = 0;
      while (index + 1 + reach < line.size() && is_host(line[index + 1 + reach])) {
        ++reach;
      }
      const std::optional<std::size_t> domain = domain_length(line.substr(index + 1, reach));
      if (domain && start < index) {
        result.append(line.substr(copied, start - copied));
        result.append(address_omitted);
        copied = index + 1 + *domain;
        index = copied;
        continue;
      }
    }
    ++index;
  }
  result.append(line.substr(copied));
  return result;
}

const std::vector<std::string_view>& allowed_licenses() {
  static const std::vector<std::string_view> allowed = {
      "MIT",     "Zlib",    "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause",  "BSL-1.0",          "ISC",
      "OFL-1.1", "CC0-1.0", "WTFPL",      "Unlicense",    "Public domain", "MIT OR Unlicense",
  };
  return allowed;
}

std::vector<Component> parse_components(std::string_view table) {
  std::vector<Component> found;
  bool header = true;
  for (const std::string& line : text::split(text::replace_all(table, "\r\n", "\n"), '\n')) {
    if (line.empty()) {
      continue;
    }
    const std::vector<std::string> cells = text::split(line, '\t');
    if (cells.size() != 9) {
      throw AppError::io(
          text::concat(components_path, ": a row needs 9 tab-separated cells, found ", cells.size(), ": ", line));
    }
    if (header) {
      header = false;
      continue;
    }
    Component component{cells[0], cells[1], cells[2], cells[3], cells[4], cells[5], cells[6], cells[7], cells[8]};
    if (component.name.empty() || component.version.empty() || component.license.empty() || component.source.empty()) {
      throw AppError::io(text::concat(components_path, ": name, version, license and source are required: ", line));
    }
    if (component.use != "app" && component.use != "gui" && component.use != "tests") {
      throw AppError::io(text::concat(components_path, ": use must be app, gui or tests: ", line));
    }
    const std::vector<std::string_view>& allowed = allowed_licenses();
    if (std::find(allowed.begin(), allowed.end(), component.license) == allowed.end()) {
      throw AppError::io(text::concat(components_path, ": ", component.name, " has the license ", component.license,
                                      ", which is not on the list of accepted licenses"));
    }
    found.push_back(std::move(component));
  }
  for (const Component& component : found) {
    if (component.part_of.empty()) {
      continue;
    }
    const bool known = std::any_of(found.begin(), found.end(), [&component](const Component& other) {
      return other.part_of.empty() && other.name == component.part_of;
    });
    if (!known) {
      throw AppError::io(text::concat(components_path, ": ", component.name, " is part of the unlisted component ",
                                      component.part_of));
    }
  }
  return found;
}

std::vector<Component> load_components(const std::filesystem::path& root) {
  return parse_components(read_text(root / app::files::from_utf8(components_path)));
}

std::string render(const std::filesystem::path& root) {
  std::vector<Component> shipped;
  for (Component& component : load_components(root)) {
    if (component.use != "tests") {
      shipped.push_back(std::move(component));
    }
  }
  if (shipped.empty()) {
    throw AppError::io(text::concat(components_path, " lists no shipped component"));
  }
  std::vector<Component> direct;
  std::vector<Component> bundled;
  for (const Component& component : shipped) {
    (component.part_of.empty() ? direct : bundled).push_back(component);
  }
  const auto by_name = [](const Component& first, const Component& second) {
    return text::lower(first.name) < text::lower(second.name);
  };
  std::sort(direct.begin(), direct.end(), by_name);
  std::sort(bundled.begin(), bundled.end(), by_name);
  std::vector<Component> ordered = direct;
  ordered.insert(ordered.end(), bundled.begin(), bundled.end());

  std::vector<Body> bodies;
  std::map<std::string, std::size_t> positions;
  std::vector<Component> bare;
  for (const Component& component : ordered) {
    if (component.license_file.empty()) {
      bare.push_back(component);
      continue;
    }
    const std::string relative = text::concat(licenses_directory, "/", component.license_file);
    auto [copyrights, body] = split_copyrights(read_text(root / app::files::from_utf8(relative)));
    if (text::trim(body).empty()) {
      throw AppError::io(text::concat(relative, " holds no license text"));
    }
    const auto [position, added] = positions.try_emplace(body, bodies.size());
    if (added) {
      bodies.push_back(Body{body, {}});
    }
    bodies[position->second].notices.push_back(Notice{titled(component), relative, std::move(copyrights)});
  }

  const std::string font_license = omit_addresses(read_text(root / app::files::from_utf8(font_directory) / "OFL.txt"));

  std::string page;
  page.append("# Third-party notices\n\n");
  page.append(text::concat(app::meta::app_name, " is published by ", app::meta::publisher,
                           " under the MIT license (see `LICENSE`). The release binaries also contain the third-party "
                           "components listed here. Each component keeps its own license; none of them is "
                           "relicensed.\n\n"));
  page.append(text::concat("This file is generated by `make notices` from `", components_path,
                           "` and the license files in `", licenses_directory,
                           "`. Do not edit it by hand. E-mail addresses in upstream copyright lines are replaced by \"",
                           address_omitted,
                           "\" because this repository contains no e-mail addresses; the unmodified files ship with "
                           "each component at the listed version.\n\n"));
  page.append(
      text::concat("The list covers every supported operating system and both builds: components marked "
                   "\"dashboard\" are absent from a build made with `-DPROJECT_GUI=OFF`. Components used only "
                   "by the tests are not listed. It has ",
                   ordered.size(), " components and ", bodies.size(), " distinct license texts.\n\n"));
  page.append("## Components bundled inside dependencies\n\n");
  page.append("Some dependencies carry further components that end up in the binary:\n\n");
  for (const Component& component : bundled) {
    page.append(text::concat("- ", component.name, " (", component.license, "), inside ", component.part_of, ".\n"));
  }
  page.append(
      "\nThe SQLite library is compiled from the amalgamation in `third_party/sqlite`; every other component "
      "is built by vcpkg at the versions pinned in `vcpkg.json`.\n\n");
  page.append(
      "The application's own icons (navigation, status, severity and action icons, and the application icon) "
      "are original drawings released under the application's license.\n\n");
  page.append("## Fonts embedded by the application\n\n");
  page.append(
      text::concat("The regular and the medium weight of Noto Sans 2.015 (SIL Open Font License 1.1), which "
                   "the dashboard embeds unchanged from `",
                   font_directory, "`. Their license file is reproduced here.\n\n"));
  const std::string font_marks = fence(font_license);
  page.append(text::concat(font_marks, "text\n", trim_end(text::trim(font_license)), "\n", font_marks, "\n\n"));
  page.append("## Components\n\n| Component | Version | License | Built into | Source |\n|---|---|---|---|---|\n");
  for (const Component& component : ordered) {
    const std::string_view build = component.use == "gui" ? "dashboard" : "every build";
    const std::string where =
        component.part_of.empty() ? std::string(build) : text::concat(build, ", inside ", component.part_of);
    page.append(text::concat("| ", escaped(component.name), " | ", escaped(component.version), " | ",
                             escaped(component.license), " | ", escaped(where), " | ", component.source, " |\n"));
  }
  if (!bare.empty()) {
    page.append("\n## Components without a license text\n\n");
    page.append(
        "These components are dedicated to the public domain and need no notice. They are listed for "
        "completeness.\n\n");
    for (const Component& component : bare) {
      page.append(text::concat("- ", titled(component), " (", component.license, ")\n"));
    }
  }
  page.append("\n## License texts\n\n");
  page.append(
      "Each text is printed once. Copyright lines are taken out of the text and listed with the component "
      "they belong to.\n");
  for (std::size_t number = 0; number < bodies.size(); ++number) {
    page.append(text::concat("\n### License text ", number + 1, "\n\nUsed by:\n\n"));
    for (const Notice& notice : bodies[number].notices) {
      page.append(text::concat("- ", notice.component, " (`", notice.file, "`)"));
      if (!notice.copyrights.empty()) {
        page.append(text::concat(": ", text::join(notice.copyrights, "; ")));
      }
      page.push_back('\n');
    }
    const std::string marks = fence(bodies[number].content);
    page.append(text::concat("\n", marks, "text\n", bodies[number].content, marks, "\n"));
  }
  return page;
}

}
