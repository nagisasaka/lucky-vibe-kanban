//! MCP substring search: literal whitespace, no normalisation or tokenisation.
pub fn matches(search: Option<&str>, fields: &[&str]) -> bool {
    search.is_none_or(|query| {
        let query = query.to_lowercase();
        fields
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn literal_unicode_substrings_and_independent_fields() {
        for (query, fields, expected) in [
            (None, vec!["日本語"], true),
            (Some(""), vec![""], true),
            (Some("ä"), vec!["Äpfel"], true),
            (Some("日本"), vec!["日本語"], true),
            (Some(" a "), vec!["A"], false),
            (Some("%_"), vec!["abc"], false),
            (Some("%_"), vec!["x%_y"], true),
            (Some("ab"), vec!["a", "b"], false),
            (Some("a"), vec!["Ａ"], false),
        ] {
            assert_eq!(super::matches(query, &fields), expected, "{query:?}");
        }
    }
}
