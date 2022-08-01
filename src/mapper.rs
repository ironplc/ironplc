use crate::dsl::*;
use time::Duration;

pub fn to_duration(val: f32, unit_per_sec: f32) -> Duration {
    let secs = val * unit_per_sec;
    Duration::new(
        secs.trunc() as i64,
        (secs.fract() * 1_000_000_000f32) as i32,
    )
}

pub fn var_init_flat_map(
    declarations: Vec<Vec<VarInit>>,
    storage_class: Option<StorageClass>,
) -> Vec<VarInit> {
    let declarations = declarations.into_iter().flatten().collect::<Vec<VarInit>>();
    declarations
        .into_iter()
        .map(|declaration| {
            let storage = storage_class
                .clone()
                .unwrap_or_else(|| StorageClass::Unspecified);
            let mut declaration = declaration.clone();
            declaration.storage_class = storage;
            declaration
        })
        .collect()
}

pub fn to_strings(input: Vec<&str>) -> Vec<String> {
    input
        .into_iter()
        .map(|item| String::from(item))
        .collect()
}
