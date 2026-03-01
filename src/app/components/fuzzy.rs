use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use std::sync::{LazyLock, Mutex};

static GLOBAL_MATCHER: LazyLock<Mutex<Matcher>> =
    LazyLock::new(|| Mutex::new(Matcher::new(Config::DEFAULT)));

#[allow(dead_code)]
pub fn fuzzy_match<I>(query: &str, entries: I) -> Vec<usize>
where
    I: IntoIterator<Item = (usize, String)>,
{
    let entries_vec: Vec<(usize, String)> = entries.into_iter().collect();

    if query.is_empty() {
        return entries_vec.into_iter().map(|(idx, _)| idx).collect();
    }

    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = GLOBAL_MATCHER.lock().unwrap();

    let mut char_buf = Vec::new();
    let mut indices_buf = Vec::new();

    let mut matches: Vec<(u32, usize)> = entries_vec
        .iter()
        .filter_map(|(idx, text)| {
            char_buf.clear();
            indices_buf.clear();
            let haystack = Utf32Str::new(text, &mut char_buf);
            pattern
                .indices(haystack, &mut matcher, &mut indices_buf)
                .map(|score| (score, *idx))
        })
        .collect();

    matches.sort_by(|(s_a, i_a), (s_b, i_b)| s_b.cmp(s_a).then_with(|| i_a.cmp(i_b)));

    matches.into_iter().map(|(_, idx)| idx).collect()
}

pub fn fuzzy_match_positioned<I>(query: &str, entries: I) -> Vec<(usize, Vec<usize>)>
where
    I: IntoIterator<Item = (usize, String)>,
{
    let entries_vec: Vec<(usize, String)> = entries.into_iter().collect();

    if query.is_empty() {
        return entries_vec
            .into_iter()
            .map(|(idx, _)| (idx, Vec::new()))
            .collect();
    }

    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut matcher = GLOBAL_MATCHER.lock().unwrap();

    let mut char_buf = Vec::new();
    let mut indices_buf = Vec::new();

    let mut matches: Vec<(u32, usize, Vec<usize>)> = entries_vec
        .iter()
        .filter_map(|(idx, text)| {
            char_buf.clear();
            indices_buf.clear();
            let haystack = Utf32Str::new(text, &mut char_buf);
            pattern
                .indices(haystack, &mut matcher, &mut indices_buf)
                .map(|score| {
                    let positions: Vec<usize> = indices_buf.iter().map(|&i| i as usize).collect();
                    (score, *idx, positions)
                })
        })
        .collect();

    matches.sort_by(|(s_a, i_a, _), (s_b, i_b, _)| s_b.cmp(s_a).then_with(|| i_a.cmp(i_b)));

    matches
        .into_iter()
        .map(|(_, idx, positions)| (idx, positions))
        .collect()
}
