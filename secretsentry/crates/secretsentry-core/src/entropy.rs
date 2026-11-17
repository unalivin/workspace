pub const MAX_TENTHS: u32 = 80;

pub fn bits_per_character(text: &str) -> f64 {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for byte in bytes {
        counts[usize::from(*byte)] += 1;
    }
    let total = bytes.len() as f64;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let share = *count as f64 / total;
            -share * share.log2()
        })
        .sum()
}

pub fn tenths(text: &str) -> u32 {
    (bits_per_character(text) * 10.0).round() as u32
}

pub fn show(tenths: u32) -> String {
    format!("{}.{}", tenths / 10, tenths % 10)
}

pub fn parse(text: &str) -> Option<u32> {
    let text = text.trim();
    let (whole, tenth) = text.split_once('.').unwrap_or((text, "0"));
    if whole.is_empty() || tenth.len() != 1 || !whole.bytes().chain(tenth.bytes()).all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole: u32 = whole.parse().ok()?;
    let tenth: u32 = tenth.parse().ok()?;
    let tenths = whole.checked_mul(10)?.checked_add(tenth)?;
    (tenths <= MAX_TENTHS).then_some(tenths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn known_texts_have_the_expected_entropy() {
        assert_eq!(tenths(""), 0);
        assert_eq!(tenths("aaaaaaaa"), 0);
        assert_eq!(tenths("abababab"), 10);
        assert_eq!(tenths("abcdabcd"), 20);
        assert_eq!(tenths("0123456789abcdef"), 40);
        assert_eq!(show(46), "4.6");
        assert_eq!(show(0), "0.0");
        assert_eq!(show(80), "8.0");
    }

    #[test]
    fn thresholds_are_parsed_with_one_decimal() {
        assert_eq!(parse("0"), Some(0));
        assert_eq!(parse(" 3.5 "), Some(35));
        assert_eq!(parse("8.0"), Some(80));
        for wrong in ["", "high", "3.55", ".5", "3.", "-1", "8.1", "9", "99999999999"] {
            assert_eq!(parse(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn random_looking_text_scores_above_words() {
        assert!(tenths("correcthorsebattery") < tenths("q7Zp2LmX9vKc4RtB8nW"));
    }

    proptest! {
        #[test]
        fn entropy_stays_between_zero_and_the_size_of_the_alphabet(text in ".{0,200}") {
            let bits = bits_per_character(&text);
            prop_assert!(bits >= 0.0);
            prop_assert!(bits <= 8.0);
            let distinct = text.bytes().collect::<std::collections::BTreeSet<u8>>().len().max(1);
            prop_assert!(bits <= (distinct as f64).log2() + 1e-9);
        }
    }
}
