use time::Duration;

pub fn to_duration(val: f32, unit_per_sec: f32) -> Duration {
    let secs = val * unit_per_sec;
    Duration::new(
        secs.trunc() as i64,
        (secs.fract() * 1_000_000_000f32) as i32,
    )
}
