use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

/// `npe.` plus the unpadded base64url of the storage path, with one leading slash.
pub fn encode_cookie_name(path: &str) -> String {
    let canonical = format!("/{}", path.trim_matches('/'));
    format!("npe.{}", URL_SAFE_NO_PAD.encode(canonical.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_normalized_path_without_padding() {
        assert_eq!(encode_cookie_name("/crypto.nped"), "npe.L2NyeXB0by5ucGVk");
        assert_eq!(
            encode_cookie_name("crypto.nped/"),
            encode_cookie_name("/crypto.nped")
        );
        assert!(!encode_cookie_name("/a").contains('='));
    }
}
