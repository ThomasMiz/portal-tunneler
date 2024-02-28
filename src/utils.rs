use std::time::Instant;

pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}

pub fn get_current_timestamp() -> u64 {
    let now = std::time::SystemTime::now();
    let unix_epoch = std::time::SystemTime::UNIX_EPOCH;
    let duration = now.duration_since(unix_epoch).expect("It is **NOT** 1970, fix your fucking clock");
    duration.as_secs()
}
