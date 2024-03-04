use std::time::Instant;

/// Sleeps until the provided instant if `Some`, or never finishes if `None`.
pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}

/// Gets the current system time as a unix timestamp.
///
// Panics with a funny message if the system's date is before 1970.
pub fn get_current_timestamp() -> u64 {
    let now = std::time::SystemTime::now();
    let unix_epoch = std::time::SystemTime::UNIX_EPOCH;
    let duration = now.duration_since(unix_epoch).expect("It is **NOT** 1970, fix your fucking clock");
    duration.as_secs()
}
