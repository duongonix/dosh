use std::collections::BTreeSet;

pub fn collect_words(lines: &[String]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in lines {
        for token in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if token.len() >= 3 {
                out.insert(token.to_string());
            }
        }
    }
    out
}

pub fn suggest(prefix: &str, words: &BTreeSet<String>) -> Option<String> {
    if prefix.len() < 2 {
        return None;
    }
    words
        .iter()
        .find(|w| w.starts_with(prefix) && w.as_str() != prefix)
        .cloned()
}
