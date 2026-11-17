#pragma once

#include <exception>
#include <string>
#include <string_view>

namespace app {

enum class ErrorKind { Validation, NotFound, Conflict, Usage, Storage, Io };

class AppError : public std::exception {
 public:
  AppError(ErrorKind kind, std::string message);

  static AppError invalid(std::string_view field, std::string_view problem, std::string_view fix);
  static AppError not_found(std::string_view what);
  static AppError conflict(std::string message);
  static AppError usage(std::string message);
  static AppError storage(std::string message);
  static AppError io(std::string message);
  static AppError busy();

  [[nodiscard]] const char* what() const noexcept override;
  [[nodiscard]] ErrorKind kind() const noexcept;
  [[nodiscard]] const std::string& message() const noexcept;
  [[nodiscard]] const std::string& field() const noexcept;
  [[nodiscard]] const std::string& problem() const noexcept;
  [[nodiscard]] const std::string& fix() const noexcept;
  [[nodiscard]] bool is_busy() const noexcept;
  [[nodiscard]] bool is_usage() const noexcept;

 private:
  ErrorKind kind_;
  std::string message_;
  std::string field_;
  std::string problem_;
  std::string fix_;
};

}
