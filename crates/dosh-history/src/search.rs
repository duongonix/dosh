use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

pub fn suggest(entries: &[String], prefix: &str) -> Option<String> {
    let p = prefix.trim();
    if p.is_empty() {
        return None;
    }

    entries
        .iter()
        .rev()
        .find(|entry| entry.starts_with(p) && entry.as_str() != p)
        .cloned()
}

pub fn fuzzy_search(entries: &[String], query: &str, limit: usize) -> Vec<String> {
    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(i64, &String)> = entries
        .iter()
        .filter_map(|entry| {
            matcher
                .fuzzy_match(entry, query)
                .map(|score| (score, entry))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, s)| s.clone())
        .collect()
}
