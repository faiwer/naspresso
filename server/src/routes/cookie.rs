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
}
