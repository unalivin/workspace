#include <doctest/doctest.h>

#include <string>
#include <vector>

#include "tui/text.hpp"
#include "tui/widgets.hpp"

using namespace app::tui;

namespace {

using Words = std::vector<std::string>;

void typed(Input& input, std::u32string_view letters) {
  for (const char32_t letter : letters) {
    input.handle(Key::letter(letter));
  }
}

void typed(Form& form, std::u32string_view letters) {
  for (const char32_t letter : letters) {
    form.handle(Key::letter(letter));
  }
}

}

TEST_CASE("text is clipped padded and wrapped by display width") {
  CHECK(clip("hello", 5) == "hello");
  CHECK(clip("hello world", 6) == "hello…");
  CHECK(clip("日本語のテキスト", 7) == "日本語…");
  CHECK(clip("a\tb\nc", 9) == "a b c");
  CHECK(clip("abc", 0).empty());
  CHECK(pad("ab", 4) == "ab  ");
  CHECK(width(pad("日本語のテキスト", 9)) == 9);
  CHECK(width("e\u0301") == 1);
  CHECK(letter_width(U'語') == 2);
  CHECK(wrap("one two three four", 9) == Words{"one two", "three", "four"});
  CHECK(wrap("abcdefghijklmnop", 8) == Words{"abcdefgh", "ijklmnop"});
  CHECK(wrap("first\n\nsecond", 20) == Words{"first", "", "second"});
  for (const std::string& line : wrap(std::string(500, 'x'), 10)) {
    CHECK(width(line) <= 10);
  }
  CHECK(repeat("─", 3) == "───");
}

TEST_CASE("the scroll window always contains the cursor") {
  using Range = std::pair<std::size_t, std::size_t>;
  CHECK(window(0, 0, 5) == Range{0, 0});
  CHECK(window(3, 2, 5) == Range{0, 3});
  CHECK(window(20, 0, 5) == Range{0, 5});
  CHECK(window(20, 19, 5) == Range{15, 20});
  for (std::size_t cursor = 0; cursor < 20; ++cursor) {
    const auto [start, end] = window(20, cursor, 5);
    CHECK_MESSAGE((start <= cursor && cursor < end && end - start == 5), cursor, ": ", start, "..", end);
  }
}

TEST_CASE("lines fit their column exactly") {
  const Line line = Line::plain("name").with(" · ", Look::Muted).with("a long value", Look::Accent);
  CHECK(line.width() == 19);
  CHECK(line.fit(10).text() == "name · a …");
  CHECK(line.fit(25).text() == "name · a long value      ");
  CHECK(Line::styled("row", Look::Selected).fit(6).spans.back().look == Look::Selected);
  CHECK(Line(line).recolor(Look::Selected).spans.at(1).look == Look::Selected);
  CHECK(look_for(app::theme::Severity::Critical) == Look::Critical);
  CHECK(look_for(app::theme::Severity::Info) == Look::Info);
}

TEST_CASE("the text input edits at the cursor and respects its limit") {
  Input input("abc", 6);
  typed(input, U"de");
  CHECK(input.value() == "abcde");
  input.handle(keys::home);
  typed(input, U"Xyz");
  CHECK(input.value() == "Xabcde");
  input.handle(keys::del);
  CHECK(input.value() == "Xbcde");
  input.handle(keys::end);
  input.handle(keys::backspace);
  input.handle(keys::left);
  input.handle(keys::left);
  input.handle(Key::ctrl(U'u'));
  CHECK(input.value() == "cd");
  input.handle(keys::right);
  input.handle(Key::letter(U'\u0007'));
  CHECK(input.value() == "cd");
  CHECK_FALSE(input.handle(Key::function(5)));
  CHECK(input.view(6, true) == "c▏d   ");
  CHECK(input.view(6, false) == "cd    ");
  const Input longer("abcdefghijklmnop", 0);
  CHECK(longer.view(8, true) == "jklmnop▏");
}

TEST_CASE("a form moves between fields changes choices and submits on the last field") {
  Form form("Demo", {
                        Field::text("name", "Name", "", "Required.", 20),
                        Field::choice("level", "Level", "b", "", {{"a", "First"}, {"b", "Second"}, {"c", "Third"}}),
                        Field::yes_no("enabled", "Enabled", true, ""),
                    });
  typed(form, U"Ada");
  CHECK(form.handle(keys::enter) == FormEvent::Open);
  CHECK(form.focus == 1);
  form.handle(keys::right);
  CHECK(form.value("level") == "c");
  form.handle(keys::right);
  CHECK(form.value("level") == "a");
  form.handle(keys::left);
  CHECK(form.value("level") == "c");
  form.handle(keys::tab);
  form.handle(Key::letter(U' '));
  CHECK(form.value("enabled") == "false");
  CHECK(form.handle(keys::enter) == FormEvent::Submit);
  form.handle(keys::back_tab);
  CHECK(form.focus == 1);
  CHECK(form.handle(Key::ctrl(U's')) == FormEvent::Submit);
  CHECK(form.handle(keys::esc) == FormEvent::Cancel);
  CHECK(form.value("name") == "Ada");
  CHECK(form.value("missing").empty());
  form.problem = "name: the name is empty";
  Words shown;
  bool third = false;
  bool refused = false;
  for (const Line& line : form.view(100)) {
    shown.push_back(line.text());
    third = third || shown.back().find("‹ Third ›") != std::string::npos;
    refused = refused || shown.back().starts_with("✖ Not saved: name");
  }
  CHECK(third);
  CHECK(refused);
  CHECK(shown.back() == form_hint);
  Form empty("Nothing", {});
  CHECK(empty.handle(keys::enter) == FormEvent::Cancel);
}

TEST_CASE("columns share spare room and rows keep their widths") {
  using Sizes = std::vector<std::size_t>;
  CHECK(layout({{5, false}, {16, true}, {8, false}}, 55) == Sizes{5, 36, 8});
  CHECK(layout({{5, false}, {16, true}}, 10) == Sizes{5, 8});
  CHECK(layout({{5, false}, {7, false}}, 40) == Sizes{5, 7});
  CHECK(row({"7", "A long name"}, {3, 6}) == "7    A lon…  ");
  CHECK(tail("/home/someone/data/app.db", 10) == "…ta/app.db");
  CHECK(tail("short", 10) == "short");
  CHECK(clip_start("https://example.org/a/b", 5).empty());
  CHECK(clip_start("https://example.org/a/b", 9) == "https://…");
  CHECK(selectable("row", true, 8).text() == "▌ row   ");
  CHECK(selectable("row", false, 8).text() == "  row");
  CHECK(step(0, -1, 4) == 0);
  CHECK(step(3, 1, 4) == 3);
  CHECK(step(1, 10, 4) == 3);
  CHECK(step(2, 1, 0) == 0);
}
