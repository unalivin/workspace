#include <doctest/doctest.h>

#include <algorithm>
#include <array>
#include <cctype>
#include <cstddef>
#include <filesystem>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <vector>

#include "app/cli.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/settings.hpp"
#include "core/text.hpp"
#include "support/testkit.hpp"

namespace files = app::files;
namespace fs = std::filesystem;
namespace meta = app::meta;
namespace settings = app::settings;
namespace text = app::text;

namespace {

constexpr std::array<std::string_view, 34> required_files = {
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    "LICENSE",
    "VERSION",
    "Makefile",
    "CMakeLists.txt",
    "CMakePresets.json",
    "vcpkg.json",
    ".clang-format",
    ".clang-tidy",
    ".gitignore",
    ".gitattributes",
    ".env.example",
    "THIRD_PARTY_NOTICES.md",
    "third_party/components.tsv",
    "docs/README.md",
    "docs/CONTRIBUTING.md",
    "docs/INSTALLATION.md",
    "docs/USAGE.md",
    "docs/COMMANDS.md",
    "docs/CONFIGURATION.md",
    "docs/TROUBLESHOOTING.md",
    "docs/SECURITY.md",
    "docs/THREAT_MODEL.md",
    "docs/PLATFORM_SUPPORT.md",
    "docs/TESTING.md",
    "docs/ARCHITECTURE.md",
    "docs/ROADMAP.md",
    "docs/RELEASING.md",
    "docs/RELEASE_NOTES.md",
    "docs/LICENSES.md",
};
constexpr std::array<std::string_view, 9> required_directories = {
    "migrations", "scripts", "assets", "testdata", "docs", "docs/adr", "fuzz", "cmake", "packaging",
};
constexpr std::array<std::string_view, 9> skipped_directories = {
    ".git", ".tools", "build", "dist", "node_modules", "corpus", "artifacts", "coverage", "vcpkg_installed",
};
constexpr std::array<std::string_view, 7> readme_sections = {
    "Download", "Install", "Usage", "Documentation", "Security", "Contributing", "License",
};
constexpr std::array<std::string_view, 4> storage_modules = {
    "src/core/store.cpp",
    "src/core/settings.cpp",
    "src/core/records.cpp",
    "src/core/notify.cpp",
};
constexpr std::array<std::string_view, 2> database_users = {
    "src/core/store.hpp",
    "fuzz/targets/records_search_expression.cpp",
};
constexpr std::array<std::string_view, 10> statement_words = {
    "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "PRAGMA ", " WHERE ", "ORDER BY", " FROM ", "CREATE ", "DROP ",
};
constexpr std::array<std::string_view, 6> text_builders = {
    "concat(", "append(", "+=", "format(", "<<", "\" +",
};
constexpr std::array<std::string_view, 1> allowed_builders = {
    "ends_with(text::concat(\"PRAGMA \", marker))",
};

template <std::size_t Count>
bool listed(const std::array<std::string_view, Count>& names, std::string_view name) {
  return std::ranges::find(names, name) != names.end();
}

bool word_letter(char letter) {
  return std::isalnum(static_cast<unsigned char>(letter)) != 0 || letter == '_';
}

std::string shown(const fs::path& path) {
  return text::replace_all(files::shown(fs::relative(path, testkit::source_root())), "\\", "/");
}

void collect(const fs::path& directory, std::vector<fs::path>& found) {
  std::vector<fs::path> entries;
  for (const fs::directory_entry& entry : fs::directory_iterator(directory)) {
    entries.push_back(entry.path());
  }
  std::ranges::sort(entries);
  for (const fs::path& path : entries) {
    if (!fs::is_directory(path)) {
      found.push_back(path);
    } else if (!listed(skipped_directories, files::shown(path.filename()))) {
      collect(path, found);
    }
  }
}

const std::vector<fs::path>& project_files() {
  static const std::vector<fs::path> found = [] {
    std::vector<fs::path> all;
    collect(testkit::source_root(), all);
    return all;
  }();
  return found;
}

std::string read(const fs::path& path) {
  return text::replace_all(files::read_all(path), "\r\n", "\n");
}

std::string read_named(std::string_view name) {
  return read(testkit::source_root() / files::from_utf8(name));
}

bool is_text(std::string_view content) {
  return content.substr(0, 8000).find('\0') == std::string_view::npos;
}

std::string extension(const fs::path& path) {
  return files::shown(path.extension());
}

bool is_document(const fs::path& path) {
  return extension(path) == ".md" && files::shown(path.filename()) != "THIRD_PARTY_NOTICES.md" &&
         !shown(path).starts_with("third_party/");
}

bool is_source(const fs::path& path) {
  const std::string kind = extension(path);
  return (kind == ".cpp" || kind == ".hpp" || kind == ".h" || kind == ".c") && !shown(path).starts_with("third_party/");
}

std::vector<std::string> lines_of(std::string_view content) {
  return text::split(content, '\n');
}

bool has_domain(std::string_view host) {
  std::size_t label = 0;
  for (std::size_t index = 0; index < host.size(); ++index) {
    if (host[index] != '.') {
      ++label;
      continue;
    }
    if (label == 0) {
      return false;
    }
    label = 0;
    std::size_t letters = 0;
    while (index + 1 + letters < host.size() &&
           std::isalpha(static_cast<unsigned char>(host[index + 1 + letters])) != 0) {
      ++letters;
    }
    if (letters >= 2) {
      return true;
    }
  }
  return false;
}

bool has_address(std::string_view line) {
  const auto local = [](char letter) {
    return std::isalnum(static_cast<unsigned char>(letter)) != 0 ||
           std::string_view("._%+-").find(letter) != std::string_view::npos;
  };
  const auto host = [](char letter) {
    return std::isalnum(static_cast<unsigned char>(letter)) != 0 || letter == '-' || letter == '.';
  };
  for (std::size_t index = 1; index < line.size(); ++index) {
    if (line[index] != '@' || !local(line[index - 1])) {
      continue;
    }
    std::size_t reach = 0;
    while (index + 1 + reach < line.size() && host(line[index + 1 + reach])) {
      ++reach;
    }
    if (has_domain(line.substr(index + 1, reach))) {
      return true;
    }
  }
  return false;
}

std::size_t skip_quoted(std::string_view source, std::size_t index, std::size_t& line) {
  const char quote = source[index];
  ++index;
  while (index < source.size() && source[index] != quote) {
    if (source[index] == '\\') {
      ++index;
    }
    if (index < source.size() && source[index] == '\n') {
      ++line;
    }
    ++index;
  }
  return index + 1;
}

std::size_t skip_raw(std::string_view source, std::size_t index, std::size_t& line) {
  const std::size_t open = source.find('(', index);
  if (open == std::string_view::npos) {
    return source.size();
  }
  const std::string closing = text::concat(")", source.substr(index + 1, open - index - 1), "\"");
  const std::size_t close = source.find(closing, open);
  const std::size_t end = close == std::string_view::npos ? source.size() : close + closing.size();
  line += static_cast<std::size_t>(std::ranges::count(source.substr(index, end - index), '\n'));
  return end;
}

std::optional<std::size_t> comment_line(std::string_view source) {
  std::size_t line = 1;
  std::size_t index = 0;
  while (index < source.size()) {
    const char letter = source[index];
    const char before = index == 0 ? ' ' : source[index - 1];
    const char next = index + 1 < source.size() ? source[index + 1] : ' ';
    if (letter == '\n') {
      ++line;
      ++index;
    } else if (letter == '/' && (next == '/' || next == '*')) {
      return line;
    } else if (letter == '"' && before == 'R' && (index < 2 || !word_letter(source[index - 2]))) {
      index = skip_raw(source, index, line);
    } else if (letter == '"' || (letter == '\'' && !word_letter(before))) {
      index = skip_quoted(source, index, line);
    } else {
      ++index;
    }
  }
  return std::nullopt;
}

std::optional<std::size_t> marked_line(std::string_view content, bool script) {
  std::size_t number = 0;
  for (const std::string& line : lines_of(content)) {
    ++number;
    if (text::trim(line).starts_with('#') && !(script && line.starts_with("#!"))) {
      return number;
    }
  }
  return std::nullopt;
}

std::optional<std::size_t> statement_comment_line(std::string_view content) {
  std::size_t number = 0;
  for (const std::string& line : lines_of(content)) {
    ++number;
    if (text::trim(line).starts_with("--") || text::contains(line, "/*")) {
      return number;
    }
  }
  return std::nullopt;
}

std::set<std::string, std::less<>> declared_tests() {
  std::set<std::string, std::less<>> declared;
  const std::string opening = text::concat("TEST_", "CASE(\"");
  for (const fs::path& path : project_files()) {
    if (!is_source(path)) {
      continue;
    }
    if (shown(path).starts_with("fuzz/targets/")) {
      declared.insert(files::shown(path.stem()));
    }
    for (const std::string& line : lines_of(read(path))) {
      if (!line.starts_with(opening)) {
        continue;
      }
      const std::size_t close = line.rfind("\")");
      if (close != std::string::npos && close > opening.size()) {
        declared.insert(line.substr(opening.size(), close - opening.size()));
      }
    }
  }
  return declared;
}

std::optional<std::string> cited_test(std::string_view span) {
  if (span.size() < 3 || !span.starts_with('"') || !span.ends_with('"')) {
    return std::nullopt;
  }
  const std::string_view name = span.substr(1, span.size() - 2);
  if (std::ranges::count(name, ' ') < 2 || text::contains(name, "\"")) {
    return std::nullopt;
  }
  return std::string(name);
}

std::vector<std::string> code_spans(std::string_view document) {
  std::vector<std::string> spans;
  const std::vector<std::string> parts = text::split(document, '`');
  for (std::size_t index = 1; index < parts.size(); index += 2) {
    spans.push_back(parts[index]);
  }
  return spans;
}

bool builds_a_statement(std::string_view line) {
  const auto present = [line](std::string_view part) { return text::contains(line, part); };
  return std::ranges::any_of(text_builders, present) && std::ranges::any_of(statement_words, present) &&
         std::ranges::none_of(allowed_builders, present);
}

}

TEST_CASE("required files and readme sections exist") {
  const fs::path base = testkit::source_root();
  for (const std::string_view name : required_files) {
    const fs::path path = base / files::from_utf8(name);
    std::error_code problem;
    const bool filled = fs::is_regular_file(path, problem) && fs::file_size(path, problem) > 0;
    CHECK_MESSAGE(filled, "required file ", name, " is missing or empty");
  }
  for (const std::string_view name : required_directories) {
    CHECK_MESSAGE(fs::is_directory(base / files::from_utf8(name)), "required directory ", name, " is missing");
  }
  REQUIRE(fs::is_regular_file(base / "README.md"));
  REQUIRE(fs::is_regular_file(base / "docs" / "README.md"));
  const std::string readme = read_named("README.md");
  for (const std::string_view section : readme_sections) {
    CHECK_MESSAGE(text::contains(readme, text::concat("\n## ", section, "\n")), "README.md has no section ", section);
  }
  const std::string index = read_named("docs/README.md");
  for (const std::string_view name : required_files) {
    if (!name.starts_with("docs/") || name == "docs/README.md") {
      continue;
    }
    const std::string_view page = name.substr(5);
    CHECK_MESSAGE(text::contains(index, text::concat("(", page, ")")), "docs/README.md does not link ", page);
  }
  CHECK_MESSAGE(text::contains(index, "(adr/README.md)"), "docs/README.md does not link the decision records");
}

TEST_CASE("relative links in documents resolve") {
  std::size_t checked = 0;
  for (const fs::path& path : project_files()) {
    if (!is_document(path)) {
      continue;
    }
    const std::string document = read(path);
    std::size_t position = document.find("](");
    while (position != std::string::npos) {
      const std::size_t close = document.find(')', position);
      if (close == std::string::npos) {
        break;
      }
      const std::string target = document.substr(position + 2, close - position - 2);
      const std::string file = target.substr(0, target.find('#'));
      position = document.find("](", close);
      if (file.empty() || text::contains(file, "://") || text::contains(file, " ") || file.starts_with("mailto:")) {
        continue;
      }
      ++checked;
      CHECK_MESSAGE(fs::exists(path.parent_path() / files::from_utf8(file)), shown(path), " links to ", file,
                    ", which does not exist");
    }
  }
  CHECK_MESSAGE(checked > 40, "only ", checked, " links were checked");
}

TEST_CASE("no email address appears anywhere") {
  CHECK(has_address(text::concat("someone", "@", "example.org")));
  CHECK(has_address(text::concat("<first.last", "@", "mail.example.co.uk>")));
  CHECK_FALSE(has_address("user@host and @mention and a@b.x1"));
  REQUIRE(project_files().size() > 120);
  for (const fs::path& path : project_files()) {
    const std::string content = read(path);
    if (!is_text(content)) {
      continue;
    }
    std::size_t number = 0;
    for (const std::string& line : lines_of(content)) {
      ++number;
      CHECK_MESSAGE(!has_address(line), shown(path), ":", number, " contains an email address");
    }
  }
}

TEST_CASE("source files have no comments") {
  const std::string marker = text::concat("/", "/ note");
  const std::string block = text::concat("/", "* note *", "/");
  CHECK(comment_line(text::concat("int main() {}\n", marker, "\n")) == std::optional<std::size_t>(2));
  CHECK(comment_line(text::concat("int value = 1;\nint other = 2; ", block, "\n")) == std::optional<std::size_t>(2));
  CHECK(comment_line(text::concat("auto text = \"", marker, "\";\nchar slash = '/';\nint big = 1'000'000;\n")) ==
        std::nullopt);
  CHECK(comment_line(text::concat("auto raw = R\"sql(\n", marker, "\n)sql\";\n", marker)) ==
        std::optional<std::size_t>(4));
  CHECK(comment_line(text::concat("char quote = '\\'';\nchar mark = '\"';\n", marker)) ==
        std::optional<std::size_t>(3));
  std::size_t sources = 0;
  for (const fs::path& path : project_files()) {
    const std::string name = shown(path);
    if (name.starts_with("third_party/") || name.starts_with("testdata/")) {
      continue;
    }
    const std::string kind = extension(path);
    const std::string file = files::shown(path.filename());
    std::optional<std::size_t> found;
    if (is_source(path)) {
      ++sources;
      found = comment_line(read(path));
    } else if (kind == ".sql") {
      found = statement_comment_line(read(path));
    } else if (kind == ".sh" || kind == ".bash" || kind == ".bats") {
      found = marked_line(read(path), true);
    } else if (kind == ".cmake" || file == "CMakeLists.txt" || file == "Makefile" || kind == ".mk") {
      found = marked_line(read(path), false);
    }
    CHECK_MESSAGE(!found.has_value(), name, " contains a comment on line ", found.value_or(0));
  }
  CHECK_MESSAGE(sources > 100, "only ", sources, " source files were scanned");
}

TEST_CASE("the configuration document lists every setting") {
  REQUIRE(fs::is_regular_file(testkit::source_root() / "docs" / "CONFIGURATION.md"));
  const std::string document = read_named("docs/CONFIGURATION.md");
  std::map<std::string, std::string, std::less<>> documented;
  for (const std::string& line : lines_of(document)) {
    const std::vector<std::string> cells = text::split(line, '|');
    if (cells.size() < 4 || !text::trim(cells[1]).starts_with('`')) {
      continue;
    }
    std::string key(text::trim(cells[1]));
    key = key.substr(1, key.size() > 1 ? key.size() - 2 : 0);
    if (settings::lookup(key) != nullptr) {
      documented[key] = std::string(text::trim(cells[2]));
      continue;
    }
    const bool setting_like = text::contains(key, ".") && key.find_first_of(" -<") == std::string::npos;
    CHECK_MESSAGE(!setting_like, "docs/CONFIGURATION.md documents ", key, ", which is not a setting");
  }
  for (const settings::Definition& definition : settings::definitions()) {
    const auto value = documented.find(definition.key);
    REQUIRE_MESSAGE(value != documented.end(), "docs/CONFIGURATION.md does not document ", definition.key);
    const std::string wanted =
        definition.fallback.empty() ? std::string("empty") : text::concat("`", definition.fallback, "`");
    CHECK_MESSAGE(value->second == wanted, "docs/CONFIGURATION.md gives another default for ", definition.key);
    for (const std::string_view option : definition.options) {
      CHECK_MESSAGE(text::contains(document, text::concat("`", option, "`")),
                    "docs/CONFIGURATION.md does not mention the value ", option, " of ", definition.key);
    }
  }
}

TEST_CASE("tests named in documents exist") {
  CHECK(cited_test("\"a dry run reports and changes nothing\"") ==
        std::optional<std::string>("a dry run reports and changes nothing"));
  CHECK(cited_test("\"one word\"") == std::nullopt);
  CHECK(cited_test("make test-nogui and more") == std::nullopt);
  const std::set<std::string, std::less<>> declared = declared_tests();
  CHECK_MESSAGE(declared.size() >= 150, "only ", declared.size(), " tests were found");
  std::size_t mentioned = 0;
  for (const fs::path& path : project_files()) {
    if (!is_document(path)) {
      continue;
    }
    for (const std::string& span : code_spans(read(path))) {
      const std::optional<std::string> name = cited_test(span);
      if (!name) {
        continue;
      }
      ++mentioned;
      CHECK_MESSAGE(declared.contains(*name), shown(path), " names the test \"", *name, "\", which does not exist");
    }
  }
  CHECK_MESSAGE(mentioned >= 30, "only ", mentioned, " test names were found in the documents");
}

TEST_CASE("the generated command tables are current") {
  const std::string current = read_named("docs/COMMANDS.md");
  const bool same = app::cli::commands_document(current) == current;
  CHECK_MESSAGE(same, "docs/COMMANDS.md is out of date: run make docs");
}

TEST_CASE("sql stays in the storage modules and is not built from text") {
  CHECK(builds_a_statement("sql += \" ORDER BY name\";"));
  CHECK(builds_a_statement("auto sql = text::concat(\"SELECT id FROM records WHERE name = '\", name, \"'\");"));
  CHECK_FALSE(builds_a_statement("constexpr std::string_view sql_get = \"SELECT id FROM records WHERE id = ?1\";"));
  CHECK_FALSE(builds_a_statement("const std::string shown = text::concat(count, \" records\");"));
  const std::string binding = text::concat("sqli", "te3");
  std::size_t scanned = 0;
  for (const fs::path& path : project_files()) {
    if (!is_source(path)) {
      continue;
    }
    const std::string name = shown(path);
    const std::string source = read(path);
    if (listed(storage_modules, name)) {
      ++scanned;
      std::size_t number = 0;
      for (const std::string& line : lines_of(source)) {
        ++number;
        CHECK_MESSAGE(!builds_a_statement(line), name, ":", number, " builds SQL from text");
      }
    } else if (!listed(database_users, name)) {
      CHECK_MESSAGE(!text::contains(source, binding), name, " uses the database outside the storage modules");
    }
  }
  CHECK(scanned == storage_modules.size());
}

TEST_CASE("the version is the same everywhere") {
  const std::string version(text::trim(read_named("VERSION")));
  CHECK(version == meta::version());
  CHECK_MESSAGE(text::contains(read_named("scripts/lib.sh"), text::concat("APP_VERSION=\"", version, "\"\n")),
                "scripts/lib.sh has another APP_VERSION");
  REQUIRE(fs::is_regular_file(testkit::source_root() / "docs" / "RELEASE_NOTES.md"));
  REQUIRE(fs::is_regular_file(testkit::source_root() / "CHANGELOG.md"));
  const std::vector<std::string> notes = lines_of(read_named("docs/RELEASE_NOTES.md"));
  const std::string title = notes.empty() ? std::string() : notes.front();
  CHECK_MESSAGE(text::contains(title, text::concat(" v", version, " ")),
                "docs/RELEASE_NOTES.md is titled for another version: ", title);
  CHECK_MESSAGE(text::contains(read_named("CHANGELOG.md"), text::concat("## [", version, "]")),
                "CHANGELOG.md has no section for ", version);
}
