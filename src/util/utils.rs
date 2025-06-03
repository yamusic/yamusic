use std::time::{SystemTime, UNIX_EPOCH};

pub fn random(min: i32, max: i32) -> i32 {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();

    let mut rng = current_time as u64;

    let range = (max - min).unsigned_abs() as u64;

    rng = (rng % range) + min as u64;

    rng as i32
}
