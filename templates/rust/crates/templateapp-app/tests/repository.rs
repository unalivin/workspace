use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use app_core::settings;

const REQUIRED_FILES: &[&str] = &[
    "README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "SECURITY.md",
    "LICENSE",
    "Makefile",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "deny.toml",
    ".gitignore",
    ".gitattributes",
    ".env.example",
    "THIRD_PARTY_NOTICES.md",
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
    "docs/adr/README.md",
];
const REQUIRED_DIRECTORIES: &[&str] = &["migrations", "scripts", "assets", "testdata", "docs", "docs/adr", "fuzz"];
const SKIPPED_DIRECTORIES: &[&str] = &[".git", "dist", "target", "node_modules", "corpus", "artifacts", "coverage"];
const README_SECTIONS: &[&str] =
    &["Download", "Install", "Usage", "Documentation", "Security", "Contributing", "License"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                collect(&path, found);
            }
        } else {
            found.push(path);
        }
    }
}

fn project_files() -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&root(), &mut found);
    assert!(found.len() > 80, "only {} project files were found", found.len());
    found
}

fn shown(path: &Path) -> String {
    let base = root();
    path.strip_prefix(&base).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn extension(path: &Path) -> String {
    path.extension().map(|extension| extension.to_string_lossy().to_string()).unwrap_or_default()
}

fn read(path: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(path).unwrap()).replace("\r\n", "\n")
}

fn is_text(path: &Path) -> bool {
    !std::fs::read(path).unwrap().iter().take(8000).any(|byte| *byte == 0)
}

fn domain_length(host: &[char]) -> Option<usize> {
    let mut best = None;
    let mut label = 0;
    for (index, letter) in host.iter().enumerate() {
        if *letter != '.' {
            label += 1;
            continue;
        }
        if label == 0 {
            break;
        }
        label = 0;
        let letters = host[index + 1..].iter().take_while(|next| next.is_ascii_alphabetic()).count();
        if letters >= 2 {
            best = Some(index + 1 + letters);
        }
    }
    best
}

fn has_address(line: &str) -> bool {
    let letters: Vec<char> = line.chars().collect();
    let local = |letter: char| letter.is_ascii_alphanumeric() || "._%+-".contains(letter);
    let host = |letter: char| letter.is_ascii_alphanumeric() || letter == '-' || letter == '.';
    for (index, letter) in letters.iter().enumerate() {
        if *letter != '@' || index == 0 || !local(letters[index - 1]) {
            continue;
        }
        let reach = letters[index + 1..].iter().take_while(|next| host(**next)).count();
        if domain_length(&letters[index + 1..index + 1 + reach]).is_some() {
            return true;
        }
    }
    false
}

fn skip_string(letters: &[char], mut index: usize, line: &mut usize) -> usize {
    while index < letters.len() {
        match letters[index] {
            '\\' => {
                if letters.get(index + 1) == Some(&'\n') {
                    *line += 1;
                }
                index += 2;
            }
            '"' => return index + 1,
            '\n' => {
                *line += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    index
}

fn raw_string_hashes(letters: &[char], index: usize) -> Option<usize> {
    let word = |letter: char| letter.is_alphanumeric() || letter == '_';
    let free = match index {
        0 => true,
        1 => !word(letters[0]) || letters[0] == 'b',
        _ => !word(letters[index - 1]) || (letters[index - 1] == 'b' && !word(letters[index - 2])),
    };
    if !free {
        return None;
    }
    let hashes = letters[index + 1..].iter().take_while(|letter| **letter == '#').count();
    (letters.get(index + 1 + hashes) == Some(&'"')).then_some(hashes)
}

fn skip_raw_string(letters: &[char], mut index: usize, hashes: usize, line: &mut usize) -> usize {
    while index < letters.len() {
        if letters[index] == '\n' {
            *line += 1;
        }
        if letters[index] == '"' {
            let closing = letters[index + 1..].iter().take_while(|letter| **letter == '#').count();
            if closing >= hashes {
                return index + 1 + hashes;
            }
        }
        index += 1;
    }
    index
}

fn skip_quote(letters: &[char], index: usize) -> usize {
    if letters.get(index + 1) == Some(&'\\') {
        let mut end = index + 3;
        while end < letters.len() && letters[end] != '\'' {
            end += 1;
        }
        return end + 1;
    }
    if letters.get(index + 2) == Some(&'\'') {
        return index + 3;
    }
    index + 1
}

fn rust_comment_line(source: &str) -> Option<usize> {
    let letters: Vec<char> = source.chars().collect();
    let mut line = 1;
    let mut index = 0;
    while index < letters.len() {
        let next = letters.get(index + 1).copied();
        match letters[index] {
            '\n' => line += 1,
            '/' if next == Some('/') || next == Some('*') => return Some(line),
            '"' => {
                index = skip_string(&letters, index + 1, &mut line);
                continue;
            }
            'r' => {
                if let Some(hashes) = raw_string_hashes(&letters, index) {
                    index = skip_raw_string(&letters, index + 2 + hashes, hashes, &mut line);
                    continue;
                }
            }
            '\'' => {
                index = skip_quote(&letters, index);
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn declared_tests() -> BTreeSet<String> {
    let mut declared = BTreeSet::new();
    for path in project_files() {
        if extension(&path) != "rs" {
            continue;
        }
        if shown(&path).starts_with("fuzz/fuzz_targets/") {
            declared.insert(path.file_stem().unwrap().to_string_lossy().to_string());
        }
        let source = read(&path);
        let mut after_attribute = false;
        for line in source.lines().map(str::trim) {
            if line == "#[test]" {
                after_attribute = true;
            } else if after_attribute && !line.starts_with("#[") {
                if let Some(name) = line.strip_prefix("fn ").and_then(|rest| rest.split('(').next()) {
                    declared.insert(name.to_string());
                }
                after_attribute = false;
            }
        }
    }
    declared
}

fn looks_like_a_test_name(word: &str) -> bool {
    word.matches('_').count() >= 3
        && word.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '_')
        && !word.starts_with('_')
        && !word.ends_with('_')
}

#[test]
fn required_files_and_readme_sections_exist() {
    let base = root();
    for name in REQUIRED_FILES {
        let size = std::fs::metadata(base.join(name)).map(|data| if data.is_file() { data.len() } else { 0 });
        assert!(size.unwrap_or(0) > 0, "required file {name} is missing or empty");
    }
    for name in REQUIRED_DIRECTORIES {
        assert!(base.join(name).is_dir(), "required directory {name} is missing");
    }
    let readme = read(&base.join("README.md"));
    for section in README_SECTIONS {
        assert!(readme.contains(&format!("\n## {section}\n")), "README.md has no section {section}");
    }
    let index = read(&base.join("docs/README.md"));
    for name in REQUIRED_FILES {
        if let Some(page) = name.strip_prefix("docs/").filter(|page| *page != "README.md") {
            assert!(index.contains(&format!("({page})")), "docs/README.md does not link {page}");
        }
    }
}

#[test]
fn relative_links_in_documents_resolve() {
    let mut checked = 0;
    for path in project_files() {
        if extension(&path) != "md" || path.file_name().unwrap() == "THIRD_PARTY_NOTICES.md" {
            continue;
        }
        let document = read(&path);
        for part in document.split("](").skip(1) {
            let Some(target) = part.split(')').next() else {
                continue;
            };
            let file = target.split('#').next().unwrap_or_default();
            if file.is_empty() || file.contains("://") || file.contains(char::is_whitespace) {
                continue;
            }
            checked += 1;
            let wanted = path.parent().unwrap().join(file);
            assert!(wanted.exists(), "{} links to {file}, which does not exist", shown(&path));
        }
    }
    assert!(checked > 40, "only {checked} links were checked");
}

#[test]
fn no_email_address_appears_anywhere() {
    assert!(has_address(&["someone", "example.org"].join("@")));
    assert!(has_address(&["<first.last", "mail.example.co.uk>"].join("@")));
    assert!(!has_address("user@host and @mention and a@b.x1"));
    for path in project_files() {
        if !is_text(&path) {
            continue;
        }
        for (number, line) in read(&path).lines().enumerate() {
            assert!(!has_address(line), "{}:{} contains an email address", shown(&path), number + 1);
        }
    }
}

#[test]
fn source_files_have_no_comments() {
    let marker = ["/", "/ note"].concat();
    assert_eq!(rust_comment_line(&format!("fn main() {{}}\n{marker}\n")), Some(2));
    assert_eq!(
        rust_comment_line(&format!("let text = \"{marker}\";\nlet letter = '/';\nlet raw = r#\"{marker}\"#;\n")),
        None
    );
    assert_eq!(rust_comment_line(&format!("fn pick<'a>(text: &'a str) -> &'a str {{ text }} {marker}")), Some(1));
    let mut sources = 0;
    for path in project_files() {
        let name = shown(&path);
        match extension(&path).as_str() {
            "rs" => {
                sources += 1;
                let found = rust_comment_line(&read(&path));
                assert_eq!(found, None, "{name} contains a comment on line {}", found.unwrap_or(0));
            }
            "sql" => {
                for (number, line) in read(&path).lines().enumerate() {
                    let comment = line.trim().starts_with("--") || line.contains("/*");
                    assert!(!comment, "{name}:{} contains a comment", number + 1);
                }
            }
            "sh" | "bash" | "bats" => {
                for (number, line) in read(&path).lines().enumerate() {
                    let comment = line.trim().starts_with('#') && !line.starts_with("#!");
                    assert!(!comment, "{name}:{} contains a comment", number + 1);
                }
            }
            _ => {}
        }
    }
    assert!(sources > 40, "only {sources} Rust files were scanned");
}

#[test]
fn the_configuration_document_lists_every_setting() {
    let document = read(&root().join("docs/CONFIGURATION.md"));
    let mut documented: BTreeMap<String, String> = BTreeMap::new();
    for line in document.lines() {
        let cells: Vec<&str> = line.split('|').collect();
        if cells.len() < 4 || !cells[1].trim().starts_with('`') {
            continue;
        }
        let key = cells[1].trim().trim_matches('`');
        if settings::lookup(key).is_some() {
            documented.insert(key.to_string(), cells[2].trim().to_string());
        } else {
            let setting_like = key.contains('.') && !key.contains([' ', '-', '<']);
            assert!(!setting_like, "docs/CONFIGURATION.md documents {key}, which is not a setting");
        }
    }
    for definition in settings::DEFINITIONS {
        let value = documented.get(definition.key);
        assert!(value.is_some(), "docs/CONFIGURATION.md does not document {}", definition.key);
        let wanted =
            if definition.default.is_empty() { "empty".to_string() } else { format!("`{}`", definition.default) };
        assert_eq!(value, Some(&wanted), "docs/CONFIGURATION.md gives another default for {}", definition.key);
        for option in definition.options {
            assert!(
                document.contains(&format!("`{option}`")),
                "docs/CONFIGURATION.md does not mention the value {option} of {}",
                definition.key
            );
        }
    }
}

#[test]
fn tests_named_in_documents_exist() {
    let declared = declared_tests();
    assert!(declared.len() >= 150, "only {} tests were found", declared.len());
    let mut mentioned = 0;
    for path in project_files() {
        if extension(&path) != "md" || path.file_name().unwrap() == "THIRD_PARTY_NOTICES.md" {
            continue;
        }
        let document = read(&path);
        for word in document.split('`').skip(1).step_by(2) {
            if looks_like_a_test_name(word) {
                mentioned += 1;
                assert!(declared.contains(word), "{} names the test {word}, which does not exist", shown(&path));
            }
        }
    }
    assert!(mentioned >= 30, "only {mentioned} test names were found in the documents");
}

#[test]
fn the_generated_command_tables_are_current() {
    let current = read(&root().join("docs/COMMANDS.md"));
    let wanted = app_cli::commands_document(&current).unwrap();
    assert!(wanted == current, "docs/COMMANDS.md is out of date: run make docs");
}

const STORAGE_MODULES: &[&str] = &[
    "crates/templateapp-core/src/store.rs",
    "crates/templateapp-core/src/settings.rs",
    "crates/templateapp-core/src/records.rs",
    "crates/templateapp-core/src/notify.rs",
];
const DATABASE_USERS: &[&str] =
    &["crates/templateapp-core/src/error.rs", "fuzz/fuzz_targets/records_search_expression.rs"];
const STATEMENT_WORDS: &[&str] =
    &["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "PRAGMA ", " WHERE ", "ORDER BY", " FROM ", "CREATE ", "DROP "];
const ALLOWED_BUILDERS: &[&str] =
    &["format!(\"EXPLAIN QUERY PLAN {sql}\")", "ends_with(&format!(\"PRAGMA {marker}\"))"];
const TEXT_BUILDERS: &[&str] = &["format!(", "push_str(", "concat!(", "write!(", "writeln!("];

fn builds_a_statement(line: &str) -> bool {
    TEXT_BUILDERS.iter().any(|builder| line.contains(builder))
        && STATEMENT_WORDS.iter().any(|word| line.contains(word))
        && !ALLOWED_BUILDERS.iter().any(|allowed| line.contains(allowed))
}

#[test]
fn sql_stays_in_the_storage_modules_and_is_not_built_from_text() {
    assert!(builds_a_statement("let sql = format!(\"SELECT id FROM records WHERE name = '{name}'\");"));
    assert!(builds_a_statement("sql.push_str(\" ORDER BY name\");"));
    assert!(!builds_a_statement("const SQL_GET: &str = \"SELECT id FROM records WHERE id = ?1\";"));
    assert!(!builds_a_statement("let text = format!(\"{count} records\");"));
    let binding = ["rus", "qlite"].concat();
    let mut scanned = 0;
    for path in project_files() {
        if extension(&path) != "rs" {
            continue;
        }
        let name = shown(&path);
        let source = read(&path);
        if STORAGE_MODULES.contains(&name.as_str()) {
            scanned += 1;
            for (number, line) in source.lines().enumerate() {
                assert!(!builds_a_statement(line), "{name}:{} builds SQL from text", number + 1);
            }
        } else if !DATABASE_USERS.contains(&name.as_str()) {
            assert!(!source.contains(&binding), "{name} uses the database outside the storage modules");
        }
    }
    assert_eq!(scanned, STORAGE_MODULES.len());
}

#[test]
fn the_version_is_the_same_everywhere() {
    let version = env!("CARGO_PKG_VERSION");
    let base = root();
    let library = read(&base.join("scripts/lib.sh"));
    assert!(library.contains(&format!("APP_VERSION=\"{version}\"\n")), "scripts/lib.sh has another APP_VERSION");
    let notes = read(&base.join("docs/RELEASE_NOTES.md"));
    let title = notes.lines().next().unwrap_or_default();
    assert!(title.contains(&format!(" v{version} ")), "docs/RELEASE_NOTES.md is titled for another version: {title}");
    let changes = read(&base.join("CHANGELOG.md"));
    assert!(changes.contains(&format!("## [{version}]")), "CHANGELOG.md has no section for {version}");
}
