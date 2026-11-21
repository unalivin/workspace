use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FieldMapping {
    pub source_field: String,
    pub target_field: String,
}

pub fn apply_field_mappings(
    input: &HashMap<String, String>,
    mappings: &[FieldMapping],
) -> HashMap<String, String> {
    let mut out = input.clone();
    for m in mappings {
        if let Some(val) = input.get(&m.source_field) {
            out.insert(m.target_field.clone(), val.clone());
        }
    }
    out
}
