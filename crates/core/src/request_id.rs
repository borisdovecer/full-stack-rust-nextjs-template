//! The request id: taken from `x-request-id` when it is safe, made otherwise, and visible to
//! everything that runs while the request is handled.

use std::future::Future;

pub const HEADER: &str = "x-request-id";

tokio::task_local! {
    static REQUEST_ID: String;
}

/// The id of the request being handled, if any.
pub fn current() -> Option<String> {
    REQUEST_ID.try_with(Clone::clone).ok()
}

/// Runs `fut` with `id` as the current request id.
pub async fn scope<F: Future>(id: String, fut: F) -> F::Output {
    REQUEST_ID.scope(id, fut).await
}

/// An incoming id is kept only when it is short and plain, so it cannot inject into logs.
pub fn accept(incoming: Option<&str>) -> Option<String> {
    let id = incoming?;
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    ok.then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_ids_only() {
        assert_eq!(accept(Some("abc-123_X")), Some("abc-123_X".into()));
        assert_eq!(accept(Some("")), None);
        assert_eq!(accept(Some("a\nb")), None);
        assert_eq!(accept(Some(&"a".repeat(65))), None);
        assert_eq!(accept(None), None);
    }

    #[tokio::test]
    async fn scope_sets_current() {
        assert_eq!(current(), None);
        let seen = scope("r1".into(), async { current() }).await;
        assert_eq!(seen.as_deref(), Some("r1"));
    }
}
