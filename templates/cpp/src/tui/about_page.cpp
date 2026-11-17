#include <algorithm>
#include <array>
#include <utility>

#include "core/meta.hpp"
#include "core/text.hpp"
#include "tui/model.hpp"

namespace app::tui {

namespace {

struct Tab {
  AboutTab tab = AboutTab::Overview;
  std::string_view title;
};

constexpr std::array<Tab, 5> tabs = {{
    {AboutTab::Overview, "Overview"},
    {AboutTab::Guide, "Usage guide"},
    {AboutTab::Shortcuts, "Shortcuts"},
    {AboutTab::Diagnostics, "Diagnostics"},
    {AboutTab::Licenses, "Licenses"},
}};

struct Extra {
  std::string_view keys;
  std::string_view action;
};

constexpr std::array<Extra, 13> extra_shortcuts = {{
    {"Tab, Shift+Tab", "Next or previous category"},
    {": or F10", "Open the menu"},
    {"Left, Right", "Switch sections inside a category"},
    {"e", "Records: edit the selected record"},
    {"x", "Records: enable or disable the selected record"},
    {"f, v", "Records: next status filter, next severity filter"},
    {"s, S", "Records: next sort column, reverse the order"},
    {"c", "Records: choose the table columns"},
    {"a", "Records: show or hide disabled records"},
    {"i, o", "Records: import, export"},
    {"l", "Overview: load demo data"},
    {"m, M", "Notifications: mark one as read, mark all as read"},
    {"u", "Notifications: unread only. Rules and Settings: reset"},
}};

constexpr std::size_t search_limit = 100;

std::size_t less(std::size_t value, std::size_t amount) {
  return value > amount ? value - amount : 0;
}

std::vector<std::size_t> every_section(const guide::Guide& usage) {
  std::vector<std::size_t> all(usage.sections.size());
  for (std::size_t index = 0; index < all.size(); ++index) {
    all[index] = index;
  }
  return all;
}

}

AboutPage::AboutPage() : usage(service::usage_guide()), search("", search_limit) {
  hits = every_section(usage);
}

void AboutPage::filter() {
  const std::string query(text::trim_whitespace(search.value()));
  if (query.empty()) {
    hits = every_section(usage);
  } else {
    hits.clear();
    for (const guide::Hit& hit : usage.search(query)) {
      if (std::ranges::find(hits, hit.section) == hits.end()) {
        hits.push_back(hit.section);
      }
    }
  }
  cursor = 0;
}

std::vector<LinkRow> AboutPage::links() const {
  const std::string issue = diagnostics ? diagnostics->issue_report().url : std::string();
  return {
      {"Source Code", service::source_link(), false},
      {"Found a bug? Please report it on GitHub.", service::external_link("Report a bug", issue), true},
      {"Report a vulnerability privately", service::advisory_link(), false},
  };
}

void AboutPage::turn(std::size_t delta) {
  const auto found = std::ranges::find(tabs, tab, &Tab::tab);
  const std::size_t at = found == tabs.end() ? 0 : static_cast<std::size_t>(found - tabs.begin());
  tab = tabs.at((at + delta) % tabs.size()).tab;
  cursor = 0;
  scroll = 0;
}

void AboutPage::scroll_key(Key key) {
  if (key == keys::up || key.is(U'k')) {
    scroll = less(scroll, 1);
  } else if (key == keys::down || key.is(U'j')) {
    scroll += 1;
  } else if (key == keys::page_up) {
    scroll = less(scroll, 10);
  } else if (key == keys::page_down || key.is(U' ')) {
    scroll += 10;
  }
}

void Model::load_about() {
  attempt([this] { about_.diagnostics = app_->diagnostics(options_.has_dashboard); });
}

void Model::about_key(Key key) {
  AboutPage& page = about_;
  if (page.searching) {
    if (key == keys::enter) {
      page.searching = false;
    } else if (key == keys::esc) {
      page.searching = false;
      page.search = Input("", search_limit);
      page.filter();
    } else if (page.search.handle(key)) {
      page.filter();
    }
    return;
  }
  const bool forward = key == keys::right || key.is(U']');
  const bool backward = key == keys::left || key.is(U'[');
  if (forward || backward) {
    if (!page.reading) {
      page.turn(forward ? 1 : tabs.size() - 1);
    }
    return;
  }
  if (page.tab == AboutTab::Overview) {
    const std::vector<LinkRow> links = page.links();
    if (move_cursor(page.cursor, key, links.size()) || key != keys::enter) {
      return;
    }
    const LinkRow& chosen = links.at(std::min(page.cursor, links.size() - 1));
    if (!chosen.issue) {
      std::string done;
      if (attempt([&] { done = open_link(chosen.link); })) {
        say(Notice::Success, std::move(done));
      }
      return;
    }
    if (!page.diagnostics) {
      return;
    }
    const service::IssueReport report = page.diagnostics->issue_report();
    std::vector<std::string> lines = {
        "The report form opens in your browser, prefilled with exactly this text and nothing else. You can edit it "
        "before you submit it.",
        "",
    };
    for (std::string& line : text::split(report.body, '\n')) {
      lines.push_back(std::move(line));
    }
    lines.emplace_back();
    lines.push_back(text::concat("Link: ", report.url));
    preview_ =
        Preview{"Report a bug", std::move(lines), 0, "open the report form in the browser", OpenLink{chosen.link}};
    return;
  }
  if (page.tab != AboutTab::Guide) {
    page.scroll_key(key);
    return;
  }
  if (page.reading) {
    if (key == keys::esc || key == keys::backspace || key == keys::enter) {
      page.reading = false;
      page.scroll = 0;
    } else {
      page.scroll_key(key);
    }
    return;
  }
  if (move_cursor(page.cursor, key, page.hits.size())) {
    return;
  }
  if (key.is(U'/')) {
    page.searching = true;
  } else if (key == keys::esc) {
    page.search = Input("", search_limit);
    page.filter();
  } else if (key == keys::enter && page.cursor < page.hits.size()) {
    page.section = page.hits[page.cursor];
    page.reading = true;
    page.scroll = 0;
  }
}

Lines Model::about_view(std::size_t size, std::size_t height) {
  const AboutPage& page = about_;
  Line header;
  for (std::size_t index = 0; index < tabs.size(); ++index) {
    if (index > 0) {
      header.add(" ", Look::Plain);
    }
    if (tabs.at(index).tab == page.tab) {
      header.add(text::concat("▌", tabs.at(index).title, " "), Look::Selected);
    } else {
      header.add(text::concat(" ", tabs.at(index).title, " "), Look::Plain);
    }
  }
  Lines lines = {std::move(header), Line{}};
  const std::size_t room = std::max<std::size_t>(less(height, 3), 3);
  Lines body;
  switch (page.tab) {
    case AboutTab::Overview: {
      lines.push_back(Line::styled(text::concat(meta::app_name, " ", meta::version()), Look::Heading));
      lines.emplace_back();
      extend(lines, wrapped(meta::tagline, size, Look::Plain));
      lines.emplace_back();
      lines.push_back(Line::plain(text::concat("Publisher: ", meta::publisher)));
      lines.push_back(Line::plain(text::concat("License: ", meta::license)));
      lines.emplace_back();
      const std::vector<LinkRow> links = page.links();
      for (std::size_t index = 0; index < links.size(); ++index) {
        Line line =
            index == page.cursor
                ? Line::styled(pad(text::concat("▌ ", links[index].label), 42), Look::Selected).with(" ", Look::Plain)
                : Line::plain(pad(text::concat("  ", links[index].label), 43));
        lines.push_back(std::move(line).with(clip_start(links[index].link.url, less(size, 46)), Look::Muted));
      }
      lines.emplace_back();
      extend(lines, wrapped("Enter opens the selected link in your browser. Nothing is sent by the application.", size,
                            Look::Muted));
      return lines;
    }
    case AboutTab::Guide: {
      if (page.reading) {
        if (page.section >= page.usage.sections.size()) {
          return lines;
        }
        const guide::Section& section = page.usage.sections[page.section];
        body.push_back(Line::styled(section.title, Look::Heading));
        body.emplace_back();
        extend(body, wrapped(guide::plain(section.body), size, Look::Plain));
        body.emplace_back();
        body.push_back(Line::styled("Esc back to the contents", Look::Muted));
        break;
      }
      std::string search = page.search.value().empty() ? std::string("press / to search") : page.search.value();
      if (page.searching) {
        search = page.search.view(30, true);
        while (!search.empty() && search.back() == ' ') {
          search.pop_back();
        }
      }
      lines.push_back(Line::styled(text::concat("Search: ", search), Look::Muted));
      lines.emplace_back();
      if (page.hits.empty()) {
        extend(lines, wrapped("No section mentions these words. Press Esc to show the contents.", size, Look::Plain));
        return lines;
      }
      const auto [start, end] = window(page.hits.size(), page.cursor, std::max<std::size_t>(less(room, 2), 1));
      for (std::size_t index = start; index < end; ++index) {
        const guide::Section& section = page.usage.sections.at(page.hits[index]);
        lines.push_back(
            selectable(text::concat(repeat("  ", less(section.level, 1)), section.title), index == page.cursor, size));
      }
      return lines;
    }
    case AboutTab::Shortcuts: {
      const std::vector<std::size_t> widths = {22, less(size, 24)};
      body.push_back(Line::styled(row({"KEY", "ACTION"}, widths), Look::Muted));
      for (const service::Shortcut& shortcut : service::shortcuts) {
        body.push_back(Line::plain(row({std::string(shortcut.terminal), std::string(shortcut.action)}, widths)));
      }
      for (const Extra& extra : extra_shortcuts) {
        body.push_back(Line::plain(row({std::string(extra.keys), std::string(extra.action)}, widths)));
      }
      break;
    }
    case AboutTab::Diagnostics: {
      if (page.diagnostics) {
        for (const auto& [label, value] : page.diagnostics->lines()) {
          body.push_back(Line::plain(text::concat(pad(label, 30), " ", value)));
        }
      }
      body.emplace_back();
      extend(body,
             wrapped("Diagnostics stay on this computer. A bug report includes only the lines shown in its preview.",
                     size, Look::Muted));
      break;
    }
    case AboutTab::Licenses:
      for (std::string& line : wrap(text::concat(meta::license_text(), "\n\n", meta::notices_text()), size)) {
        body.push_back(Line::plain(std::move(line)));
      }
      break;
  }
  const std::size_t scroll = std::min(page.scroll, less(body.size(), room));
  for (std::size_t index = scroll; index < body.size() && index < scroll + room; ++index) {
    lines.push_back(std::move(body[index]));
  }
  about_.scroll = scroll;
  return lines;
}

}
