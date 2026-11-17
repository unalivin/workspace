#include "tui/model.hpp"

#include <algorithm>
#include <array>
#include <utility>

#include "core/files.hpp"
#include "core/meta.hpp"
#include "core/text.hpp"
#include "core/userconfig.hpp"

namespace app::tui {

namespace {

constexpr std::size_t max_notice_lines = 3;
constexpr std::string_view busy = "The database is busy in another window. Nothing was lost; press r to try again.";
constexpr std::string_view no_dashboard_hint = "Not available: this build has no dashboard";
constexpr std::string_view no_dashboard =
    "This build has no dashboard. Download the standard build to use the dashboard window.";

enum class MenuAction { Dashboard, Guide, Shortcuts, Settings, Refresh, Sidebar, Quit };

struct MenuEntry {
  std::string_view label;
  std::string_view hint;
  MenuAction action = MenuAction{};
};

constexpr std::array<MenuEntry, 7> menu = {{
    {"Open dashboard", "Close the terminal UI and open the dashboard window", MenuAction::Dashboard},
    {"Usage guide", "Read and search the guide offline", MenuAction::Guide},
    {"Keyboard shortcuts", "List every key", MenuAction::Shortcuts},
    {"Settings", "Appearance, data and notifier", MenuAction::Settings},
    {"Refresh now", "Read the database again", MenuAction::Refresh},
    {"Collapse or expand the sidebar", "", MenuAction::Sidebar},
    {"Quit", "", MenuAction::Quit},
}};

constexpr auto& nav = service::navigation;

std::size_t less(std::size_t value, std::size_t amount) {
  return value > amount ? value - amount : 0;
}

}

Model::Model(service::App& app, Options options)
    : app_(&app), options_(std::move(options)), welcome_(app.created_database()) {
  reload();
  sidebar_ = setting_flag(settings::key_sidebar_collapsed);
}

std::string_view Model::switch_to() const {
  return switch_to_;
}

bool Model::finished() const {
  return quitting_ || !switch_to_.empty();
}

Notice Model::notice_kind() const {
  return kind_;
}

const std::string& Model::notice_text() const {
  return notice_;
}

void Model::resize(std::size_t columns, std::size_t rows) {
  width_ = columns;
  height_ = rows;
}

std::chrono::seconds Model::interval() const {
  const settings::Value* value = setting(settings::key_refresh_seconds);
  const std::int64_t seconds = value == nullptr ? 0 : value->number();
  return std::chrono::seconds(seconds < 1 ? 2 : seconds);
}

theme::Style Model::style(Look look) const {
  if (!terminal_) {
    return {};
  }
  switch (look) {
    case Look::Plain:
      return terminal_->text;
    case Look::Muted:
      return terminal_->muted;
    case Look::Accent:
      return terminal_->accent;
    case Look::Heading:
      return terminal_->heading;
    case Look::Selected:
      return terminal_->selected;
    case Look::Border:
      return terminal_->border;
    case Look::Status:
      return terminal_->status_bar;
    case Look::Success:
      return terminal_->success;
    case Look::Warning:
      return terminal_->warning;
    case Look::Failure:
      return terminal_->error;
    case Look::Info:
      return terminal_->severity(theme::Severity::Info);
    case Look::Low:
      return terminal_->severity(theme::Severity::Low);
    case Look::Medium:
      return terminal_->severity(theme::Severity::Medium);
    case Look::High:
      return terminal_->severity(theme::Severity::High);
    case Look::Critical:
      return terminal_->severity(theme::Severity::Critical);
  }
  return terminal_->text;
}

theme::Profile Model::profile() const {
  return terminal_ ? terminal_->profile : theme::Profile::Mono;
}

const RecordsPage& Model::records_page() const {
  return rec_;
}

const InboxPage& Model::inbox_page() const {
  return inbox_;
}

const SettingsPage& Model::settings_page() const {
  return set_;
}

const AboutPage& Model::about_page() const {
  return about_;
}

const Form* Model::form() const {
  return form_ ? &form_->form : nullptr;
}

const Confirmation* Model::confirmation() const {
  return confirm_ ? &*confirm_ : nullptr;
}

const Preview* Model::preview() const {
  return preview_ ? &*preview_ : nullptr;
}

void Model::say(Notice kind, std::string text) {
  kind_ = kind;
  notice_ = std::move(text);
}

void Model::report(const AppError& error) {
  if (error.is_busy()) {
    say(Notice::Warning, std::string(busy));
  } else {
    say(Notice::Failure, error.what());
  }
}

const settings::Value* Model::setting(std::string_view key) const {
  const auto found = values_.find(key);
  return found == values_.end() ? nullptr : &found->second;
}

bool Model::setting_flag(std::string_view key) const {
  const settings::Value* value = setting(key);
  return value != nullptr && value->flag();
}

void Model::restyle() {
  attempt([this] {
    auto values = app_->settings().effective();
    theme::Tokens tokens = app_->appearance(options_.system_dark);
    const auto monochrome = values.find(settings::key_monochrome);
    const bool plain = monochrome != values.end() && monochrome->second.flag();
    terminal_ = theme::for_terminal(tokens, theme::detect_profile(options_.env, plain));
    warnings_ = std::move(tokens.warnings);
    values_ = std::move(values);
  });
}

void Model::reload() {
  restyle();
  if (!attempt([this] { overview_ = app_->records().overview(); })) {
    return;
  }
  load_records();
  load_inbox();
  load_settings();
  load_about();
  try {
    static_cast<void>(app_->poll());
  } catch (const AppError&) {
    return;
  }
}

void Model::changed(std::string text) {
  reload();
  say(Notice::Success, std::move(text));
}

void Model::tick() {
  try {
    if (app_->poll()) {
      reload();
      if (!form_) {
        say(Notice::Info, "Updated by another window");
      }
    }
  } catch (const AppError& error) {
    if (!error.is_busy()) {
      report(error);
    }
  }
}

std::string_view Model::page_id() const {
  return nav.at(page_).id;
}

void Model::open(std::string_view id) {
  const auto found = std::ranges::find(nav, id, &service::NavItem::id);
  if (found != nav.end()) {
    page_ = static_cast<std::size_t>(found - nav.begin());
  }
}

void Model::ask(std::string_view title, std::string statement, std::string_view accept, Action action) {
  if (!setting_flag(settings::key_confirm)) {
    run(action);
    return;
  }
  confirm_ = Confirmation{std::string(title), std::move(statement), std::string(accept), std::move(action)};
}

void Model::run(const Action& action) {
  std::string done;
  if (attempt([&] { done = perform(action); })) {
    changed(std::move(done));
  }
}

std::string Model::perform(const Action& action) {
  if (const auto* record = std::get_if<RemoveRecord>(&action)) {
    app_->records().remove(record->id);
    return text::concat("Removed record ", record->id);
  }
  if (std::holds_alternative<RemoveDemo>(action)) {
    return text::concat("Removed ", app_->records().remove_demo(false), " demo records");
  }
  if (std::holds_alternative<InstallNotifier>(action)) {
    static_cast<void>(autostart().install(false));
    return "The notifier starts the next time you sign in";
  }
  if (std::holds_alternative<UninstallNotifier>(action)) {
    static_cast<void>(autostart().uninstall(false));
    return "The notifier no longer starts at sign-in";
  }
  if (const auto* move = std::get_if<MoveDatabase>(&action)) {
    app_->move_database(move->plan);
    return text::concat("Now using ", files::shown(app_->location().path));
  }
  if (const auto* source = std::get_if<ImportFile>(&action)) {
    const transfer::ImportResult done = app_->import_file(files::from_utf8(source->path), source->format, false);
    rec_.query.include_disabled = true;
    return text::concat("Import finished: ", done.summary());
  }
  return open_link(std::get<OpenLink>(action).link);
}

const notifier::Autostart& Model::autostart() const {
  if (!options_.autostart) {
    throw AppError::usage("This build cannot manage the sign-in entry");
  }
  return *options_.autostart;
}

std::string Model::open_link(const service::Link& link) const {
  if (!link.allowed) {
    throw AppError::usage(link.reason);
  }
  if (!options_.open_url) {
    throw AppError::usage(text::concat("no browser can be started from here. Copy this link: ", link.url));
  }
  try {
    options_.open_url(link.url);
  } catch (const AppError& error) {
    throw AppError::io(text::concat(error.what(), ": ", link.url));
  }
  return text::concat("Opened ", link.label, " in the browser");
}

void Model::open_guide() {
  open(service::nav_about);
  about_.tab = AboutTab::Guide;
}

void Model::toggle_sidebar() {
  sidebar_ = !sidebar_;
  if (attempt([this] {
        static_cast<void>(app_->settings().set(settings::key_sidebar_collapsed, sidebar_ ? "true" : "false"));
      })) {
    try {
      static_cast<void>(app_->poll());
    } catch (const AppError&) {
      return;
    }
  }
}

void Model::load_demo() {
  std::size_t added = 0;
  if (!attempt([&] { added = app_->records().load_demo(); })) {
    return;
  }
  if (added == 0) {
    changed("The demo data is already loaded");
  } else {
    changed(text::concat("Added ", added, " demo records. Remove them in Settings, Remove demo data."));
  }
}

void Model::choose(std::size_t entry) {
  switch (menu.at(entry).action) {
    case MenuAction::Dashboard:
      if (options_.has_dashboard) {
        switch_to_ = std::string(userconfig::front_end_gui);
      } else {
        say(Notice::Warning, std::string(no_dashboard));
      }
      break;
    case MenuAction::Guide:
      open_guide();
      break;
    case MenuAction::Shortcuts:
      open(service::nav_about);
      about_.tab = AboutTab::Shortcuts;
      break;
    case MenuAction::Settings:
      open(service::nav_settings);
      break;
    case MenuAction::Refresh:
      changed("Refreshed");
      break;
    case MenuAction::Sidebar:
      toggle_sidebar();
      break;
    case MenuAction::Quit:
      quitting_ = true;
      break;
  }
}

void Model::handle(Key key) {
  if (key.is_ctrl(U'c')) {
    quitting_ = true;
    return;
  }
  notice_.clear();
  if (welcome_) {
    welcome_ = false;
    app_->dismiss_welcome();
    if (key.is(U'l')) {
      load_demo();
    }
    return;
  }
  if (form_) {
    OpenForm open = std::move(*form_);
    form_.reset();
    switch (open.form.handle(key)) {
      case FormEvent::Open:
        form_ = std::move(open);
        break;
      case FormEvent::Cancel:
        say(Notice::Info, std::string(cancelled));
        break;
      case FormEvent::Submit:
        try {
          submit(open.form, open.submit);
        } catch (const AppError& error) {
          open.form.problem = error.what();
          form_ = std::move(open);
        }
        break;
    }
    return;
  }
  if (confirm_) {
    const Confirmation confirm = std::move(*confirm_);
    confirm_.reset();
    if (key.is(U'y') || key.is(U'Y')) {
      run(confirm.action);
    } else if (key.is(U'n') || key.is(U'N') || key.is(U'q') || key == keys::esc) {
      say(Notice::Info, std::string(cancelled));
    } else {
      confirm_ = confirm;
    }
    return;
  }
  if (preview_) {
    Preview preview = std::move(*preview_);
    preview_.reset();
    if (key == keys::up || key.is(U'k')) {
      preview.offset = less(preview.offset, 1);
    } else if (key == keys::down || key.is(U'j')) {
      preview.offset += 1;
    } else if (key == keys::page_up) {
      preview.offset = less(preview.offset, 10);
    } else if (key == keys::page_down || key.is(U' ')) {
      preview.offset += 10;
    } else if (key == keys::esc || key.is(U'q') || key.is(U'n')) {
      if (preview.action) {
        say(Notice::Info, std::string(cancelled));
      }
      return;
    } else if (key == keys::enter || key.is(U'y')) {
      if (preview.action) {
        run(*preview.action);
      }
      return;
    }
    preview_ = std::move(preview);
    return;
  }
  if (menu_) {
    const std::size_t at = *menu_;
    if (key == keys::up || key.is(U'k')) {
      menu_ = (at + menu.size() - 1) % menu.size();
    } else if (key == keys::down || key.is(U'j')) {
      menu_ = (at + 1) % menu.size();
    } else if (key == keys::esc || key.is(U'q') || key.is(U':') || key.is_function(10)) {
      menu_.reset();
    } else if (key == keys::enter) {
      menu_.reset();
      choose(at);
    }
    return;
  }
  if (captures_text()) {
    page_key(key);
    return;
  }
  if (key.is(U'q')) {
    quitting_ = true;
  } else if (key == keys::tab) {
    page_ = (page_ + 1) % nav.size();
  } else if (key == keys::back_tab) {
    page_ = (page_ + nav.size() - 1) % nav.size();
  } else if (key.is(U',')) {
    open(service::nav_settings);
  } else if (key.is(U'?') || key.is_function(1)) {
    open_guide();
  } else if (key.is(U'r')) {
    changed("Refreshed");
  } else if (key.is(U'b')) {
    toggle_sidebar();
  } else if (key.is(U':') || key.is_function(10)) {
    menu_ = 0;
  } else if (key.kind == KeyKind::Char && key.value >= U'1' && key.value <= U'9' &&
             static_cast<std::size_t>(key.value - U'1') < nav.size()) {
    page_ = static_cast<std::size_t>(key.value - U'1');
  } else {
    page_key(key);
  }
}

bool Model::captures_text() const {
  const std::string_view page = page_id();
  if (page == service::nav_records) {
    return rec_.searching;
  }
  return page == service::nav_about && about_.searching;
}

void Model::page_key(Key key) {
  const std::string_view page = page_id();
  if (page == service::nav_records) {
    records_key(key);
  } else if (page == service::nav_notifications) {
    inbox_key(key);
  } else if (page == service::nav_settings) {
    settings_key(key);
  } else if (page == service::nav_about) {
    about_key(key);
  } else if (key.is(U'l')) {
    load_demo();
  } else if (key == keys::enter) {
    open(service::nav_records);
  }
}

void Model::submit(const Form& form, const Submit& submit) {
  switch (submit.kind) {
    case SubmitKind::AddRecord:
      submit_add(form);
      break;
    case SubmitKind::EditRecord:
      submit_edit(form, submit.id, submit.revision);
      break;
    case SubmitKind::Columns:
      submit_columns(form);
      break;
    case SubmitKind::Import:
      submit_import(form);
      break;
    case SubmitKind::Export:
      submit_export(form);
      break;
    case SubmitKind::Rule:
      submit_rule(form, submit.name);
      break;
    case SubmitKind::Setting:
      submit_setting(form, submit.name);
      break;
    case SubmitKind::MoveDatabase:
      submit_move(form);
      break;
  }
}

std::string Model::hints() const {
  constexpr std::string_view common = ": menu · ? guide · q quit";
  const std::string_view page = page_id();
  if (page == service::nav_records) {
    if (rec_.searching) {
      return "Type to search · Enter keep · Esc clear";
    }
    return text::concat(
        "n add · e edit · x enable/disable · d remove · / search · f status · v severity · s sort · "
        "c columns · a all · i import · o export · ",
        common);
  }
  if (page == service::nav_notifications) {
    if (inbox_.rules) {
      return text::concat("e edit rule · u reset rule · Left inbox · ", common);
    }
    return text::concat("m mark read · M mark all · u unread only · Enter open record · Right rules · ", common);
  }
  if (page == service::nav_settings) {
    return text::concat("Enter change · u reset · ", common);
  }
  if (page == service::nav_about) {
    if (about_.searching) {
      return "Type to search the guide · Enter keep · Esc clear";
    }
    return text::concat("Left/Right section · / search guide · Enter open · ", common);
  }
  return text::concat("l load demo data · Enter records · 1-5 pages · ", common);
}

Lines Model::view() {
  if (finished()) {
    return {};
  }
  if (width_ < min_width || height_ < min_height) {
    return {
        Line::plain(text::concat(meta::app_name, " needs a terminal of at least ", min_width, " columns and ",
                                 min_height, " rows.")),
        Line::plain(text::concat("This one is ", width_, " by ", height_,
                                 ". Enlarge the window, or use the commands: ", meta::slug, " --help")),
    };
  }
  Lines status = status_lines();
  const std::size_t body_height = height_ - 2 - status.size();
  const std::size_t sidebar_width = sidebar_ ? 4 : 20;
  const std::size_t content_width = width_ - sidebar_width - 3;
  Lines content;
  if (welcome_) {
    content = welcome_view(content_width);
  } else if (form_) {
    content = form_->form.view(content_width);
  } else if (confirm_) {
    content = confirm_view(content_width);
  } else if (preview_) {
    content = preview_view(content_width, body_height);
  } else if (menu_) {
    content = menu_view(*menu_, content_width);
  } else {
    content = page_view(content_width, body_height);
  }
  const Lines sidebar = sidebar_view(sidebar_width, body_height);
  const std::string title = text::concat(meta::app_name, " · ", nav.at(page_).label);
  const std::size_t room = less(width_, width(title) + 4);
  const std::string location = room < 8 ? std::string() : tail(files::shown(app_->location().path), room);
  const std::size_t gap = std::max<std::size_t>(less(width_, width(title) + width(location)), 1);
  Lines lines = {
      Line::styled(title, Look::Heading).with(std::string(gap, ' '), Look::Plain).with(location, Look::Muted),
      Line::styled(repeat("─", width_), Look::Border),
  };
  const Line blank;
  for (std::size_t index = 0; index + 1 < body_height; ++index) {
    Line line = (index < sidebar.size() ? sidebar[index] : blank).fit(sidebar_width).with(" │ ", Look::Border);
    Line right = (index < content.size() ? content[index] : blank).fit(content_width);
    line.spans.insert(line.spans.end(), std::make_move_iterator(right.spans.begin()),
                      std::make_move_iterator(right.spans.end()));
    lines.push_back(std::move(line));
  }
  extend(lines, std::move(status));
  return lines;
}

std::string Model::screen() {
  std::string out;
  for (const Line& line : view()) {
    std::string shown = line.text();
    while (!shown.empty() && shown.back() == ' ') {
      shown.pop_back();
    }
    out.append(shown);
    out.push_back('\n');
  }
  return out;
}

Lines Model::status_lines() const {
  if (notice_.empty()) {
    return {Line::styled(pad(text::concat(" ", short_hints(hints(), less(width_, 2))), width_), Look::Status)};
  }
  std::string_view glyph = "ℹ";
  switch (kind_) {
    case Notice::Info:
      break;
    case Notice::Success:
      glyph = "✔";
      break;
    case Notice::Warning:
      glyph = "▲";
      break;
    case Notice::Failure:
      glyph = "✖";
      break;
  }
  std::vector<std::string> parts = wrap(notice_, less(width_, 4));
  if (parts.size() > max_notice_lines) {
    parts.resize(max_notice_lines);
    parts.back().append("…");
  }
  Lines lines;
  for (std::size_t index = 0; index < parts.size(); ++index) {
    const std::string lead = index == 0 ? text::concat(" ", glyph, " ") : std::string("   ");
    lines.push_back(Line::styled(pad(text::concat(lead, parts[index]), width_), Look::Status));
  }
  return lines;
}

Lines Model::sidebar_view(std::size_t size, std::size_t height) const {
  const auto entry = [this, size](std::size_t index) {
    const service::NavItem& item = nav.at(index);
    std::string label = text::concat(index + 1, " ", item.label);
    if (sidebar_) {
      label = std::to_string(index + 1);
    } else if (item.id == service::nav_notifications && overview_.unread_notifications > 0) {
      label.append(text::concat(" (", overview_.unread_notifications, ")"));
    }
    if (index == page_) {
      return Line::styled(pad(text::concat("▌", label), size), Look::Selected);
    }
    return Line::plain(text::concat(" ", label));
  };
  const auto first_bottom = std::ranges::find(nav, true, &service::NavItem::bottom);
  const auto bottom_start = static_cast<std::size_t>(first_bottom - nav.begin());
  Lines lines;
  for (std::size_t index = 0; index < bottom_start; ++index) {
    lines.push_back(entry(index));
  }
  const std::size_t bottom = nav.size() - bottom_start;
  while (lines.size() + bottom + 1 < height) {
    lines.emplace_back();
  }
  for (std::size_t index = bottom_start; index < nav.size(); ++index) {
    lines.push_back(entry(index));
  }
  return lines;
}

Lines Model::welcome_view(std::size_t size) const {
  Lines lines = {Line::styled(text::concat("Welcome to ", meta::app_name), Look::Heading), Line{}};
  extend(lines, wrapped(text::concat("A new database was created at ", files::shown(app_->location().path),
                                     ". Everything stays in this one file on your computer. You can move it later in "
                                     "Settings, Database location."),
                        size, Look::Plain));
  lines.emplace_back();
  extend(lines, wrapped("Press l to load labeled demo data and look around.", size, Look::Plain));
  extend(lines, wrapped("Press any other key to start with an empty database.", size, Look::Plain));
  return lines;
}

Lines Model::confirm_view(std::size_t size) const {
  if (!confirm_) {
    return {};
  }
  Lines lines = {Line::styled(confirm_->title, Look::Heading), Line{}};
  extend(lines, wrapped(confirm_->statement, size, Look::Plain));
  lines.emplace_back();
  extend(lines,
         wrapped(text::concat("▲ Press y to ", confirm_->accept, ", or n or Esc to cancel."), size, Look::Warning));
  return lines;
}

Lines Model::preview_view(std::size_t size, std::size_t height) {
  if (!preview_) {
    return {};
  }
  Preview& preview = *preview_;
  std::vector<std::string> body;
  for (const std::string& line : preview.lines) {
    for (std::string& part : wrap(line, size)) {
      body.push_back(std::move(part));
    }
  }
  Lines prompt =
      preview.action
          ? wrapped(text::concat("▲ Press Enter to ", preview.accept, ", or Esc to cancel. Up and Down scroll."), size,
                    Look::Warning)
          : wrapped("Up and Down scroll · Esc close", size, Look::Muted);
  const std::size_t room = std::max<std::size_t>(less(height, 5 + prompt.size()), 1);
  preview.offset = std::min(preview.offset, less(body.size(), room));
  Lines lines = {Line::styled(preview.title, Look::Heading), Line{}};
  for (std::size_t index = preview.offset; index < body.size() && index < preview.offset + room; ++index) {
    lines.push_back(Line::plain(std::move(body[index])));
  }
  lines.emplace_back();
  extend(lines, std::move(prompt));
  return lines;
}

Lines Model::menu_view(std::size_t at, std::size_t size) const {
  Lines lines = {Line::styled("Menu", Look::Heading), Line{}};
  for (std::size_t index = 0; index < menu.size(); ++index) {
    const MenuEntry& entry = menu.at(index);
    const std::string_view hint =
        entry.action == MenuAction::Dashboard && !options_.has_dashboard ? no_dashboard_hint : entry.hint;
    Line line = index == at ? Line::styled(pad(text::concat("▌ ", entry.label), 34), Look::Selected)
                            : Line::plain(pad(text::concat("  ", entry.label), 34));
    lines.push_back(std::move(line).with(std::string(hint), Look::Muted).fit(size));
  }
  lines.emplace_back();
  extend(lines, wrapped("Up and Down move · Enter choose · Esc close", size, Look::Muted));
  return lines;
}

Lines Model::page_view(std::size_t size, std::size_t height) {
  const std::size_t room = less(height, 1);
  const std::string_view page = page_id();
  if (page == service::nav_records) {
    return records_view(size, room);
  }
  if (page == service::nav_notifications) {
    return inbox_view(size, room);
  }
  if (page == service::nav_settings) {
    return settings_view(size, room);
  }
  if (page == service::nav_about) {
    return about_view(size, room);
  }
  return overview_view(size);
}

Lines Model::overview_view(std::size_t size) const {
  const auto card = [](std::string_view label, std::int64_t value, std::string_view hint) {
    return Line::plain(pad(label, 24))
        .with(pad(std::to_string(value), 6), Look::Accent)
        .with(std::string(hint), Look::Muted);
  };
  Lines lines = {
      Line::styled("What needs attention", Look::Heading),
      Line{},
      card("Open records", overview_.open, "not done and enabled"),
      card("Urgent records", overview_.urgent, "open with high or critical severity"),
      card("Unread notifications", overview_.unread_notifications, "press 3 to read them"),
      card("Disabled records", overview_.disabled, "kept but not counted"),
      Line{},
  };
  if (overview_.total() == 0) {
    extend(lines, wrapped("No records yet. Press 2 and then n to add the first record, or press l to load labeled "
                          "demo data.",
                          size, Look::Plain));
    return lines;
  }
  lines.push_back(Line::styled("Enabled records by status and severity", Look::Heading));
  lines.emplace_back();
  const std::vector<std::size_t> widths = {14, 12, 8};
  lines.push_back(Line::styled(row({"STATUS", "SEVERITY", "RECORDS"}, widths), Look::Muted));
  for (const records::Breakdown& item : overview_.breakdown) {
    lines.push_back(Line::plain(
        row({text::concat(records::glyph(item.status), " ", records::label(item.status)),
             text::concat(theme::glyph(item.severity), " ", theme::label(item.severity)), std::to_string(item.total)},
            widths)));
  }
  if (overview_.demo > 0) {
    lines.emplace_back();
    extend(lines, wrapped(text::concat("ℹ Demo data is loaded: ", overview_.demo,
                                       " sample records whose names start with \"Demo:\". Remove them in Settings, "
                                       "Remove demo data."),
                          size, Look::Plain));
  }
  return lines;
}

bool move_cursor(std::size_t& cursor, Key key, std::size_t count) {
  std::ptrdiff_t delta = 0;
  if (key == keys::up || key.is(U'k')) {
    delta = -1;
  } else if (key == keys::down || key.is(U'j')) {
    delta = 1;
  } else if (key == keys::page_up) {
    delta = -10;
  } else if (key == keys::page_down) {
    delta = 10;
  } else {
    return false;
  }
  cursor = step(cursor, delta, count);
  return true;
}

std::string short_hints(std::string_view hints, std::size_t size) {
  if (width(hints) <= size) {
    return std::string(hints);
  }
  constexpr std::string_view ending = "… · ? guide · q quit";
  constexpr std::string_view separator = " · ";
  std::string kept;
  std::size_t start = 0;
  while (start <= hints.size()) {
    const std::size_t found = hints.find(separator, start);
    const std::string_view part = hints.substr(start, found == std::string_view::npos ? found : found - start);
    const std::string candidate = kept.empty() ? std::string(part) : text::concat(kept, separator, part);
    if (width(candidate) + width(ending) + 3 > size) {
      break;
    }
    kept = candidate;
    if (found == std::string_view::npos) {
      break;
    }
    start = found + separator.size();
  }
  return kept.empty() ? std::string(ending) : text::concat(kept, separator, ending);
}

}
