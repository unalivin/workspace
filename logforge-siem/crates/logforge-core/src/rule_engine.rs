use crate::model::DetectionRule;

pub fn evaluate_rules<'a>(rules: &'a [DetectionRule], message: &str) -> Vec<&'a DetectionRule> {
    let lower = message.to_ascii_lowercase();
    rules
        .iter()
        .filter(|r| r.enabled && lower.contains(&r.pattern.to_ascii_lowercase()))
        .collect()
}
