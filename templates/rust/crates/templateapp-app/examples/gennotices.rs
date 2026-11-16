use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use app_core::meta;
use serde_json::Value;

const MAX_DEPTH: usize = 4;
const MAX_LICENSE_BYTES: usize = 200_000;
const ADDRESS_OMITTED: &str = "(address omitted)";
const SKIPPED_DIRECTORIES: &[&str] = &["target", "tests", "benches", "examples", "src", ".git", ".github"];
const SKIPPED_EXTENSIONS: &[&str] =
    &["rs", "toml", "json", "yml", "yaml", "py", "sh", "html", "js", "css", "lock", "c", "h", "cpp", "hpp", "png"];

struct Component {
    name: String,
    version: String,
    license: String,
    repository: String,
    directory: PathBuf,
}

struct Notice {
    component: String,
    file: String,
    copyrights: Vec<String>,
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn metadata(root: &Path) -> Result<Value, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--locked", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .output()
        .map_err(|problem| format!("cargo metadata did not start: {problem}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    serde_json::from_slice(&output.stdout).map_err(|problem| format!("cargo metadata printed no JSON: {problem}"))
}

fn shipped_ids(graph: &Value) -> BTreeSet<String> {
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let nodes = graph.pointer("/resolve/nodes").and_then(Value::as_array).cloned().unwrap_or_default();
    for node in &nodes {
        let mut targets = Vec::new();
        for dependency in node.get("deps").and_then(Value::as_array).into_iter().flatten() {
            let kinds = dependency.get("dep_kinds").and_then(Value::as_array).cloned().unwrap_or_default();
            let used =
                kinds.is_empty() || kinds.iter().any(|kind| kind.get("kind").and_then(Value::as_str) != Some("dev"));
            if used {
                targets.push(text(dependency, "pkg"));
            }
        }
        edges.insert(text(node, "id"), targets);
    }
    let mut seen = BTreeSet::new();
    let mut queue: Vec<String> = graph
        .get("workspace_members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|member| member.as_str().map(str::to_string))
        .collect();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(targets) = edges.get(&id) {
            queue.extend(targets.iter().cloned());
        }
    }
    seen
}

fn components(graph: &Value) -> Vec<Component> {
    let shipped = shipped_ids(graph);
    let mut found = Vec::new();
    for package in graph.get("packages").and_then(Value::as_array).into_iter().flatten() {
        let external = package.get("source").is_some_and(|source| !source.is_null());
        if !external || !shipped.contains(&text(package, "id")) {
            continue;
        }
        let manifest = PathBuf::from(text(package, "manifest_path"));
        let Some(directory) = manifest.parent() else {
            continue;
        };
        let declared = text(package, "license");
        found.push(Component {
            name: text(package, "name"),
            version: text(package, "version"),
            license: if declared.is_empty() { "see the license file".to_string() } else { declared },
            repository: text(package, "repository"),
            directory: directory.to_path_buf(),
        });
    }
    found.sort_by(|first, second| (&first.name, &first.version).cmp(&(&second.name, &second.version)));
    found
}

fn is_license_name(name: &str) -> bool {
    let upper = name.to_uppercase();
    let extension = upper.rsplit_once('.').map(|(_, extension)| extension.to_lowercase()).unwrap_or_default();
    if SKIPPED_EXTENSIONS.contains(&extension.as_str()) {
        return false;
    }
    let stem = upper.split('.').next().unwrap_or_default();
    ["LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE", "COPYRIGHT"].iter().any(|word| upper.contains(word))
        || stem == "OFL"
        || stem == "UFL"
}

fn license_files(directory: &Path, prefix: &str, depth: usize, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        let relative = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        if path.is_dir() {
            if depth < MAX_DEPTH && !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                license_files(&path, &relative, depth + 1, found);
            }
        } else if is_license_name(&name) {
            found.push((relative, path));
        }
    }
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

fn omit_addresses(line: &str) -> String {
    let letters: Vec<char> = line.chars().collect();
    let local = |letter: char| letter.is_ascii_alphanumeric() || "._%+-".contains(letter);
    let host = |letter: char| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-';
    let mut result = String::new();
    let mut copied = 0;
    let mut index = 0;
    while index < letters.len() {
        if letters[index] == '@' {
            let mut start = index;
            while start > copied && local(letters[start - 1]) {
                start -= 1;
            }
            let reach = letters[index + 1..].iter().take_while(|letter| host(**letter)).count();
            let domain = domain_length(&letters[index + 1..index + 1 + reach]);
            if let Some(length) = domain.filter(|_| start < index) {
                result.extend(&letters[copied..start]);
                result.push_str(ADDRESS_OMITTED);
                copied = index + 1 + length;
                index = copied;
                continue;
            }
        }
        index += 1;
    }
    result.extend(&letters[copied..]);
    result
}

fn is_copyright_line(line: &str) -> bool {
    if line.len() >= 200 {
        return false;
    }
    let lower = line.to_lowercase();
    let Some(rest) = lower.strip_prefix("copyright") else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with("(c)") || rest.starts_with('©') || rest.starts_with(|letter: char| letter.is_ascii_digit())
}

fn split_copyrights(raw: &str) -> (Vec<String>, String) {
    let normalized: String = raw
        .replace("\r\n", "\n")
        .chars()
        .filter(|letter| *letter == '\n' || *letter == '\t' || !letter.is_control())
        .collect();
    let mut copyrights: Vec<String> = Vec::new();
    let mut body: Vec<String> = Vec::new();
    for line in normalized.lines().map(str::trim_end) {
        let cleaned = omit_addresses(line);
        let trimmed = cleaned.trim();
        if is_copyright_line(trimmed) {
            if !copyrights.iter().any(|known| known == trimmed) {
                copyrights.push(trimmed.to_string());
            }
            continue;
        }
        body.push(cleaned);
    }
    let joined = body.join("\n");
    let mut compact = String::new();
    let mut blank = 0;
    for line in joined.trim_matches('\n').lines() {
        blank = if line.trim().is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            compact.push_str(line);
            compact.push('\n');
        }
    }
    (copyrights, compact)
}

fn fence(body: &str) -> String {
    let mut longest = 2;
    let mut current = 0;
    for letter in body.chars() {
        current = if letter == '`' { current + 1 } else { 0 };
        longest = longest.max(current);
    }
    "`".repeat(longest + 1)
}

fn render(list: &[Component]) -> String {
    let mut texts: Vec<(String, Vec<Notice>)> = Vec::new();
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut bare: Vec<&Component> = Vec::new();
    for component in list {
        let mut files = Vec::new();
        license_files(&component.directory, "", 0, &mut files);
        files.sort();
        let mut any = false;
        for (file, path) in files {
            let Ok(raw) = std::fs::read(&path) else {
                continue;
            };
            if raw.len() > MAX_LICENSE_BYTES || raw.iter().take(8000).any(|byte| *byte == 0) {
                continue;
            }
            let (copyrights, body) = split_copyrights(&String::from_utf8_lossy(&raw));
            if body.trim().is_empty() {
                continue;
            }
            any = true;
            let position = *positions.entry(body.clone()).or_insert_with(|| {
                texts.push((body.clone(), Vec::new()));
                texts.len() - 1
            });
            texts[position].1.push(Notice {
                component: format!("{} {}", component.name, component.version),
                file,
                copyrights,
            });
        }
        if !any {
            bare.push(component);
        }
    }

    let mut page = String::new();
    page.push_str("# Third-party notices\n\n");
    page.push_str(&format!(
        "{} is released under the MIT license (see `LICENSE`). The release binaries also contain the third-party components listed here. Each component keeps its own license; none of them is relicensed.\n\n",
        meta::APP_NAME
    ));
    page.push_str(&format!(
        "This file is generated by `make notices` from the crate versions in `Cargo.lock` and the license files those crates ship. Do not edit it by hand. E-mail addresses in upstream copyright lines are replaced by \"{ADDRESS_OMITTED}\" because this repository contains no e-mail addresses; the unmodified files ship with each crate at the listed version.\n\n"
    ));
    page.push_str(&format!(
        "The list covers every supported operating system, so it names crates that the build for one system does not contain. Crates used only by the tests are not listed. It has {} crates and {} distinct license texts.\n\n",
        list.len(),
        texts.len()
    ));
    page.push_str("## Components bundled inside dependencies\n\n");
    page.push_str("Some dependencies carry further components that end up in the binary:\n\n");
    page.push_str("- Fonts shipped by the dashboard toolkit egui in the crate `epaint_default_fonts`. Their license files are reproduced below under that crate.\n");
    page.push_str("- The SQLite library (public domain), compiled in through `libsqlite3-sys`.\n\n");
    page.push_str("The application's own icons (navigation, status, severity and action icons, and the application icon) are original drawings released under the application's license.\n\n");
    page.push_str("## Components\n\n| Crate | Version | License | Source |\n|---|---|---|---|\n");
    for component in list {
        let source = if component.repository.is_empty() {
            format!("https://crates.io/crates/{}", component.name)
        } else {
            component.repository.clone()
        };
        page.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            component.name,
            component.version,
            component.license.replace('|', "\\|"),
            source
        ));
    }
    if !bare.is_empty() {
        page.push_str("\n## Crates published without a license file\n\n");
        page.push_str("These crates state their license in their manifest and ship no license file. The license named in the table above applies; the text is in the repository of the crate.\n\n");
        for component in bare {
            page.push_str(&format!("- {} {} ({})\n", component.name, component.version, component.license));
        }
    }
    page.push_str("\n## License texts\n\n");
    page.push_str("Each text is printed once. Copyright lines are taken out of the text and listed with the crate they belong to.\n");
    for (number, (body, notices)) in texts.iter().enumerate() {
        page.push_str(&format!("\n### License text {}\n\nUsed by:\n\n", number + 1));
        for notice in notices {
            page.push_str(&format!("- {} (`{}`)", notice.component, notice.file));
            if !notice.copyrights.is_empty() {
                page.push_str(&format!(": {}", notice.copyrights.join("; ")));
            }
            page.push('\n');
        }
        let marks = fence(body);
        page.push_str(&format!("\n{marks}text\n{body}{marks}\n"));
    }
    page
}

fn run(check: bool) -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = metadata(&root)?;
    let list = components(&graph);
    if list.is_empty() {
        return Err("cargo metadata listed no third-party crate".to_string());
    }
    let wanted = render(&list);
    let path = root.join("THIRD_PARTY_NOTICES.md");
    let current = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
    if wanted == current {
        return Ok(());
    }
    if check {
        return Err("THIRD_PARTY_NOTICES.md is out of date: run make notices".to_string());
    }
    std::fs::write(&path, wanted).map_err(|problem| format!("THIRD_PARTY_NOTICES.md cannot be written: {problem}"))?;
    println!("wrote THIRD_PARTY_NOTICES.md ({} crates)", list.len());
    Ok(())
}

fn main() -> ExitCode {
    let mut check = false;
    for argument in std::env::args().skip(1) {
        match argument.trim_start_matches('-') {
            "check" => check = true,
            _ => {
                eprintln!("error: unknown argument {argument}. Usage: gennotices [-check]");
                return ExitCode::from(2);
            }
        }
    }
    match run(check) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("error: {problem}");
            ExitCode::FAILURE
        }
    }
}
