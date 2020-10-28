#![deny(missing_docs)]
//! P2p / dht communication framework.

mod types;
use futures::future::BoxFuture;
pub use types::*;

mod config;
pub use config::*;

mod spawn;
pub use spawn::*;

#[cfg(test)]
mod test;

pub mod fixt;

/// TODO: move to standalone util crate?
#[async_trait::async_trait]
pub trait FutureTimeoutExt<'a, T: 'a + Send>: futures::Future<Output = T>
where
    Self: Sized + Send + 'a,
{
    /// TODO
    fn timeout(self, duration: std::time::Duration, reason: &'static str) -> BoxFuture<'a, T> {
        use futures::FutureExt;
        async move {
            tokio::time::timeout(duration, self)
                .await
                .expect(&format!("We timed out!! {}", reason))
        }
        .boxed()
    }
}
impl<'a, F: Send + 'a, T: 'a + Send> FutureTimeoutExt<'a, T> for F where
    F: futures::Future<Output = T>
{
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[should_panic]
    async fn test_name() {
        tokio::time::delay_for(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(1), "because")
            .await;
    }
}
