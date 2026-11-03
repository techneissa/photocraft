//! The dim line under a Home preview: the image size in pixels and when the file was last opened,
//! e.g. `6000 × 4000 · 2 h ago`. A part that is not known is left out.

/// `6000 × 4000 · 2 h ago`: `size` in pixels once the preview is decoded, `opened` and `now` in
/// seconds since 1970. Empty when neither is known.
pub(super) fn line(size: Option<(u32, u32)>, opened: Option<u64>, now: u64) -> String {
    let mut parts = Vec::new();
    if let Some((w, h)) = size {
        parts.push(format!("{w} × {h}"));
    }
    if let Some(t) = opened {
        parts.push(ago(now, t));
    }
    parts.join(" · ")
}

/// How long before `now` a file was opened: minutes, hours, `Yesterday` (one to two days), then
/// the date (`3 Nov`). Dates are in UTC.
fn ago(now: u64, then: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..60 => tl!("Just now").to_string(),
        60..3600 => crate::i18n::fmt(tl!("{n} min ago"), &[("n", &(secs / 60).to_string())]),
        3600..86_400 => crate::i18n::fmt(tl!("{n} h ago"), &[("n", &(secs / 3600).to_string())]),
        86_400..172_800 => tl!("Yesterday").to_string(),
        _ => i64::try_from(then).ok().and_then(|s| chrono::DateTime::from_timestamp(s, 0)).map_or_else(String::new, |d| d.format("%-d %b").to_string()),
    }
}
