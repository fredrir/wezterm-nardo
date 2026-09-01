use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked<T> {
    pub item: T,
    pub score: u32,
    /// Char indices into the haystack that matched, for highlighting.
    pub indices: Vec<u32>,
}

pub struct Searcher {
    matcher: Matcher,
}

impl Default for Searcher {
    fn default() -> Self {
        Self { matcher: Matcher::new(Config::DEFAULT) }
    }
}

impl Searcher {
    /// Empty query → every item in input order with score 0 and no indices.
    /// Otherwise nucleo fuzzy (smart case), sorted by score desc then input order.
    pub fn rank<T>(&mut self, query: &str, items: impl IntoIterator<Item = (T, String)>) -> Vec<Ranked<T>> {
        let query = query.trim();
        if query.is_empty() {
            return items.into_iter().map(|(item, _)| Ranked { item, score: 0, indices: Vec::new() }).collect();
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut indices = Vec::new();
        let mut ranked: Vec<Ranked<T>> = items
            .into_iter()
            .filter_map(|(item, haystack)| {
                let haystack = Utf32Str::new(&haystack, &mut buf);
                indices.clear();
                let score = pattern.indices(haystack, &mut self.matcher, &mut indices)?;
                indices.sort_unstable();
                indices.dedup();
                Some(Ranked { item, score, indices: indices.clone() })
            })
            .collect();
        ranked.sort_by_key(|r| std::cmp::Reverse(r.score));
        ranked
    }
}

/// `d:archie w:main ws:dev #12 vim` → filters + free text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub domain: Option<String>,
    pub window: Option<String>,
    pub workspace: Option<String>,
    pub pane_id: Option<u64>,
}

impl Query {
    pub fn parse(raw: &str) -> Query {
        let mut query = Query::default();
        let mut text = Vec::new();
        for token in raw.split_whitespace() {
            if let Some(value) = token.strip_prefix("ws:") {
                set_filter(&mut query.workspace, value);
            } else if let Some(value) = token.strip_prefix("d:") {
                set_filter(&mut query.domain, value);
            } else if let Some(value) = token.strip_prefix("w:") {
                set_filter(&mut query.window, value);
            } else if let Some(id) = token.strip_prefix('#').and_then(|v| v.parse().ok()) {
                query.pane_id = Some(id);
            } else {
                text.push(token);
            }
        }
        query.text = text.join(" ");
        query
    }
}

fn set_filter(slot: &mut Option<String>, value: &str) {
    if !value.is_empty() {
        *slot = Some(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank_labels(query: &str, items: &[&str]) -> Vec<Ranked<usize>> {
        let mut searcher = Searcher::default();
        searcher.rank(query, items.iter().enumerate().map(|(i, s)| (i, s.to_string())))
    }

    #[test]
    fn empty_query_keeps_input_order() {
        let ranked = rank_labels("  ", &["b", "a", "c"]);
        let items: Vec<usize> = ranked.iter().map(|r| r.item).collect();
        assert_eq!(items, [0, 1, 2]);
        assert!(ranked.iter().all(|r| r.score == 0 && r.indices.is_empty()));
    }

    #[test]
    fn fuzzy_ranks_better_matches_first_and_drops_misses() {
        let ranked = rank_labels("vim", &["neovim ~/x", "zsh", "vim ~/y", "v i m spaced"]);
        let items: Vec<usize> = ranked.iter().map(|r| r.item).collect();
        assert_eq!(items[0], 2, "exact contiguous prefix match wins");
        assert!(!items.contains(&1), "non-matching items are dropped");
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn indices_are_sorted_deduped_char_offsets() {
        let ranked = rank_labels("ab", &["xa-b", "áab"]);
        let first = ranked.iter().find(|r| r.item == 0).unwrap();
        assert_eq!(first.indices, [1, 3]);
        let second = ranked.iter().find(|r| r.item == 1).unwrap();
        assert!(second.indices.windows(2).all(|w| w[0] < w[1]));
        assert!(second.indices.iter().all(|&i| i < 3), "indices are char offsets, not bytes");
    }

    #[test]
    fn ties_keep_input_order() {
        let ranked = rank_labels("zsh", &["zsh", "zsh", "zsh"]);
        let items: Vec<usize> = ranked.iter().map(|r| r.item).collect();
        assert_eq!(items, [0, 1, 2]);
    }

    #[test]
    fn smart_case_is_insensitive_for_lowercase_queries() {
        let ranked = rank_labels("vim", &["VIM"]);
        assert_eq!(ranked.len(), 1);
        let ranked = rank_labels("VIM", &["vim"]);
        assert!(ranked.is_empty());
    }

    #[test]
    fn query_parse_extracts_filters_and_text() {
        let q = Query::parse("d:archie  w:main ws:dev #12 vim ~/x");
        assert_eq!(q.domain.as_deref(), Some("archie"));
        assert_eq!(q.window.as_deref(), Some("main"));
        assert_eq!(q.workspace.as_deref(), Some("dev"));
        assert_eq!(q.pane_id, Some(12));
        assert_eq!(q.text, "vim ~/x");
    }

    #[test]
    fn query_parse_ignores_empty_filters_and_keeps_odd_tokens_as_text() {
        let q = Query::parse("d: #x #");
        assert_eq!(q, Query { text: "#x #".into(), ..Query::default() });
        assert_eq!(Query::parse(""), Query::default());
    }
}
