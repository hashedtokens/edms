//! The "base URL" of an endpoint: scheme + host (+ port), nothing else.
//! Stored in its own `history.base_url` column so a card can show the
//! domain without parsing the full endpoint string.

/// `https://api.microsoft.com:8443/v1/users?x=1` -> `https://api.microsoft.com:8443`.
/// Lower-cased, credentials (`user:pass@`) dropped. `None` when the string
/// is not an absolute URL (no `scheme://host`).
pub fn base_url_of(endpoint_str: &str) -> Option<String> {
    let s = endpoint_str.trim();
    let (scheme, rest) = s.split_once("://")?;
    if scheme.is_empty() || !scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    let authority = rest.split(|c| matches!(c, '/' | '?' | '#')).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    if host.is_empty() {
        return None;
    }
    Some(format!("{}://{}", scheme.to_ascii_lowercase(), host.to_ascii_lowercase()))
}

/// What is left of `endpoint_str` after its base URL (`/v1/users?x=1`);
/// `""` when there is nothing after it, the whole string when it has no
/// base URL.
pub fn path_of(endpoint_str: &str) -> String {
    let s = endpoint_str.trim();
    let Some((_, rest)) = s.split_once("://") else { return s.to_string() };
    match rest.find(|c| matches!(c, '/' | '?' | '#')) {
        Some(i) => rest[i..].to_string(),
        None => String::new(),
    }
}
