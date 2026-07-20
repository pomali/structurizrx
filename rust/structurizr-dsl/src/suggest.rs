//! "Did you mean …?" suggestions for parse errors.

/// Return the candidate closest to `word` (case-insensitive Levenshtein),
/// provided it is close enough to plausibly be a typo: distance ≤ 1 for short
/// words, ≤ ⌈len/3⌉ for longer ones. Ties resolve to the first candidate.
pub fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let word_lower = word.to_lowercase();
    let max_distance = (word_lower.chars().count().div_ceil(3)).max(1);
    let mut best: Option<(usize, &str)> = None;
    for cand in candidates {
        let d = levenshtein(&word_lower, &cand.to_lowercase());
        if d <= max_distance && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, cand));
        }
    }
    best.map(|(_, c)| c.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_close_match() {
        assert_eq!(closest("statuss", ["status", "tags"]), Some("status".to_string()));
        assert_eq!(closest("shoop", ["shop", "user"]), Some("shop".to_string()));
    }

    #[test]
    fn rejects_distant_match() {
        assert_eq!(closest("zzz", ["status", "tags"]), None);
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(closest("softwaresystm", ["softwareSystem"]), Some("softwareSystem".to_string()));
    }
}
