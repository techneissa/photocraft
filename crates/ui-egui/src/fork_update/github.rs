//! The one place that talks to GitHub: a small trait the checks use and tests fake, and the real
//! reader (public data only, no token, a generic user agent).

/// One answer from GitHub's API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Body(String),
    NotFound,
    /// Out of requests; `minutes` until the quota returns, if GitHub said.
    RateLimited {
        minutes: Option<u64>,
    },
}

/// Reads one GitHub API address. Tests give canned answers; the app gives [`Github`].
pub trait Fetch: Send + Sync {
    fn get(&self, url: &str) -> Result<Reply, String>;
}

/// GitHub's API through `craft-update`'s HTTP client.
pub struct Github;

impl Fetch for Github {
    fn get(&self, url: &str) -> Result<Reply, String> {
        use craft_update::http::Fetched;
        let http = craft_update::Http::new("PhotoCraft/Update")?;
        match http.get_api(url, None).map_err(friendly)? {
            Fetched::Body { text, .. } => Ok(Reply::Body(text)),
            Fetched::NotFound => Ok(Reply::NotFound),
            Fetched::RateLimited { reset } => {
                let minutes = reset.map(|at| u64::try_from(at.saturating_sub(craft_update::now()).max(0)).unwrap_or(0).div_ceil(60));
                Ok(Reply::RateLimited { minutes })
            }
            Fetched::NotModified => Err("GitHub sent an answer this check did not ask for.".into()),
        }
    }
}

/// A transport error as one short sentence.
pub fn friendly(error: String) -> String {
    let mut chars = error.trim().chars();
    let mut text: String = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => return "Could not reach GitHub.".into(),
    };
    if text.chars().count() > 120 {
        text = text.chars().take(117).collect::<String>() + "…";
    }
    if !text.ends_with(['.', '…']) {
        text.push('.');
    }
    text
}
