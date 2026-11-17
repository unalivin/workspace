#include "core/error.hpp"

#include <utility>

namespace app {

namespace {
constexpr std::string_view busy_message =
    "The database is busy: another program is writing to it. Wait a moment and try again.";
}

AppError::AppError(ErrorKind kind, std::string message) : kind_(kind), message_(std::move(message)) {}

AppError AppError::invalid(std::string_view field, std::string_view problem, std::string_view fix) {
  std::string message;
  message.append(field).append(": ").append(problem).append(". ").append(fix);
  AppError error(ErrorKind::Validation, std::move(message));
  error.field_ = field;
  error.problem_ = problem;
  error.fix_ = fix;
  return error;
}

AppError AppError::not_found(std::string_view what) {
  return {ErrorKind::NotFound, std::string(what) + " was not found"};
}

AppError AppError::conflict(std::string message) {
  return {ErrorKind::Conflict, std::move(message)};
}

AppError AppError::usage(std::string message) {
  return {ErrorKind::Usage, std::move(message)};
}

AppError AppError::storage(std::string message) {
  return {ErrorKind::Storage, std::move(message)};
}

AppError AppError::io(std::string message) {
  return {ErrorKind::Io, std::move(message)};
}

AppError AppError::busy() {
  return {ErrorKind::Storage, std::string(busy_message)};
}

const char* AppError::what() const noexcept {
  return message_.c_str();
}

ErrorKind AppError::kind() const noexcept {
  return kind_;
}

const std::string& AppError::message() const noexcept {
  return message_;
}

const std::string& AppError::field() const noexcept {
  return field_;
}

const std::string& AppError::problem() const noexcept {
  return problem_;
}

const std::string& AppError::fix() const noexcept {
  return fix_;
}

bool AppError::is_busy() const noexcept {
  return kind_ == ErrorKind::Storage && message_ == busy_message;
}

bool AppError::is_usage() const noexcept {
  return kind_ == ErrorKind::Validation || kind_ == ErrorKind::Usage;
}

}
