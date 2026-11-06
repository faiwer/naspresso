use axum::http::HeaderValue;

use crate::error::AppError;

/// `Set-Cookie` for `name` on `path`. `max_age` of `0` tells the browser to drop it.
pub(crate) fn set_cookie(
    name: &str,
    value: &str,
    path: &str,
    max_age: i64,
) -> Result<HeaderValue, AppError> {
    let cookie = cookie::Cookie::build((name, value))
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .path(path)
        .max_age(cookie::time::Duration::seconds(max_age))
        .build();
    HeaderValue::from_str(&cookie.to_string())
        .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))
}

/// The value of `name` in a `Cookie` header. A malformed pair is skipped.
pub(crate) fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    for cookie in cookie::Cookie::split_parse(header) {
        let Ok(cookie) = cookie else { continue };
        if cookie.name() == name {
            return cookie.value_raw();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_named_value_and_skips_a_broken_pair() {
        let header = "other=1; not a cookie; npe.abc=sealed";
        assert_eq!(cookie_value(header, "npe.abc"), Some("sealed"));
        assert_eq!(cookie_value(header, "missing"), None);
    }

    #[test]
    fn zero_max_age_clears_the_cookie() {
        let header = set_cookie("npe.abc", "", "/api", 0).unwrap();
        let text = header.to_str().unwrap();
        assert!(text.starts_with("npe.abc="));
        assert!(text.contains("Max-Age=0"));
        assert!(text.contains("Path=/api"));
        assert!(text.contains("HttpOnly"));
    }
}
