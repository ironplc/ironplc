use ironplc_dsl::dsl::*;
use time::Duration;

pub fn to_duration(val: f32, unit_per_sec: f32) -> Duration {
    let secs = val * unit_per_sec;
    Duration::new(
        secs.trunc() as i64,
        (secs.fract() * 1_000_000_000f32) as i32,
    )
}

pub fn var_init_kind_map(declarations: Vec<VarInitDecl>) -> Vec<VarInitKind> {
    declarations
        .into_iter()
        .map(|d| VarInitKind::VarInit(d))
        .collect::<Vec<VarInitKind>>()
}

pub fn located_var_init_kind_map(declarations: Vec<LocatedVarInit>) -> Vec<VarInitKind> {
    declarations
        .into_iter()
        .map(|d| VarInitKind::LocatedVarInit(d))
        .collect::<Vec<VarInitKind>>()
}

pub fn var_init_map(
    declarations: Vec<VarInitDecl>,
    storage_class: Option<StorageClass>,
) -> Vec<VarInitDecl> {
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

pub fn var_init_flat_map(
    declarations: Vec<Vec<VarInitDecl>>,
    storage_class: Option<StorageClass>,
) -> Vec<VarInitDecl> {
    let declarations = declarations.into_iter().flatten().collect::<Vec<VarInitDecl>>();
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
    input.into_iter().map(|item| String::from(item)).collect()
}
