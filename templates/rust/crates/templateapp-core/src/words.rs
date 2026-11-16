pub fn count(total: i64, one: &str, many: &str) -> String {
    if total == 1 { format!("1 {one}") } else { format!("{total} {many}") }
}

#[cfg(test)]
mod tests {
    use super::count;

    #[test]
    fn count_chooses_the_singular_only_for_one() {
        assert_eq!(count(0, "record", "records"), "0 records");
        assert_eq!(count(1, "record", "records"), "1 record");
        assert_eq!(count(2, "record", "records"), "2 records");
    }
}
