use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub level: usize,
    pub title: String,
    pub anchor: String,
    pub body: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guide {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub section: usize,
    pub title: String,
    pub excerpt: String,
}

fn anchor(title: &str, taken: &mut HashMap<String, usize>) -> String {
    let mut text = String::new();
    for letter in title.to_lowercase().chars() {
        if letter.is_alphanumeric() {
            text.push(letter);
        } else if matches!(letter, ' ' | '-' | '_') {
            text.push('-');
        }
    }
    let trimmed = text.trim_matches('-');
    let base = if trimmed.is_empty() { "section".to_string() } else { trimmed.to_string() };
    let mut candidate = base.clone();
    let mut number = taken.get(&base).copied().unwrap_or(0);
    while taken.contains_key(&candidate) {
        number += 1;
        candidate = format!("{base}-{number}");
    }
    taken.insert(base, number);
    taken.entry(candidate.clone()).or_insert(0);
    candidate
}

fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if level == 0 || level > 6 || !line[level..].starts_with(' ') {
        return None;
    }
    let title = line[level..].trim().trim_end_matches('#').trim();
    (!title.is_empty()).then(|| (level, title.to_string()))
}

pub fn parse(markdown: &str) -> Guide {
    let mut guide = Guide::default();
    let mut taken = HashMap::new();
    let mut body: Vec<&str> = Vec::new();
    let mut current: Option<(usize, String)> = None;
    let mut fenced = false;
    let normalized = markdown.replace("\r\n", "\n");
    let mut flush = |current: &Option<(usize, String)>, body: &mut Vec<&str>, guide: &mut Guide| {
        let text = body.join("\n").trim().to_string();
        body.clear();
        match current {
            Some((level, title)) => {
                let anchor = anchor(title, &mut taken);
                guide.sections.push(Section { level: *level, title: title.clone(), anchor, body: text });
            }
            None if !text.is_empty() => {
                let anchor = anchor("Introduction", &mut taken);
                guide.sections.push(Section { level: 1, title: "Introduction".to_string(), anchor, body: text });
            }
            None => {}
        }
    };
    for line in normalized.split('\n') {
        if line.trim().starts_with("```") {
            fenced = !fenced;
        }
        match heading(line) {
            Some(found) if !fenced => {
                flush(&current, &mut body, &mut guide);
                current = Some(found);
            }
            _ => body.push(line),
        }
    }
    flush(&current, &mut body, &mut guide);
    guide
}

fn excerpt(text: &str, position: usize, length: usize) -> String {
    let letters: Vec<char> = text.chars().collect();
    let before = text[..position].chars().count();
    let start = before.saturating_sub(40);
    let end = (start + 80 + length).min(letters.len());
    let mut out = letters[start..end].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 {
        out.insert(0, '…');
    }
    if end < letters.len() {
        out.push('…');
    }
    out
}

impl Guide {
    pub fn find(&self, anchor: &str) -> Option<usize> {
        self.sections.iter().position(|section| section.anchor == anchor)
    }

    pub fn search(&self, query: &str) -> Vec<Hit> {
        let lowered_query = query.to_lowercase();
        let terms: Vec<&str> = lowered_query.split_whitespace().collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for (index, section) in self.sections.iter().enumerate() {
            let haystack = format!("{}\n{}", section.title, section.body).to_lowercase();
            if !terms.iter().all(|term| haystack.contains(term)) {
                continue;
            }
            let lowered = section.body.to_lowercase();
            let excerpt = match lowered.find(terms[0]) {
                Some(position) if lowered.len() == section.body.len() && section.body.is_char_boundary(position) => {
                    excerpt(&section.body, position, terms[0].chars().count())
                }
                _ => section.title.clone(),
            };
            hits.push(Hit { section: index, title: section.title.clone(), excerpt });
        }
        hits
    }
}

pub fn plain(markdown: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut fenced = false;
    for line in markdown.replace("\r\n", "\n").split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            lines.push(format!("    {line}"));
            continue;
        }
        if trimmed.starts_with('|') {
            if trimmed.trim_matches(['|', '-', ':', ' ']).is_empty() {
                continue;
            }
            let cells: Vec<&str> = trimmed.trim_matches('|').split('|').map(str::trim).collect();
            lines.push(cells.join("  ·  ").replace("**", "").replace('`', ""));
            continue;
        }
        lines.push(line.replace("**", "").replace('`', ""));
    }
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SAMPLE: &str = "Lead text.\n\n# Usage\n\nIntro.\n\n## Getting started\n\nOpen the **dashboard** and press `n`.\n\n```\n# not a heading\ncommand --flag\n```\n\n## Getting started\n\nSecond section with the same title.\n\n### Tables ###\n\n| Key | Action |\n|-----|--------|\n| `q` | Quit |\n";

    #[test]
    fn headings_become_sections_with_unique_anchors() {
        let guide = parse(SAMPLE);
        let titles: Vec<(usize, &str, &str)> = guide
            .sections
            .iter()
            .map(|section| (section.level, section.title.as_str(), section.anchor.as_str()))
            .collect();
        assert_eq!(
            titles,
            vec![
                (1, "Introduction", "introduction"),
                (1, "Usage", "usage"),
                (2, "Getting started", "getting-started"),
                (2, "Getting started", "getting-started-1"),
                (3, "Tables", "tables"),
            ]
        );
        assert!(guide.sections[2].body.contains("# not a heading"));
        assert_eq!(guide.find("getting-started-1"), Some(3));
        assert_eq!(guide.find("missing"), None);
        let clashing = parse("# Step\n\n# Step\n\n# Step 1\n\n# Step\n\n# Step-1\n");
        let anchors: Vec<&str> = clashing.sections.iter().map(|section| section.anchor.as_str()).collect();
        assert_eq!(anchors, vec!["step", "step-1", "step-1-1", "step-2", "step-1-2"]);
        assert!(parse("").sections.is_empty());
        assert_eq!(parse("#NoSpace\n####### seven\n# \n").sections[0].title, "Introduction");
    }

    #[test]
    fn search_requires_every_term_and_shows_an_excerpt() {
        let guide = parse(SAMPLE);
        let hits = guide.search("DASHBOARD press");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].section, 2);
        assert!(hits[0].excerpt.contains("dashboard"), "{}", hits[0].excerpt);
        assert!(guide.search("dashboard missingword").is_empty());
        assert!(guide.search("   ").is_empty());
        assert_eq!(guide.search("tables")[0].excerpt, "Tables");
        let long = parse(&format!("# Long\n\n{} needle {}", "before ".repeat(30), "after ".repeat(30)));
        let excerpt = &long.search("needle")[0].excerpt;
        assert!(excerpt.starts_with('…') && excerpt.ends_with('…') && excerpt.contains("needle"), "{excerpt}");
    }

    #[test]
    fn plain_text_drops_markup_and_keeps_code_and_tables_readable() {
        let text = plain(SAMPLE);
        assert!(text.contains("Open the dashboard and press n."));
        assert!(text.contains("    command --flag"));
        assert!(text.contains("Key  ·  Action"));
        assert!(text.contains("q  ·  Quit"));
        assert!(!text.contains("```") && !text.contains("|--") && !text.contains("**"));
    }

    proptest! {
        #[test]
        fn parsing_searching_and_flattening_never_panic(markdown in "[#`| a-c0-2é…\\n-]{0,200}", query in ".{0,20}") {
            let guide = parse(&markdown);
            let _ = guide.search(&query);
            let _ = plain(&markdown);
            let mut anchors: Vec<&str> = guide.sections.iter().map(|section| section.anchor.as_str()).collect();
            let total = anchors.len();
            anchors.sort_unstable();
            anchors.dedup();
            prop_assert_eq!(anchors.len(), total);
        }
    }
}
