use std::time;

pub fn now_millis() -> u64 {
    time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64
}
