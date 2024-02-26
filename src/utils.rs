use std::time::Instant;

pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}
