//! Password hashing for the routes. Argon2 is slow and holds about 19 MiB per call, so it runs
//! on blocking threads, at most `password.max_concurrent_hashes` at a time; more calls wait.

use std::sync::Arc;

use tokio::sync::Semaphore;

use app_auth::password;
use app_core::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct Passwords {
    permits: Arc<Semaphore>,
}

impl Passwords {
    /// `max` hashes at once; 0 means twice the CPU cores.
    pub fn new(max: u32) -> Self {
        let max = match usize::try_from(max) {
            Ok(n) if n > 0 => n,
            _ => std::thread::available_parallelism().map_or(2, |n| n.get()) * 2,
        };
        Self {
            permits: Arc::new(Semaphore::new(max)),
        }
    }

    /// Runs `f` on a blocking thread once a permit is free. The permit moves into the thread,
    /// so a request that gives up waiting still cannot add another hash to the ones running.
    async fn run<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(AppError::internal)?;
        tokio::task::spawn_blocking(move || {
            let out = f();
            drop(permit);
            out
        })
        .await
        .map_err(AppError::internal)
    }

    pub async fn hash(&self, pw: String) -> AppResult<String> {
        self.run(move || password::hash(&pw))
            .await?
            .map_err(AppError::internal)
    }

    /// Hashes a password when there is one.
    pub async fn hash_some(&self, pw: Option<String>) -> AppResult<Option<String>> {
        match pw {
            Some(pw) => Ok(Some(self.hash(pw).await?)),
            None => Ok(None),
        }
    }

    pub async fn verify(&self, hash: String, pw: String) -> AppResult<bool> {
        self.run(move || password::verify(&hash, &pw)).await
    }

    /// Spends the time of a real check, for an account that has no password to check.
    pub async fn verify_dummy(&self, pw: String) -> AppResult<()> {
        self.run(move || password::verify_dummy(&pw)).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn hashes_wait_for_a_free_permit() {
        let p = Passwords::new(1);
        let held = Arc::clone(&p.permits).acquire_owned().await.unwrap();
        let waited = tokio::time::timeout(
            Duration::from_millis(200),
            p.hash("a-long-password!".into()),
        )
        .await;
        assert!(waited.is_err(), "hashed without a permit");
        drop(held);
        let h = p.hash("a-long-password!".into()).await.unwrap();
        assert!(p.verify(h, "a-long-password!".into()).await.unwrap());
    }

    #[test]
    fn zero_means_twice_the_cores() {
        assert!(Passwords::new(0).permits.available_permits() >= 2);
        assert_eq!(Passwords::new(3).permits.available_permits(), 3);
    }
}
