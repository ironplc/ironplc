use ironplc_dsl::core::Id;
use phf::{phf_set, Set};

static STANDARD_LIBRARY_TYPES_LOWER_CASE: Set<&'static str> = phf_set! {
    "ctd", // 2.5.2.3.3
    "ctd_dint", // 2.5.2.3.3
    "ctd_lint", // 2.5.2.3.3
    "ctd_udint", // 2.5.2.3.3
    "ctd_ulint", // 2.5.2.3.3
    "ctu", // 2.5.2.3.3
    "ctu_dint", // 2.5.2.3.3
    "ctu_lint", // 2.5.2.3.3
    "ctu_udint", // 2.5.2.3.3
    "ctu_ulint", // 2.5.2.3.3
    "ctud", // 2.5.2.3.3
    "ctud_dint", // 2.5.2.3.3
    "ctud_lint", // 2.5.2.3.3
    "ctud_ulint", // 2.5.2.3.3
    "f_trig", // 2.5.2.3.2
    "r_trig", // 2.5.2.3.2
    "rs", // 2.5.2.3.1
    "sr", // 2.5.2.3.1
    "ton", // 2.5.2.3.4
    "tof", // 2.5.2.3.4
    "tp", // 2.5.2.3.4
    // TODO there is more in IEC 61131-5
};

pub(crate) fn is_unsupported_standard_type(id: &Id) -> bool {
    STANDARD_LIBRARY_TYPES_LOWER_CASE.contains(&id.lower_case().to_string())
}

static ELEMENTARY_TYPES_LOWER_CASE: Set<&'static str> = phf_set! {
    // signed_integer_type_name
    "sint",
    "int",
    "dint",
    "lint",
    // unsigned_integer_type_name
    "usint",
    "uint",
    "udint",
    "ulint",
    // real_type_name
    "real",
    "lreal",
    // date_type_name
    "date",
    "time_of_day",
    "tod",
    "date_and_time",
    "dt",
    // bit_string_type_name
    "bool",
    "byte",
    "word",
    "dword",
    "lword",
    // remaining elementary_type_name
    "string",
    "wstring",
    "time"
};

pub(crate) fn is_elementary_type(id: &Id) -> bool {
    ELEMENTARY_TYPES_LOWER_CASE.contains(&id.lower_case().to_string())
}
