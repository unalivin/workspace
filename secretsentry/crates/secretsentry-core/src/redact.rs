use crate::entropy;

pub const MASK: &str = "********";
pub const PREFIX_LENGTH: usize = 4;
pub const PREFIX_MIN_SECRET: usize = 16;
pub const MAX_CONTEXT_LENGTH: usize = 240;
pub const RANDOM_WORD_LENGTH: usize = 20;
pub const RANDOM_WORD_ENTROPY_TENTHS: u32 = 35;

const CUT: char = '…';

pub fn length(secret: &str) -> usize {
    secret.chars().count()
}

pub fn preview(secret: &str) -> String {
    let mut shown = String::new();
    if length(secret) >= PREFIX_MIN_SECRET {
        let prefix: String = secret.chars().take(PREFIX_LENGTH).collect();
        if prefix.chars().all(|letter| letter.is_ascii_alphanumeric() || "_-.".contains(letter)) {
            shown.push_str(&prefix);
        }
    }
    shown.push_str(MASK);
    shown
}

fn in_random_word(letter: char) -> bool {
    letter.is_ascii_alphanumeric() || "+/_=-".contains(letter)
}

fn looks_random(word: &str) -> bool {
    word.len() >= RANDOM_WORD_LENGTH
        && word.bytes().any(|byte| byte.is_ascii_digit())
        && entropy::tenths(word) >= RANDOM_WORD_ENTROPY_TENTHS
}

fn flush(out: &mut String, word: &mut String) {
    if looks_random(word) {
        out.push_str(MASK);
    } else {
        out.push_str(word);
    }
    word.clear();
}

pub fn mask_random_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for letter in text.chars() {
        if in_random_word(letter) {
            word.push(letter);
        } else {
            flush(&mut out, &mut word);
            out.push(letter);
        }
    }
    flush(&mut out, &mut word);
    out
}

pub fn printable(text: &str) -> String {
    text.chars().map(|letter| if letter.is_control() { ' ' } else { letter }).collect()
}

pub fn window(text: &str, focus: usize, limit: usize) -> String {
    let letters: Vec<char> = text.chars().collect();
    if letters.len() <= limit {
        return text.trim().to_string();
    }
    let mut start = focus.min(letters.len()).saturating_sub(limit / 3);
    let end = (start + limit).min(letters.len());
    if end == letters.len() {
        start = end - limit;
    }
    let mut shown = String::new();
    if start > 0 {
        shown.push(CUT);
    }
    shown.extend(letters[start..end].iter());
    if end < letters.len() {
        shown.push(CUT);
    }
    shown.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_preview_shows_at_most_four_characters_of_a_long_value() {
        assert_eq!(preview("abcdefghijklmnop"), "abcd********");
        assert_eq!(preview("abcdefghijklmno"), "********");
        assert_eq!(preview(""), "********");
        assert_eq!(preview("SG.abcdefghijklmnopqrstuv"), "SG.a********");
        assert_eq!(preview("ab\u{1b}[31mefghijklmnopqrstuv"), "********");
        assert_eq!(preview("ab cdefghijklmnopqrstuv"), "********");
        assert_eq!(length("häßlich"), 7);
    }

    #[test]
    fn long_random_looking_words_are_masked_and_ordinary_text_is_kept() {
        let random = ["q7Zp2LmX9v", "Kc4RtB8nWd", "3FgH6jS1"].concat();
        assert_eq!(mask_random_words(&format!("key = {random};")), "key = ********;");
        assert_eq!(mask_random_words("call authenticationTokenProvider now"), "call authenticationTokenProvider now");
        assert_eq!(mask_random_words("src/components/login/form.rs"), "src/components/login/form.rs");
        assert_eq!(mask_random_words("short 1a2b3c"), "short 1a2b3c");
        assert_eq!(mask_random_words(""), "");
    }

    #[test]
    fn control_characters_become_spaces() {
        assert_eq!(printable("a\tb\u{1b}[0m\u{7}c"), "a b [0m c");
    }

    #[test]
    fn a_long_line_is_cut_around_the_focus() {
        let line = format!("{}FOCUS{}", "a".repeat(300), "b".repeat(300));
        let shown = window(&line, 300, 60);
        assert!(shown.contains("FOCUS"));
        assert!(shown.starts_with('…') && shown.ends_with('…'));
        assert_eq!(shown.chars().count(), 62);
        assert_eq!(window("  short  ", 0, 60), "short");
        let tail = window(&line, 600, 60);
        assert!(tail.starts_with('…') && !tail.ends_with('…'));
        let head = window(&line, 0, 60);
        assert!(!head.starts_with('…') && head.ends_with('…'));
    }

    proptest! {
        #[test]
        fn a_preview_never_contains_more_than_the_prefix(secret in "[A-Za-z0-9_-]{0,80}") {
            let shown = preview(&secret);
            prop_assert!(shown.ends_with(MASK));
            let visible = shown.len() - MASK.len();
            prop_assert!(visible <= PREFIX_LENGTH);
            if length(&secret) < PREFIX_MIN_SECRET {
                prop_assert_eq!(visible, 0);
            }
        }

        #[test]
        fn masking_leaves_no_long_random_word(text in "[A-Za-z0-9+/=_ .:\"-]{0,200}") {
            let masked = mask_random_words(&text);
            for word in masked.split(|letter: char| !in_random_word(letter)) {
                prop_assert!(!looks_random(word));
            }
        }

        #[test]
        fn a_window_never_exceeds_the_limit(text in ".{0,400}", focus in 0usize..500, limit in 3usize..120) {
            let shown = window(&text, focus, limit);
            prop_assert!(shown.chars().count() <= limit + 2);
        }
    }
}
