fn pick<'a>(x: &'a str) -> &'a str { x }
const A: &str = r#"// not a comment "quoted" /* no */"#;
const B: char = '/';
const C: &str = "multi
// still string
";
const D: &[u8] = br"//";
const E: char = '\'';
const F: &str = r##"one "# two // three"##;
