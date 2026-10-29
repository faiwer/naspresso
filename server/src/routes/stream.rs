use axum::{
    body::Body,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::Response,
};

/// A `200` with `body` of `len` bytes, revalidated by `etag`.
pub(super) fn stream(body: Body, len: u64, mime: &'static str, etag: HeaderValue) -> Response {
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
    set_cache_headers(headers, etag);
    response
}

/// A `304` when the client already has `etag`.
pub(super) fn not_modified(request: &HeaderMap, etag: &HeaderValue) -> Option<Response> {
    if request.get(header::IF_NONE_MATCH) != Some(etag) {
        return None;
    }
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NOT_MODIFIED;
    set_cache_headers(response.headers_mut(), etag.clone());
    Some(response)
}

/// A strong ETag from a hex fingerprint.
pub(super) fn etag(fingerprint: &str) -> HeaderValue {
    HeaderValue::try_from(format!("\"{fingerprint}\"")).expect("a hex etag is a valid header value")
}

fn set_cache_headers(headers: &mut HeaderMap, etag: HeaderValue) {
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::ETAG, etag);
}
