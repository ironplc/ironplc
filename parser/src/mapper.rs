use ironplc_dsl::ast::Id;
use ironplc_dsl::dsl::*;
use time::Duration;

pub fn to_duration(val: f32, unit_per_sec: f32) -> Duration {
    let secs = val * unit_per_sec;
    Duration::new(
        secs.trunc() as i64,
        (secs.fract() * 1_000_000_000f32) as i32,
    )
}

pub fn var_init_flat_map(
    declarations: Vec<Vec<VarInitDecl>>,
    storage_class: Option<StorageClass>,
) -> Vec<VarInitDecl> {
    let declarations = declarations
        .into_iter()
        .flatten()
        .collect::<Vec<VarInitDecl>>();
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

pub fn to_ids(input: Vec<&str>) -> Vec<Id> {
    input.into_iter().map(|item| Id::from(item)).collect()
}
