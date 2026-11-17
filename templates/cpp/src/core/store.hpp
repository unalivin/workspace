#pragma once

#include <chrono>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "core/embedded.hpp"
#include "core/error.hpp"

struct sqlite3;
struct sqlite3_stmt;

namespace app::store {

inline constexpr std::chrono::milliseconds default_busy_timeout{5000};

class SqliteError : public AppError {
 public:
  SqliteError(int code, std::string message);
  [[nodiscard]] int code() const noexcept;
  [[nodiscard]] bool is_constraint() const noexcept;

 private:
  int code_;
};

struct Migration {
  std::int64_t number = 0;
  std::string name;
  std::string sql;

  bool operator==(const Migration&) const = default;
};

struct Options {
  bool read_only = false;
  std::optional<std::chrono::milliseconds> busy_timeout;
};

class Statement {
 public:
  Statement(sqlite3* database, std::string_view sql);
  Statement(const Statement&) = delete;
  Statement& operator=(const Statement&) = delete;
  Statement(Statement&& other) noexcept;
  Statement& operator=(Statement&& other) noexcept;
  ~Statement();

  Statement& bind(std::int64_t value);
  Statement& bind(std::string_view value);
  Statement& bind_null();
  Statement& bind_optional(const std::optional<std::string>& value);
  [[nodiscard]] bool step();
  void run();
  void reset();
  [[nodiscard]] std::int64_t integer(int column) const;
  [[nodiscard]] std::string text(int column) const;
  [[nodiscard]] bool is_null(int column) const;
  [[nodiscard]] int parameters() const;

 private:
  void close() noexcept;
  sqlite3* database_ = nullptr;
  sqlite3_stmt* statement_ = nullptr;
  int next_ = 1;
};

class Store {
 public:
  [[nodiscard]] static Store open(const std::filesystem::path& path, Options options = {});
  [[nodiscard]] static Store open_ready(const std::filesystem::path& path);

  Store(const Store&) = delete;
  Store& operator=(const Store&) = delete;
  Store(Store&& other) noexcept;
  Store& operator=(Store&& other) noexcept;
  ~Store();

  [[nodiscard]] Statement prepare(std::string_view sql) const;
  void execute_batch(std::string_view sql) const;
  [[nodiscard]] std::int64_t last_insert_id() const;
  [[nodiscard]] std::int64_t changes() const;
  [[nodiscard]] const std::filesystem::path& path() const;
  [[nodiscard]] bool read_only() const;
  [[nodiscard]] std::int64_t schema_version() const;
  [[nodiscard]] std::int64_t data_version() const;
  [[nodiscard]] std::string journal_mode() const;
  [[nodiscard]] bool foreign_keys_enabled() const;
  [[nodiscard]] std::chrono::milliseconds busy_timeout() const;
  std::size_t migrate(const std::vector<Migration>& migrations) const;
  void integrity_check() const;
  void foreign_key_check() const;
  [[nodiscard]] std::string schema_snapshot() const;
  [[nodiscard]] bool has_full_text_search() const;
  [[nodiscard]] std::vector<std::string> query_plan(std::string_view sql, int parameters) const;

 private:
  Store(sqlite3* database, std::filesystem::path path, bool read_only);
  void close() noexcept;
  [[nodiscard]] std::int64_t pragma_integer(std::string_view sql) const;
  [[nodiscard]] bool apply_migration(const Migration& migration) const;

  sqlite3* database_ = nullptr;
  std::filesystem::path path_;
  bool read_only_ = false;
};

class Transaction {
 public:
  explicit Transaction(const Store& store);
  Transaction(const Transaction&) = delete;
  Transaction& operator=(const Transaction&) = delete;
  ~Transaction();
  void commit();
  void rollback();

 private:
  const Store* store_;
  bool open_ = true;
};

[[nodiscard]] std::vector<Migration> load_migrations(const std::vector<embedded::File>& files);
[[nodiscard]] std::vector<Migration> embedded_migrations();
[[nodiscard]] std::string_view sqlite_version();

}
