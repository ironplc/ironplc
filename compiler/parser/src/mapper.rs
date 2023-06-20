use ironplc_dsl::common::Integer;
use time::Duration;

pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl DurationUnit {
    fn per_sec(&self) -> f32 {
        match *self {
            Self::Milliseconds => 0.001,
            Self::Seconds => 1.0,
            Self::Minutes => 60.0,
            Self::Hours => 3600.0,
            Self::Days => 3600.0 * 24.0,
        }
    }

    pub fn fp(&self, val: f32) -> Duration {
        Self::to_duration(val, self.per_sec())
    }

    pub fn int(&self, val: Integer) -> Duration {
        Self::to_duration(val.into(), self.per_sec())
    }

    fn to_duration(val: f32, unit_per_sec: f32) -> Duration {
        let secs = val * unit_per_sec;
        Duration::new(
            secs.trunc() as i64,
            (secs.fract() * 1_000_000_000f32) as i32,
        )
    }
}
