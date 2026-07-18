pub fn format_record(fields: &[(&str, String)]) -> String {
    let mut record = String::from("competitive_benchmark");
    for (key, value) in fields {
        record.push(' ');
        record.push_str(key);
        record.push('=');
        record.push_str(value);
    }
    record
}

#[allow(dead_code)]
pub fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key}=");
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("missing {key} in benchmark record: {line}"))
        .trim_matches('"')
}
