#include "app/cli.hpp"

#include <algorithm>
#include <istream>
#include <optional>
#include <ostream>
#include <utility>

#include <nlohmann/json.hpp>

#include "app/command.hpp"
#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"

namespace app::cli {

namespace {

constexpr std::size_t longest_confirmation = 256;

bool confirmation_is_off(const service::App* app) {
  if (app == nullptr) {
    return false;
  }
  try {
    return !app->settings().flag(settings::key_confirm);
  } catch (const AppError&) {
    return false;
  }
}

const std::vector<Flag>& global_flags() {
  static const std::vector<Flag> flags = {
      text_flag("db", "", "database file to use for this run"),
      text_flag("set", "", "override a setting for this run, as key=value (repeatable)"),
      text_flag("ui", "", "open the dashboard (gui) or the terminal UI (tui)"),
      toggle("ask", "ask which front end to open even when a choice is remembered"),
      toggle("no-prompt", "never ask, open the remembered or default front end"),
      toggle("help", "show help"),
      toggle("version", "print the version"),
  };
  return flags;
}

std::optional<bool> parse_switch(std::string_view value) {
  const std::string lowered = text::lower(value);
  if (lowered == "true" || lowered == "1" || lowered == "t" || lowered == "yes" || lowered == "on") {
    return true;
  }
  if (lowered == "false" || lowered == "0" || lowered == "f" || lowered == "no" || lowered == "off") {
    return false;
  }
  return std::nullopt;
}

struct Spelled {
  std::string_view name;
  std::optional<std::string> inline_value;
};

Spelled spell(std::string_view argument) {
  argument.remove_prefix(argument.starts_with("--") ? 2 : 1);
  const std::size_t split = argument.find('=');
  if (split == std::string_view::npos) {
    return {argument, std::nullopt};
  }
  return {argument.substr(0, split), std::string(argument.substr(split + 1))};
}

const Flag* find_flag(const std::vector<Flag>& flags, std::string_view name) {
  const auto found = std::ranges::find(flags, name, &Flag::name);
  return found == flags.end() ? nullptr : &*found;
}

bool switch_value(std::string_view name, const std::optional<std::string>& inline_value) {
  if (!inline_value) {
    return true;
  }
  const std::optional<bool> parsed = parse_switch(*inline_value);
  if (!parsed) {
    throw usage(text::concat("flag --", name, " needs true or false, found \"", *inline_value, "\""));
  }
  return *parsed;
}

std::string taken_value(std::string_view name, std::optional<std::string> inline_value,
                        const std::vector<std::string>& arguments, std::size_t& index) {
  if (inline_value) {
    return std::move(*inline_value);
  }
  if (index >= arguments.size()) {
    throw usage(text::concat("flag --", name, " needs a value"));
  }
  return arguments[index++];
}

struct GlobalsParsed {
  Globals globals;
  std::vector<std::string> rest;
};

GlobalsParsed parse_globals(const std::vector<std::string>& arguments) {
  GlobalsParsed parsed;
  Globals& globals = parsed.globals;
  std::size_t index = 0;
  while (index < arguments.size()) {
    const std::string& argument = arguments[index];
    if (argument == "--") {
      ++index;
      break;
    }
    if (!argument.starts_with('-') || argument == "-") {
      break;
    }
    ++index;
    Spelled spelled = spell(argument);
    const std::string_view name = spelled.name == "h" ? std::string_view("help") : spelled.name;
    const Flag* flag = find_flag(global_flags(), name);
    if (flag == nullptr) {
      throw usage(text::concat("unknown flag --", name));
    }
    if (flag->kind == Kind::Switch) {
      const bool value = switch_value(name, spelled.inline_value);
      if (name == "ask") {
        globals.ask = value;
      } else if (name == "no-prompt") {
        globals.no_prompt = value;
      } else if (name == "help") {
        globals.help = value;
      } else {
        globals.version = value;
      }
      continue;
    }
    std::string value = taken_value(name, std::move(spelled.inline_value), arguments, index);
    if (name == "db") {
      globals.database = std::move(value);
    } else if (name == "ui") {
      globals.ui = std::move(value);
    } else {
      const std::size_t split = value.find('=');
      if (split == std::string::npos || split == 0) {
        throw usage(text::concat("invalid value \"", value, "\" for flag --set: use the form key=value"));
      }
      const std::string key = value.substr(0, split);
      if (settings::lookup(key) == nullptr) {
        throw usage(text::concat("invalid value \"", value, "\" for flag --set: unknown setting \"", key, "\": run \"",
                                 meta::slug, " config list\" to see the keys"));
      }
      globals.overrides[key] = value.substr(split + 1);
    }
  }
  parsed.rest.assign(arguments.begin() + static_cast<std::ptrdiff_t>(index), arguments.end());
  return parsed;
}

std::string version_line() {
  return text::concat(meta::slug, " ", meta::version());
}

Command root() {
  return Command::group(
      meta::slug, "",
      {
          overview_command(),
          records_command(),
          demo_command(),
          notifications_command(),
          notifier_command(),
          config_command(),
          database_command(),
          Command::leaf("diagnostics", "Show build, database and settings status", run_diagnostics),
          Command::standalone("guide", "Read or search the usage guide", run_guide).taking("[search terms]"),
          Command::leaf("about", "Show version, source and bug report links", run_about)
              .with({toggle("issue", "print the bug report preview and its link")}),
          Command::standalone("licenses", "Show the license and third-party notices", run_licenses),
          Command::standalone("version", "Print the version", [](Env& env, const Call&) { line(env, version_line()); }),
      });
}

std::vector<std::string> describe_flags(const std::vector<Flag>& flags) {
  std::vector<Flag> sorted = flags;
  std::ranges::sort(sorted, {}, &Flag::name);
  std::vector<std::string> lines;
  for (const Flag& flag : sorted) {
    std::string described = text::concat("  --", flag.name);
    if (flag.kind != Kind::Switch) {
      described.append(" <value>");
    }
    described.push_back('\t');
    described.append(flag.help);
    if (!flag.fallback.empty() && flag.fallback != "false" && flag.fallback != "0") {
      described.append(text::concat(" (default ", flag.fallback, ")"));
    }
    lines.push_back(std::move(described));
  }
  return lines;
}

std::string help_text(const Command& target, std::string_view path) {
  std::string out;
  if (path.empty()) {
    out.append(text::concat(meta::app_name, " ", meta::version(), "\n", meta::tagline, "\n\n"));
    out.append(text::concat("Usage:\n  ", meta::slug, "\topen the dashboard or the terminal UI\n  ", meta::slug,
                            " <command> [arguments]\trun a command\n\nCommands:\n"));
  } else {
    std::string usage_line = text::concat(meta::slug, " ", path);
    if (!target.sub.empty()) {
      usage_line.append(" <command>");
    }
    if (!target.args.empty()) {
      usage_line.push_back(' ');
      usage_line.append(target.args);
    }
    if (!target.flags.empty()) {
      usage_line.append(" [flags]");
    }
    out.append(text::concat(target.summary, "\n\nUsage:\n  ", usage_line, "\n"));
    if (!target.sub.empty()) {
      out.append("\nCommands:\n");
    }
  }
  for (const Command& child : target.sub) {
    out.append(text::concat("  ", child.name, "\t", child.summary, "\n"));
  }
  if (!path.empty() && !target.flags.empty()) {
    out.append("\nFlags:\n");
    for (const std::string& described : describe_flags(target.flags)) {
      out.append(described);
      out.push_back('\n');
    }
  }
  out.append("\nGlobal flags:\n");
  for (const std::string& described : describe_flags(global_flags())) {
    out.append(described);
    out.push_back('\n');
  }
  if (path.empty()) {
    out.append(text::concat("\nEnvironment:\n  ", meta::env_name("ui"), "\tgui or tui, same as --ui\n  ",
                            meta::env_name("db"), "\tdatabase file, same as --db\n  ", meta::env_name("config_dir"),
                            "\tdirectory for the saved database path and front-end choice\n  NO_COLOR\tturn colors "
                            "off in the terminal UI\n"));
    out.append(text::concat("\nRun \"", meta::slug, " help <command>\" for details.\n"));
  }
  return tabulate(out);
}

struct Leaf {
  std::string path;
  const Command* command = nullptr;
};

void leaves(const Command& parent, std::string_view prefix, std::vector<Leaf>& all) {
  for (const Command& child : parent.sub) {
    const std::string path = prefix.empty() ? std::string(child.name) : text::concat(prefix, " ", child.name);
    if (child.sub.empty()) {
      all.push_back({path, &child});
    }
    leaves(child, path, all);
  }
}

std::string joined_path(std::string_view path, std::string_view name) {
  return std::string(text::trim(text::concat(path, " ", name)));
}

std::string suggestion(const Command& parent, std::string_view path, std::string_view word) {
  for (const Command& child : parent.sub) {
    if (child.name.starts_with(word) || word.starts_with(child.name)) {
      return text::concat("Did you mean \"", joined_path(path, child.name), "\"?");
    }
  }
  std::vector<std::string> names;
  for (const Command& child : parent.sub) {
    names.emplace_back(child.name);
  }
  return text::concat("Available: ", text::join(names, ", "));
}

int report(Env& env, const AppError& error) {
  note(env, text::concat("error: ", error.message()));
  return error.is_usage() ? exit_usage : exit_failure;
}

void execute(Env& env, Globals globals, const Command& target, std::string_view path, std::string parent,
             const std::vector<std::string>& arguments) {
  Parsed parsed;
  try {
    parsed = parse_flags(target.flags, arguments);
  } catch (const AppError& problem) {
    throw usage(text::concat(problem.message(), ": run \"", meta::slug, " help ", path, "\" to see the flags"));
  }
  const Call call{std::move(globals), std::move(parent), std::move(parsed.positional), std::move(parsed.values),
                  std::move(parsed.given)};
  if (target.without_app) {
    target.without_app(env, call);
    return;
  }
  service::App opened = service::App::open(app_options(env, call.globals, false));
  if (opened.created_database()) {
    note(env, text::concat("Created a new database at ", files::shown(opened.location().path)));
  }
  target.with_app(env, call, opened);
}

void terminal_window(Env& env, const Globals& globals) {
  if (!env.hooks.open_terminal_window) {
    throw failure("no terminal window can be opened from here: start the application from a terminal with --ui tui");
  }
  std::vector<std::string> arguments = {"--ui", "tui"};
  if (!globals.database.empty()) {
    arguments.emplace_back("--db");
    arguments.push_back(globals.database);
  }
  for (const auto& [key, value] : globals.overrides) {
    arguments.emplace_back("--set");
    arguments.push_back(text::concat(key, "=", value));
  }
  env.hooks.open_terminal_window(env.executable, arguments);
}

int launch(Env& env, const Globals& globals) {
  const service::Location location = service::locate(app_options(env, globals, false));
  launcher::Input base;
  base.ui_flag = globals.ui;
  base.ask = globals.ask;
  base.no_prompt = globals.no_prompt;
  base.env_ui = env.options.env.get(meta::env_name("ui")).value_or("");
  base.remembered = location.config.front_end;
  base.gui_available = static_cast<bool>(env.hooks.open_gui);
  const launcher::Input input = env.session.input(std::move(base));
  const launcher::Decision decision = launcher::decide(input);
  bool remember = false;
  std::string mode;
  switch (decision.action) {
    case launcher::Action::Fail:
      throw usage(decision.reason);
    case launcher::Action::PrintHelp:
    case launcher::Action::RunCommand:
      note(env, text::concat(meta::app_name, ": ", decision.reason, ".\n"));
      note(env, text::trim_whitespace(help_text(root(), "")));
      return exit_ok;
    case launcher::Action::OpenGui:
      mode = std::string(launcher::mode_gui);
      break;
    case launcher::Action::OpenTui:
      if (input.headless && globals.ui.empty()) {
        note(env, text::concat(meta::app_name, ": ", decision.reason, "."));
      }
      mode = std::string(launcher::mode_tui);
      break;
    case launcher::Action::OpenTerminalWindow:
      terminal_window(env, globals);
      return exit_ok;
    case launcher::Action::PromptTerminal:
    case launcher::Action::ShowChooser: {
      launcher::PromptResult asked;
      if (decision.action == launcher::Action::PromptTerminal) {
        asked = launcher::prompt_terminal(*env.input, *env.output, decision.preferred);
      } else if (env.hooks.show_chooser) {
        asked = env.hooks.show_chooser(decision.preferred);
      } else {
        throw failure("this build cannot show the chooser window: pass --ui gui or --ui tui");
      }
      if (asked.quit) {
        return exit_ok;
      }
      remember = asked.remember;
      mode = std::move(asked.mode);
      break;
    }
  }
  service::App opened = service::App::open(app_options(env, globals, false));
  if (remember) {
    try {
      opened.remember_front_end(mode);
    } catch (const AppError& error) {
      note(env, text::concat("warning: the choice could not be saved: ", error.message()));
    }
  }
  const bool terminal = input.stdin_interactive && input.terminal_output;
  while (!mode.empty()) {
    if (mode == launcher::mode_gui && env.hooks.open_gui) {
      mode = env.hooks.open_gui(opened);
    } else if (mode == launcher::mode_tui && !terminal) {
      terminal_window(env, globals);
      return exit_ok;
    } else if (mode == launcher::mode_tui && env.hooks.open_tui) {
      mode = env.hooks.open_tui(opened);
    } else {
      throw failure(text::concat("this build cannot open the ", mode, " front end"));
    }
  }
  return exit_ok;
}

int dispatch(Env& env) {
  GlobalsParsed parsed;
  try {
    parsed = parse_globals(env.args);
  } catch (const AppError& problem) {
    note(env, text::concat("error: ", problem.message(), "\nRun \"", meta::slug, " --help\" to see the flags."));
    return exit_usage;
  }
  Globals& globals = parsed.globals;
  std::vector<std::string>& rest = parsed.rest;
  if (globals.version) {
    line(env, version_line());
    return exit_ok;
  }
  if (!rest.empty() && rest.front() == "help") {
    globals.help = true;
    rest.erase(rest.begin());
  }
  const Command tree = root();
  const Command* target = &tree;
  std::string path;
  std::string parent;
  while (!rest.empty()) {
    const std::string& word = rest.front();
    if (target->sub.empty() || word == "--help" || word == "-h") {
      break;
    }
    const Command* next = target->find(word);
    if (next == nullptr) {
      note(env,
           text::concat("error: unknown command \"", joined_path(path, word), "\"\n", suggestion(*target, path, word)));
      return exit_usage;
    }
    parent = path;
    path = joined_path(path, next->name);
    target = next;
    rest.erase(rest.begin());
  }
  for (const std::string& word : rest) {
    if (word == "--") {
      break;
    }
    if (word == "--help" || word == "-h") {
      globals.help = true;
    }
  }
  if (globals.help) {
    print(env, help_text(*target, path));
    return exit_ok;
  }
  if (path.empty()) {
    return launch(env, globals);
  }
  if (!target->runnable()) {
    note(env, text::trim_whitespace(help_text(*target, path)));
    return exit_usage;
  }
  execute(env, std::move(globals), *target, path, std::move(parent), rest);
  return exit_ok;
}

}

AppError usage(std::string message) {
  return AppError::usage(std::move(message));
}

AppError failure(std::string message) {
  return AppError::io(std::move(message));
}

Parsed parse_flags(const std::vector<Flag>& flags, const std::vector<std::string>& arguments) {
  Parsed parsed;
  for (const Flag& flag : flags) {
    parsed.values[flag.name] = std::string(flag.fallback);
  }
  std::size_t index = 0;
  while (index < arguments.size()) {
    const std::string& argument = arguments[index];
    ++index;
    if (argument == "--") {
      parsed.positional.insert(parsed.positional.end(), arguments.begin() + static_cast<std::ptrdiff_t>(index),
                               arguments.end());
      break;
    }
    if (!argument.starts_with('-') || argument == "-") {
      parsed.positional.push_back(argument);
      continue;
    }
    Spelled spelled = spell(argument);
    const Flag* flag = find_flag(flags, spelled.name);
    if (flag == nullptr) {
      throw usage(text::concat("unknown flag --", spelled.name));
    }
    std::string value;
    if (flag->kind == Kind::Switch) {
      value = switch_value(flag->name, spelled.inline_value) ? "true" : "false";
    } else {
      value = taken_value(flag->name, std::move(spelled.inline_value), arguments, index);
    }
    if (flag->kind == Kind::Number && !text::parse_integer(value)) {
      throw usage(text::concat("flag --", flag->name, " needs a whole number, found \"", value, "\""));
    }
    parsed.values[flag->name] = std::move(value);
    parsed.given.insert(flag->name);
  }
  return parsed;
}

std::string_view Call::text(std::string_view name) const {
  const auto found = values.find(name);
  return found == values.end() ? std::string_view() : std::string_view(found->second);
}

bool Call::given(std::string_view name) const {
  return given_flags.contains(name);
}

bool Call::truth(std::string_view name) const {
  return text(name) == "true";
}

std::int64_t Call::number(std::string_view name) const {
  return text::parse_integer(text(name)).value_or(0);
}

void Call::no_arguments() const {
  if (!args.empty()) {
    throw usage(text::concat("unexpected argument \"", args.front(), "\""));
  }
}

std::int64_t Call::identifier(std::string_view what) const {
  if (args.size() != 1) {
    throw usage(
        text::concat("give exactly one ", what, " identifier: run \"", meta::slug, " ", parent, " list\" to see them"));
  }
  const std::optional<std::int64_t> id = text::parse_integer(args.front());
  if (!id || *id <= 0) {
    throw usage(
        text::concat("\"", args.front(), "\" is not a ", what, " identifier: use the number from the ID column"));
  }
  return *id;
}

Command Command::group(std::string_view name, std::string_view summary, std::vector<Command> sub) {
  Command command;
  command.name = name;
  command.summary = summary;
  command.sub = std::move(sub);
  return command;
}

Command Command::leaf(std::string_view name, std::string_view summary, WithApp run) {
  Command command;
  command.name = name;
  command.summary = summary;
  command.with_app = std::move(run);
  return command;
}

Command Command::standalone(std::string_view name, std::string_view summary, WithoutApp run) {
  Command command;
  command.name = name;
  command.summary = summary;
  command.without_app = std::move(run);
  return command;
}

Command Command::taking(std::string_view arguments) && {
  args = arguments;
  return std::move(*this);
}

Command Command::with(std::vector<Flag> list) && {
  flags = std::move(list);
  return std::move(*this);
}

const Command* Command::find(std::string_view child) const {
  const auto found = std::ranges::find(sub, child, &Command::name);
  return found == sub.end() ? nullptr : &*found;
}

bool Command::runnable() const {
  return static_cast<bool>(with_app) || static_cast<bool>(without_app);
}

std::string tabulate(std::string_view content) {
  const std::vector<std::string> lines = text::split(content, '\n');
  std::string out;
  std::size_t start = 0;
  while (start < lines.size()) {
    if (!text::contains(lines[start], "\t")) {
      out.append(lines[start]);
      if (start + 1 < lines.size()) {
        out.push_back('\n');
      }
      ++start;
      continue;
    }
    std::size_t end = start;
    while (end < lines.size() && text::contains(lines[end], "\t")) {
      ++end;
    }
    std::vector<std::vector<std::string>> rows;
    for (std::size_t index = start; index < end; ++index) {
      rows.push_back(text::split(lines[index], '\t'));
    }
    std::vector<std::size_t> widths;
    for (const std::vector<std::string>& row : rows) {
      for (std::size_t column = 0; column + 1 < row.size(); ++column) {
        if (widths.size() <= column) {
          widths.resize(column + 1, 0);
        }
        widths[column] = std::max(widths[column], text::length(row[column]) + 2);
      }
    }
    for (const std::vector<std::string>& row : rows) {
      std::string aligned;
      for (std::size_t column = 0; column < row.size(); ++column) {
        aligned.append(row[column]);
        if (column + 1 < row.size()) {
          const std::size_t width = column < widths.size() ? widths[column] : 0;
          const std::size_t used = text::length(row[column]);
          aligned.append(width > used ? width - used : 0, ' ');
        }
      }
      out.append(text::trim_whitespace(text::concat("|", aligned)).substr(1));
      out.push_back('\n');
    }
    start = end;
    if (start == lines.size()) {
      out.pop_back();
    }
  }
  return out;
}

void print(Env& env, std::string_view content) {
  env.output->write(content.data(), static_cast<std::streamsize>(content.size()));
}

void line(Env& env, std::string_view content) {
  print(env, content);
  print(env, "\n");
}

void note(Env& env, std::string_view content) {
  env.errors->write(content.data(), static_cast<std::streamsize>(content.size()));
  env.errors->put('\n');
}

void table(Env& env, const std::vector<std::string_view>& header, const std::vector<std::vector<std::string>>& rows) {
  std::string content;
  for (std::size_t index = 0; index < header.size(); ++index) {
    if (index > 0) {
      content.push_back('\t');
    }
    content.append(header[index]);
  }
  content.push_back('\n');
  for (const std::vector<std::string>& row : rows) {
    for (std::size_t index = 0; index < row.size(); ++index) {
      if (index > 0) {
        content.push_back('\t');
      }
      for (const char letter : row[index]) {
        content.push_back(letter == '\t' || letter == '\n' || letter == '\r' ? ' ' : letter);
      }
    }
    content.push_back('\n');
  }
  print(env, tabulate(content));
}

void json(Env& env, const nlohmann::ordered_json& value) {
  line(env, value.dump(2, ' ', false, nlohmann::ordered_json::error_handler_t::replace));
}

bool interactive(const Env& env) {
  return env.session.stdin_terminal && !env.session.ci();
}

void confirm(Env& env, const Call& call, const service::App* app, std::string_view statement) {
  if (call.truth("yes")) {
    return;
  }
  if (confirmation_is_off(app)) {
    return;
  }
  if (!interactive(env)) {
    throw usage(text::concat(statement,
                             " This needs confirmation: repeat the command with --yes, or use --dry-run to preview"));
  }
  *env.errors << statement << "\nType yes to continue: ";
  env.errors->flush();
  std::string answer;
  while (answer.size() < longest_confirmation) {
    const int next = env.input->get();
    if (next == std::istream::traits_type::eof() || next == '\n') {
      break;
    }
    answer.push_back(static_cast<char>(next));
  }
  if (text::lower(text::trim_whitespace(answer)) != "yes") {
    throw failure("cancelled, nothing was changed");
  }
}

service::Options app_options(const Env& env, const Globals& globals, bool read_only) {
  service::Options options = env.options;
  if (globals.database.empty()) {
    options.database_path.reset();
  } else {
    options.database_path = files::from_utf8(globals.database);
  }
  options.overrides = globals.overrides;
  options.read_only = read_only;
  return options;
}

std::string_view enabled_text(bool enabled) {
  return enabled ? "yes" : "no";
}

std::string capitalize(std::string_view content) {
  std::string out(content);
  if (!out.empty() && out.front() >= 'a' && out.front() <= 'z') {
    out.front() = static_cast<char>(out.front() - 'a' + 'A');
  }
  return out;
}

int run(Env& env) {
  try {
    return dispatch(env);
  } catch (const AppError& error) {
    return report(env, error);
  }
}

std::vector<std::string> command_paths() {
  const Command tree = root();
  std::vector<Leaf> all;
  leaves(tree, "", all);
  std::vector<std::string> paths;
  for (Leaf& leaf : all) {
    paths.push_back(std::move(leaf.path));
  }
  std::ranges::sort(paths);
  return paths;
}

std::string reference() {
  const Command tree = root();
  std::vector<Leaf> all;
  leaves(tree, "", all);
  std::string out = "| Command | Arguments and flags | What it does |\n|---|---|---|\n";
  for (const Leaf& leaf : all) {
    std::vector<std::string> parts;
    if (!leaf.command->args.empty()) {
      parts.push_back(text::concat("`", text::replace_all(leaf.command->args, "|", "\\|"), "`"));
    }
    std::vector<std::string_view> names;
    for (const Flag& flag : leaf.command->flags) {
      names.push_back(flag.name);
    }
    std::ranges::sort(names);
    for (const std::string_view name : names) {
      parts.push_back(text::concat("`--", name, "`"));
    }
    if (parts.empty()) {
      parts.emplace_back("none");
    }
    out.append(text::concat("| `", meta::slug, " ", leaf.path, "` | ", text::join(parts, " "), " | ",
                            leaf.command->summary, " |\n"));
  }
  return out;
}

namespace {

std::string replace_block(std::string_view document, std::string_view name, std::string_view content) {
  const std::string start = text::concat("<!-- ", name, ":start -->\n");
  const std::string end = text::concat("<!-- ", name, ":end -->");
  const std::size_t from = document.find(start);
  if (from == std::string_view::npos) {
    throw failure(text::concat("docs/COMMANDS.md lacks the ", name, " start marker"));
  }
  const std::size_t to = document.find(end);
  if (to == std::string_view::npos) {
    throw failure(text::concat("docs/COMMANDS.md lacks the ", name, " end marker"));
  }
  if (to < from) {
    throw failure(text::concat("docs/COMMANDS.md has the ", name, " markers in the wrong order"));
  }
  return text::concat(document.substr(0, from + start.size()), content, document.substr(to));
}

}

std::string commands_document(std::string_view current) {
  return replace_block(replace_block(current, "capabilities", service::capability_table()), "reference", reference());
}

}
