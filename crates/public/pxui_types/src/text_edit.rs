//! Pure text-editing helpers: line bounds, fuzzy matching and number-literal scrubbing.

/// The byte index where the line holding byte `i` of `text` starts: just after the previous `\n`, or 0.
///
/// Allocates: never.
/// Panics: when `i` is past the end of `text`.
///
/// ```
/// assert_eq!(pxui_types::text_edit::line_start("ab\ncd", 4), 3);
/// ```
pub fn line_start(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let mut i = i;
    while i > 0 && b[i - 1] != b'\n' {
        i -= 1;
    }
    i
}

/// The byte index where the line holding byte `i` of `text` ends: the next `\n` at or after `i`, or the end of `text`.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// assert_eq!(pxui_types::text_edit::line_end("ab\ncd", 0), 2);
/// assert_eq!(pxui_types::text_edit::line_end("ab\ncd", 3), 5);
/// ```
pub fn line_end(text: &str, i: usize) -> usize {
    let b = text.as_bytes();
    let n = b.len();
    let mut i = i;
    while i < n && b[i] != b'\n' {
        i += 1;
    }
    i
}

/// Whether the bytes of `query` appear in order in `text`, comparing ASCII letters without regard to case (an empty query matches).
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::text_edit::fuzzy_match;
/// assert!(fuzzy_match("fb", "FooBar"));
/// assert!(!fuzzy_match("bf", "FooBar"));
/// ```
pub fn fuzzy_match(query: &str, text: &str) -> bool {
    fuzzy_score(query, text).is_some()
}

/// `fuzzy_match` with a rank: `None` when `query` is not a subsequence of `text`, else higher for a
/// better match. Each matched byte scores 1, plus 5 when it follows the previous match directly and
/// 8 when it starts a word (the text's start, after a non-alphanumeric byte, or an upper-case byte
/// after a lower-case one); each byte skipped before a match costs 1. A byte that cannot continue the
/// previous match takes the next word start that still leaves the rest of the query matchable.
/// No allocation (queries past 64 bytes match leftmost); O(text x query) at worst.
///
/// Allocates: never.
/// Panics: never.
///
/// ```
/// use pxui_types::text_edit::fuzzy_score;
/// assert!(fuzzy_score("abc", "abcd") > fuzzy_score("abc", "xaxbxc"));
/// assert_eq!(fuzzy_score("zz", "abc"), None);
/// ```
pub fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let (q, t) = (query.as_bytes(), text.as_bytes());
    let word_start = |i: usize| i == 0 || !t[i - 1].is_ascii_alphanumeric() || (t[i].is_ascii_uppercase() && t[i - 1].is_ascii_lowercase());
    // the latest position each query byte can take with the rest still matchable after it
    let mut limit = [usize::MAX; 64];
    let mut end = t.len();
    for k in (0..q.len()).rev() {
        end = (0..end).rev().find(|&i| t[i].eq_ignore_ascii_case(&q[k]))?;
        if k < 64 {
            limit[k] = end;
        }
    }
    let (mut score, mut at, mut last) = (0, 0usize, None::<usize>);
    for (k, &c) in q.iter().enumerate() {
        let hit = |i: &usize| t[*i].eq_ignore_ascii_case(&c);
        let first = (at..t.len()).find(hit)?;
        let i = if last.is_some_and(|l| l + 1 == first) || word_start(first) || k >= 64 {
            first
        } else {
            (first..=limit[k]).find(|i| hit(i) && word_start(*i)).unwrap_or(first)
        };
        score += 1 + if last.is_some_and(|l| l + 1 == i) { 5 } else { 0 } + if word_start(i) { 8 } else { 0 } - (i - at) as i32;
        (at, last) = (i + 1, Some(i));
    }
    Some(score)
}

/// A number literal dragged `dx` points: a float moves a tenth of its last decimal place per
/// point (Shift: ten times that) and keeps one more decimal; an integer moves one per six points,
/// as the inspector's integers do.
///
/// `dx` is the pointer travel in points; `coarse` (Shift) makes a float move ten times as far and does not affect integers. A literal
/// that is neither an integer nor a decimal comes back unchanged, and an integer that does not move is returned as it was written.
/// Allocates: one `String`.
/// Panics: never.
///
/// ```
/// use pxui_types::text_edit::scrubbed;
/// assert_eq!(scrubbed("10", 12.0, false), "12");
/// assert_eq!(scrubbed("1.5", 10.0, false), "1.6");
/// assert_eq!(scrubbed("1.5", 10.0, true), "2.5");
/// assert_eq!(scrubbed("0x10", 12.0, false), "0x10");
/// ```
pub fn scrubbed(literal: &str, dx: f32, coarse: bool) -> String {
    let dx = dx as f64;
    let int_value = literal.parse::<i64>().ok();
    match (literal.find('.'), int_value) {
        (None, Some(v)) => {
            let n = (dx / 6.0).round();
            if n == 0.0 {
                literal.to_owned()
            } else {
                (((v as f64) + n).round() as i64).to_string()
            }
        }
        (Some(dot), _) => {
            let decimals = literal.len() - dot - 1;
            let step = 10.0f64.powf(-((decimals + 1) as f64)) * if coarse { 10.0 } else { 1.0 };
            let v = (literal).parse::<f64>().ok().unwrap_or(0.0) + dx * step;
            let s = format!("{:.*}", decimals + 1, v);
            let mut n = s.len();
            while n > 0 && s.as_bytes()[n - 1] == b'0' {
                n -= 1;
            }
            if n > 0 && s.as_bytes()[n - 1] == b'.' { s[..n + 1].to_owned() } else { s[..n].to_owned() }
        }
        (None, None) => literal.to_owned(),
    }
}
