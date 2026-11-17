#include "gui/ledger.hpp"

#include <algorithm>
#include <array>
#include <cmath>

#include "core/text.hpp"

namespace app::gui::ledger {

namespace {

constexpr std::array<std::int64_t, 13> units = {1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000};
constexpr float row_gap = 4.0F;
constexpr float label_ems = 8.0F;
constexpr std::string_view widest_figure = "0000";
constexpr double done_fade = 0.6;
constexpr float square_radius = 3.0F;

}

std::int64_t squares(std::int64_t total, std::int64_t unit) {
  total = std::max<std::int64_t>(total, 0);
  unit = std::max<std::int64_t>(unit, 1);
  return total / unit + (total % unit != 0 ? 1 : 0);
}

std::int64_t unit(std::int64_t largest, std::int64_t capacity) {
  capacity = std::max<std::int64_t>(capacity, 1);
  for (const std::int64_t candidate : units) {
    if (squares(largest, candidate) <= capacity) {
      return candidate;
    }
  }
  return squares(largest, capacity);
}

std::string caption(std::int64_t unit) {
  if (unit <= 1) {
    return "One square is one enabled record.";
  }
  return text::concat("One square is ", unit, " enabled records, rounded up.");
}

std::int64_t count(const std::vector<records::Breakdown>& rows, records::Status status, theme::Severity level) {
  std::int64_t total = 0;
  for (const records::Breakdown& row : rows) {
    if (row.status == status && row.severity == level) {
      total += row.total;
    }
  }
  return total;
}

std::int64_t largest(const std::vector<records::Breakdown>& rows) {
  std::int64_t found = 0;
  for (const records::Status status : records::all_statuses) {
    for (const theme::Severity level : theme::all_severities) {
      found = std::max(found, count(rows, status, level));
    }
  }
  return found;
}

float height(const theme::Tokens& tokens) {
  const auto lines = static_cast<float>(theme::all_severities.size() + 1);
  return lines * tokens.spacing.row_height + (lines - 1.0F) * row_gap;
}

std::int64_t show(const Ui& ui, const std::vector<records::Breakdown>& rows) {
  const theme::Tokens& tokens = ui.tokens();
  const theme::Spacing& gaps = tokens.spacing;
  const theme::TypeScale& sizes = tokens.type_scale;
  const float width = ImGui::GetContentRegionAvail().x;
  const ImVec2 corner = ImGui::GetCursorScreenPos();
  ImGui::Dummy(ImVec2(width, std::round(height(tokens))));
  ImDrawList* canvas = ImGui::GetWindowDrawList();
  canvas->PushClipRect(corner, ImVec2(corner.x + width, corner.y + height(tokens)), true);
  const ImU32 muted = color(tokens.palette.text_muted);
  const ImU32 ink = color(tokens.palette.text);
  ImFont* figures = ui.faces().mono;
  const float label = label_ems * sizes.body;
  const float column = std::max((width - label) / static_cast<float>(records::all_statuses.size()), 0.0F);
  const float step = gaps.row_height + row_gap;
  const float square = gaps.m;
  const float pitch = square + gaps.xs;
  const float widest = ui.measure(widest_figure, figures, sizes.caption).x;
  const auto capacity = static_cast<std::int64_t>(std::floor((column - gaps.l - widest - gaps.s) / pitch));
  const std::int64_t chosen = unit(largest(rows), capacity);
  const float side = ui.icon_side();
  const auto figure = [&](std::int64_t total, float left, float middle) {
    ui.paint_text(canvas, std::to_string(total), ImVec2(left, middle - sizes.caption / 2.0F), muted, figures,
                  sizes.caption);
  };

  float middle = corner.y + gaps.row_height / 2.0F;
  for (std::size_t index = 0; index < records::all_statuses.size(); ++index) {
    const records::Status status = records::all_statuses[index];
    float left = corner.x + label + static_cast<float>(index) * column;
    ui.paint_icon(canvas, status_icon(status), ImVec2(left, middle - side / 2.0F), side, muted);
    left += side + gaps.s;
    const std::string_view name = records::label(status);
    ui.paint_text(canvas, name, ImVec2(left, middle - sizes.body / 2.0F), ink, ui.faces().medium, sizes.body);
    left += ui.measure(name, ui.faces().medium, sizes.body).x + gaps.s;
    std::int64_t total = 0;
    for (const theme::Severity level : theme::all_severities) {
      total += count(rows, status, level);
    }
    figure(total, left, middle);
  }

  std::size_t line = 0;
  for (auto level = theme::all_severities.rbegin(); level != theme::all_severities.rend(); ++level, ++line) {
    const float top = corner.y + static_cast<float>(line + 1) * step;
    middle = top + gaps.row_height / 2.0F;
    const float rule = std::round(top - row_gap / 2.0F) + 0.5F;
    canvas->AddLine(ImVec2(corner.x, rule), ImVec2(corner.x + width, rule), color(tokens.palette.divider), 1.0F);
    const theme::Color tint = tokens.semantic.severity(*level);
    ui.paint_icon(canvas, severity_icon(*level), ImVec2(corner.x, middle - side / 2.0F), side, color(tint));
    ui.paint_text(canvas, theme::label(*level), ImVec2(corner.x + side + gaps.s, middle - sizes.body / 2.0F), ink);
    for (std::size_t index = 0; index < records::all_statuses.size(); ++index) {
      const records::Status status = records::all_statuses[index];
      const std::int64_t total = count(rows, status, *level);
      if (total == 0) {
        continue;
      }
      const theme::Color fill =
          status == records::Status::Done ? theme::mix(tint, tokens.palette.surface, done_fade) : tint;
      const float start = corner.x + label + static_cast<float>(index) * column;
      const std::int64_t drawn = squares(total, chosen);
      for (std::int64_t position = 0; position < drawn; ++position) {
        const ImVec2 from(std::round(start + static_cast<float>(position) * pitch), std::round(middle - square / 2.0F));
        canvas->AddRectFilled(from, ImVec2(from.x + square, from.y + square), color(fill),
                              std::min(tokens.radius, ui.u(square_radius)));
      }
      figure(total, start + static_cast<float>(drawn) * pitch + gaps.xs, middle);
    }
  }
  canvas->PopClipRect();
  return chosen;
}

}
