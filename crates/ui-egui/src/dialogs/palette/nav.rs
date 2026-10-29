//! The palette's list rules: row size, ranking, and how ↑/↓/PageUp/PageDown move the highlight.

/// One palette row: score, command id, label, detail text, enabled.
pub(super) type Hit = (i32, String, String, Option<String>, bool);

/// A row's height.
pub(super) const ROW_H: f32 = 32.0;
/// The most rows shown at once; more scroll.
pub(super) const MAX_ROWS: usize = 12;
/// How far PageUp / PageDown move the highlight.
pub(super) const PAGE: usize = 10;

/// Bonus for a label in which every word of the query starts a word ("sel inv" finds Select ›
/// Inverse), more when the label itself starts with the query. Puts those ahead of loose matches
/// whose letters are only spread through the label.
pub(super) fn word_start_bonus(query: &str, label: &str) -> i32 {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 0;
    }
    let label = label.to_lowercase();
    let words: Vec<&str> = label.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if !q.split_whitespace().all(|tok| words.iter().any(|w| w.starts_with(tok))) {
        0
    } else if label.starts_with(&q) {
        400
    } else {
        300
    }
}

/// Best first; a command that can't run now comes after every one that can.
pub(super) fn rank(hits: &mut [Hit]) {
    hits.sort_by(|a, b| b.4.cmp(&a.4).then(b.0.cmp(&a.0)).then(a.2.cmp(&b.2)));
}

/// The highlight after ↑/↓ (`delta` ±1): wraps around the list.
pub(super) fn step(selected: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let len = len as isize;
    (selected as isize + delta).rem_euclid(len) as usize
}

/// The highlight after PageUp / PageDown (`delta` ±[`PAGE`]): stops at the ends, so a page never wraps.
pub(super) fn page(selected: usize, delta: isize, len: usize) -> usize {
    ((selected as isize + delta).max(0) as usize).min(len.saturating_sub(1))
}

/// The scroll offset that keeps row `selected` inside a view of `height` starting at `offset`.
pub(super) fn reveal(offset: f32, selected: usize, height: f32) -> f32 {
    let top = selected as f32 * ROW_H;
    let bottom = top + ROW_H;
    if top < offset {
        top
    } else if bottom > offset + height {
        bottom - height
    } else {
        offset
    }
}
