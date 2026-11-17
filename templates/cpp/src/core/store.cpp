#include "core/store.hpp"

#include <algorithm>
#include <fstream>
#include <utility>

#include <sqlite3.h>

#include "core/files.hpp"
#include "core/text.hpp"

namespace app::store {

namespace fs = std::filesystem;

namespace {

constexpr std::string_view file_header{"SQLite format 3\0", 16};
constexpr std::string_view sql_schema_snapshot =
    "SELECT type, name, coalesce(sql, '') FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND name NOT LIKE "
    "'records_fts_%' ORDER BY type, name";
constexpr std::string_view sql_compile_options = "PRAGMA compile_options";
constexpr std::string_view sql_foreign_key_check = "PRAGMA foreign_key_check";
constexpr std::string_view sql_integrity_check = "PRAGMA integrity_check";
constexpr std::string_view sql_user_version = "PRAGMA user_version";
constexpr std::string_view sql_data_version = "PRAGMA data_version";
constexpr std::string_view sql_journal_mode = "PRAGMA journal_mode";
constexpr std::string_view sql_set_wal = "PRAGMA journal_mode = WAL";
constexpr std::string_view sql_foreign_keys = "PRAGMA foreign_keys";
constexpr std::string_view sql_busy_timeout = "PRAGMA busy_timeout";
constexpr std::string_view sql_foreign_keys_on = "PRAGMA foreign_keys = ON";
constexpr std::string_view sql_query_only_on = "PRAGMA query_only = ON";
constexpr std::string_view sql_synchronous_normal = "PRAGMA synchronous = NORMAL";
constexpr std::string_view sql_begin_immediate = "BEGIN IMMEDIATE";
constexpr std::string_view sql_commit = "COMMIT";
constexpr std::string_view sql_rollback = "ROLLBACK";
constexpr std::string_view explain_prefix = "EXPLAIN QUERY PLAN ";

SqliteError failure(sqlite3* database, int code) {
  const int primary = code & 0xFF;
  if (primary == SQLITE_BUSY || primary == SQLITE_LOCKED) {
    return {code, std::string(AppError::busy().message())};
  }
  const char* detail = database != nullptr ? sqlite3_errmsg(database) : sqlite3_errstr(code);
  return {code, text::concat("database error: ", detail)};
}

std::optional<std::int64_t> migration_number(std::string_view name) {
  if (!name.ends_with(".sql")) {
    return std::nullopt;
  }
  const std::string_view stem = name.substr(0, name.size() - 4);
  const std::size_t split = stem.find('_');
  if (split == std::string_view::npos) {
    return std::nullopt;
  }
  const std::string_view number = stem.substr(0, split);
  const std::string_view title = stem.substr(split + 1);
  const bool valid_title = !title.empty() && std::all_of(title.begin(), title.end(), [](char letter) {
    return (letter >= 'a' && letter <= 'z') || (letter >= '0' && letter <= '9') || letter == '_';
  });
  if (number.size() != 4 || !text::is_digits(number) || !valid_title) {
    return std::nullopt;
  }
  return text::parse_integer(number);
}

AppError unusable(const fs::path& path, std::string_view reason) {
  return AppError::storage(
      text::concat("\"", files::shown(path),
                   "\" is not a usable SQLite database and was left untouched; choose another file (", reason, ")"));
}

void check_header(const fs::path& path) {
  if (!files::exists(path)) {
    return;
  }
  std::ifstream file(path, std::ios::binary);
  if (!file) {
    throw AppError::storage(text::concat("the database file \"", files::shown(path), "\" cannot be read"));
  }
  std::string header(file_header.size(), '\0');
  file.read(header.data(), static_cast<std::streamsize>(header.size()));
  const std::streamsize got = file.gcount();
  if (got == 0) {
    return;
  }
  if (static_cast<std::size_t>(got) != file_header.size() || header != file_header) {
    throw unusable(path, "it does not start with the SQLite file header");
  }
}

}

SqliteError::SqliteError(int code, std::string message)
    : AppError(ErrorKind::Storage, std::move(message)), code_(code) {}

int SqliteError::code() const noexcept {
  return code_;
}

bool SqliteError::is_constraint() const noexcept {
  return (code_ & 0xFF) == SQLITE_CONSTRAINT;
}

Statement::Statement(sqlite3* database, std::string_view sql) : database_(database) {
  const int code = sqlite3_prepare_v2(database_, sql.data(), static_cast<int>(sql.size()), &statement_, nullptr);
  if (code != SQLITE_OK) {
    throw failure(database_, code);
  }
}

Statement::Statement(Statement&& other) noexcept
    : database_(other.database_), statement_(std::exchange(other.statement_, nullptr)), next_(other.next_) {}

Statement& Statement::operator=(Statement&& other) noexcept {
  if (this != &other) {
    close();
    database_ = other.database_;
    statement_ = std::exchange(other.statement_, nullptr);
    next_ = other.next_;
  }
  return *this;
}

Statement::~Statement() {
  close();
}

void Statement::close() noexcept {
  if (statement_ != nullptr) {
    sqlite3_finalize(statement_);
    statement_ = nullptr;
  }
}

Statement& Statement::bind(std::int64_t value) {
  const int code = sqlite3_bind_int64(statement_, next_++, value);
  if (code != SQLITE_OK) {
    throw failure(database_, code);
  }
  return *this;
}

Statement& Statement::bind(std::string_view value) {
  const char* bytes = value.data() == nullptr ? "" : value.data();
  const int code = sqlite3_bind_text64(statement_, next_++, bytes, value.size(), SQLITE_TRANSIENT, SQLITE_UTF8);
  if (code != SQLITE_OK) {
    throw failure(database_, code);
  }
  return *this;
}

Statement& Statement::bind_null() {
  const int code = sqlite3_bind_null(statement_, next_++);
  if (code != SQLITE_OK) {
    throw failure(database_, code);
  }
  return *this;
}

Statement& Statement::bind_optional(const std::optional<std::string>& value) {
  return value ? bind(std::string_view(*value)) : bind_null();
}

bool Statement::step() {
  const int code = sqlite3_step(statement_);
  if (code == SQLITE_ROW) {
    return true;
  }
  if (code == SQLITE_DONE) {
    return false;
  }
  throw failure(database_, code);
}

void Statement::run() {
  while (step()) {}
}

void Statement::reset() {
  sqlite3_reset(statement_);
  sqlite3_clear_bindings(statement_);
  next_ = 1;
}

std::int64_t Statement::integer(int column) const {
  return sqlite3_column_int64(statement_, column);
}

std::string Statement::text(int column) const {
  const unsigned char* data = sqlite3_column_text(statement_, column);
  const int size = sqlite3_column_bytes(statement_, column);
  if (data == nullptr || size <= 0) {
    return {};
  }
  return {reinterpret_cast<const char*>(data), static_cast<std::size_t>(size)};
}

bool Statement::is_null(int column) const {
  return sqlite3_column_type(statement_, column) == SQLITE_NULL;
}

int Statement::parameters() const {
  return sqlite3_bind_parameter_count(statement_);
}

Store::Store(sqlite3* database, fs::path path, bool read_only)
    : database_(database), path_(std::move(path)), read_only_(read_only) {}

Store::Store(Store&& other) noexcept
    : database_(std::exchange(other.database_, nullptr)), path_(std::move(other.path_)), read_only_(other.read_only_) {}

Store& Store::operator=(Store&& other) noexcept {
  if (this != &other) {
    close();
    database_ = std::exchange(other.database_, nullptr);
    path_ = std::move(other.path_);
    read_only_ = other.read_only_;
  }
  return *this;
}

Store::~Store() {
  close();
}

void Store::close() noexcept {
  if (database_ != nullptr) {
    sqlite3_close_v2(database_);
    database_ = nullptr;
  }
}

Store Store::open(const fs::path& path, Options options) {
  if (!path.is_absolute()) {
    throw AppError::usage(text::concat("database path \"", files::shown(path), "\" must be absolute"));
  }
  if (files::is_directory(path)) {
    throw AppError::usage(
        text::concat("database path \"", files::shown(path), "\" is a directory: choose a file path"));
  }
  check_header(path);
  int flags = SQLITE_OPEN_NOMUTEX;
  if (options.read_only) {
    if (!files::exists(path)) {
      throw AppError::storage(
          text::concat("database \"", files::shown(path), "\" cannot be opened read-only: the file does not exist"));
    }
    flags |= SQLITE_OPEN_READONLY;
  } else {
    if (path.has_parent_path()) {
      files::create_private_dir(path.parent_path());
    }
    if (!files::exists(path)) {
      files::write_private(path, "", false);
    }
    flags |= SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE;
  }
  sqlite3* database = nullptr;
  const int code = sqlite3_open_v2(files::shown(path).c_str(), &database, flags, nullptr);
  if (code != SQLITE_OK) {
    const std::string reason = database != nullptr ? sqlite3_errmsg(database) : sqlite3_errstr(code);
    sqlite3_close_v2(database);
    throw unusable(path, reason);
  }
  Store store(database, path, options.read_only);
  sqlite3_busy_timeout(database, static_cast<int>(options.busy_timeout.value_or(default_busy_timeout).count()));
  try {
    store.execute_batch(sql_foreign_keys_on);
    if (options.read_only) {
      store.execute_batch(sql_query_only_on);
    } else {
      Statement journal = store.prepare(sql_set_wal);
      const std::string mode = journal.step() ? journal.text(0) : std::string();
      if (!text::equals_ignoring_case(mode, "wal")) {
        throw AppError::storage(text::concat(
            "\"", files::shown(path), "\" could not be switched to write-ahead logging (journal mode ", mode, ")"));
      }
    }
    static_cast<void>(store.schema_version());
  } catch (const SqliteError& error) {
    if (error.is_busy()) {
      throw;
    }
    throw unusable(path, error.message());
  }
  if (!options.read_only) {
    store.execute_batch(sql_synchronous_normal);
  }
  return store;
}

Store Store::open_ready(const fs::path& path) {
  Store store = open(path);
  store.migrate(embedded_migrations());
  return store;
}

Statement Store::prepare(std::string_view sql) const {
  return {database_, sql};
}

void Store::execute_batch(std::string_view sql) const {
  const std::string owned(sql);
  const int code = sqlite3_exec(database_, owned.c_str(), nullptr, nullptr, nullptr);
  if (code != SQLITE_OK) {
    throw failure(database_, code);
  }
}

std::int64_t Store::last_insert_id() const {
  return sqlite3_last_insert_rowid(database_);
}

std::int64_t Store::changes() const {
  return sqlite3_changes64(database_);
}

const fs::path& Store::path() const {
  return path_;
}

bool Store::read_only() const {
  return read_only_;
}

std::int64_t Store::pragma_integer(std::string_view sql) const {
  Statement statement = prepare(sql);
  return statement.step() ? statement.integer(0) : 0;
}

std::int64_t Store::schema_version() const {
  return pragma_integer(sql_user_version);
}

std::int64_t Store::data_version() const {
  return pragma_integer(sql_data_version);
}

std::string Store::journal_mode() const {
  Statement statement = prepare(sql_journal_mode);
  return statement.step() ? statement.text(0) : std::string();
}

bool Store::foreign_keys_enabled() const {
  return pragma_integer(sql_foreign_keys) == 1;
}

std::chrono::milliseconds Store::busy_timeout() const {
  return std::chrono::milliseconds(std::max<std::int64_t>(pragma_integer(sql_busy_timeout), 0));
}

std::size_t Store::migrate(const std::vector<Migration>& migrations) const {
  if (read_only_) {
    throw AppError::storage("a read-only database cannot be migrated");
  }
  const std::int64_t current = schema_version();
  if (current > static_cast<std::int64_t>(migrations.size())) {
    throw AppError::storage(text::concat("the database has schema version ", current, " but this build knows only ",
                                         migrations.size(), ": use a newer build"));
  }
  std::size_t applied = 0;
  for (const Migration& migration : migrations) {
    if (migration.number <= current) {
      continue;
    }
    try {
      if (apply_migration(migration)) {
        ++applied;
      }
    } catch (const AppError& error) {
      if (error.is_busy()) {
        throw;
      }
      throw AppError::storage(
          text::concat("migration ", migration.name, " failed and was rolled back: ", error.message()));
    }
  }
  return applied;
}

bool Store::apply_migration(const Migration& migration) const {
  Transaction transaction(*this);
  const std::int64_t current = schema_version();
  if (current >= migration.number) {
    return false;
  }
  if (current != migration.number - 1) {
    throw AppError::storage(text::concat("expected schema version ", migration.number - 1, ", found ", current));
  }
  execute_batch(migration.sql);
  transaction.commit();
  return true;
}

void Store::integrity_check() const {
  Statement statement = prepare(sql_integrity_check);
  const std::string result = statement.step() ? statement.text(0) : std::string("no answer");
  if (result != "ok") {
    throw AppError::storage(text::concat("integrity check reported: ", result));
  }
}

void Store::foreign_key_check() const {
  Statement statement = prepare(sql_foreign_key_check);
  if (statement.step()) {
    throw AppError::storage("foreign key check reported at least one violation");
  }
}

std::string Store::schema_snapshot() const {
  Statement statement = prepare(sql_schema_snapshot);
  std::string snapshot;
  while (statement.step()) {
    snapshot.append(
        text::concat("[", statement.text(0), " ", statement.text(1), "]\n", text::trim(statement.text(2)), "\n\n"));
  }
  return snapshot;
}

bool Store::has_full_text_search() const {
  Statement statement = prepare(sql_compile_options);
  while (statement.step()) {
    if (statement.text(0) == "ENABLE_FTS5") {
      return true;
    }
  }
  return false;
}

std::vector<std::string> Store::query_plan(std::string_view sql, int parameters) const {
  Statement statement = prepare(text::concat(explain_prefix, sql));
  for (int index = 0; index < parameters; ++index) {
    statement.bind(std::int64_t{1});
  }
  std::vector<std::string> plan;
  while (statement.step()) {
    plan.push_back(statement.text(3));
  }
  return plan;
}

Transaction::Transaction(const Store& store) : store_(&store) {
  store_->execute_batch(sql_begin_immediate);
}

Transaction::~Transaction() {
  if (open_) {
    try {
      store_->execute_batch(sql_rollback);
    } catch (const AppError&) {
      open_ = false;
    }
  }
}

void Transaction::commit() {
  store_->execute_batch(sql_commit);
  open_ = false;
}

void Transaction::rollback() {
  open_ = false;
  store_->execute_batch(sql_rollback);
}

std::vector<Migration> load_migrations(const std::vector<embedded::File>& files) {
  std::vector<Migration> migrations;
  for (const embedded::File& file : files) {
    const std::optional<std::int64_t> number = migration_number(file.name);
    if (!number) {
      throw AppError::storage(text::concat("migration \"", file.name, "\" must be named NNNN_name.sql"));
    }
    const std::string_view content = text::trim(file.content);
    const std::string marker = text::concat("user_version = ", *number, ";");
    if (!content.ends_with(text::concat("PRAGMA ", marker))) {
      throw AppError::storage(text::concat("migration ", file.name, " must end with \"", marker, "\""));
    }
    migrations.push_back({*number, std::string(file.name), std::string(content)});
  }
  std::sort(migrations.begin(), migrations.end(),
            [](const Migration& left, const Migration& right) { return left.number < right.number; });
  for (std::size_t index = 0; index < migrations.size(); ++index) {
    if (migrations[index].number != static_cast<std::int64_t>(index) + 1) {
      throw AppError::storage(text::concat("migrations must be numbered without gaps from 0001: found ",
                                           migrations[index].name, " at position ", index + 1));
    }
  }
  if (migrations.empty()) {
    throw AppError::storage("no migrations found");
  }
  return migrations;
}

std::vector<Migration> embedded_migrations() {
  return load_migrations(embedded::migrations());
}

std::string_view sqlite_version() {
  return sqlite3_libversion();
}

}
