//! How the type environment numbers types.
//!
//! An elementary type's [`TypeId`] is its debug-section `iec_type_tag`
//! (ADR-0019), so the analyzer, codegen and a debugger number built-in types
//! the same way. Ids from the end of the tag range up to [`FIRST_ALLOCATED`]
//! are never used, so no id can be mistaken for an aggregate tag (`STRUCT`,
//! `ARRAY`, `FB_INSTANCE`) or for `OTHER`. Every other type gets an id from
//! [`FIRST_ALLOCATED`] up, in the order the environment meets it. See
//! ADR-0055.

use ironplc_container::debug_section::iec_type_tag;
use ironplc_dsl::common::ElementaryTypeName;
use ironplc_dsl::type_id::TypeId;

/// The first id the environment allocates to a type that is not elementary.
pub(crate) const FIRST_ALLOCATED: u32 = 256;

/// The id of an elementary type: its debug type tag.
pub(crate) fn elementary(elementary: &ElementaryTypeName) -> TypeId {
    let tag = match elementary {
        ElementaryTypeName::BOOL => iec_type_tag::BOOL,
        ElementaryTypeName::SINT => iec_type_tag::SINT,
        ElementaryTypeName::INT => iec_type_tag::INT,
        ElementaryTypeName::DINT => iec_type_tag::DINT,
        ElementaryTypeName::LINT => iec_type_tag::LINT,
        ElementaryTypeName::USINT => iec_type_tag::USINT,
        ElementaryTypeName::UINT => iec_type_tag::UINT,
        ElementaryTypeName::UDINT => iec_type_tag::UDINT,
        ElementaryTypeName::ULINT => iec_type_tag::ULINT,
        ElementaryTypeName::REAL => iec_type_tag::REAL,
        ElementaryTypeName::LREAL => iec_type_tag::LREAL,
        ElementaryTypeName::BYTE => iec_type_tag::BYTE,
        ElementaryTypeName::WORD => iec_type_tag::WORD,
        ElementaryTypeName::DWORD => iec_type_tag::DWORD,
        ElementaryTypeName::LWORD => iec_type_tag::LWORD,
        ElementaryTypeName::STRING => iec_type_tag::STRING,
        ElementaryTypeName::WSTRING => iec_type_tag::WSTRING,
        ElementaryTypeName::TIME => iec_type_tag::TIME,
        ElementaryTypeName::LTIME => iec_type_tag::LTIME,
        ElementaryTypeName::DATE => iec_type_tag::DATE,
        ElementaryTypeName::LDATE => iec_type_tag::LDATE,
        ElementaryTypeName::TimeOfDay => iec_type_tag::TIME_OF_DAY,
        ElementaryTypeName::LTimeOfDay => iec_type_tag::LTOD,
        ElementaryTypeName::DateAndTime => iec_type_tag::DATE_AND_TIME,
        ElementaryTypeName::LDateAndTime => iec_type_tag::LDT,
    };
    TypeId::from_raw(tag.into())
}

/// The debug type tag of the type `id` identifies when that type is
/// elementary, else `None`: a debugger learns nothing more precise about
/// any other type from its tag.
pub fn elementary_debug_tag(id: TypeId) -> Option<u8> {
    u8::try_from(id.raw())
        .ok()
        .filter(|tag| *tag <= iec_type_tag::LDT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::IntermediateType;
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::SourceSpan;
    use std::collections::HashSet;

    fn elementary_environment() -> TypeEnvironment {
        TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap()
    }

    const ALL_ELEMENTARY: [ElementaryTypeName; 25] = [
        ElementaryTypeName::BOOL,
        ElementaryTypeName::SINT,
        ElementaryTypeName::INT,
        ElementaryTypeName::DINT,
        ElementaryTypeName::LINT,
        ElementaryTypeName::USINT,
        ElementaryTypeName::UINT,
        ElementaryTypeName::UDINT,
        ElementaryTypeName::ULINT,
        ElementaryTypeName::REAL,
        ElementaryTypeName::LREAL,
        ElementaryTypeName::TIME,
        ElementaryTypeName::LTIME,
        ElementaryTypeName::DATE,
        ElementaryTypeName::LDATE,
        ElementaryTypeName::TimeOfDay,
        ElementaryTypeName::LTimeOfDay,
        ElementaryTypeName::DateAndTime,
        ElementaryTypeName::LDateAndTime,
        ElementaryTypeName::STRING,
        ElementaryTypeName::BYTE,
        ElementaryTypeName::WORD,
        ElementaryTypeName::DWORD,
        ElementaryTypeName::LWORD,
        ElementaryTypeName::WSTRING,
    ];

    #[test]
    fn elementary_when_every_type_then_distinct_ids_that_are_debug_tags() {
        let ids: HashSet<TypeId> = ALL_ELEMENTARY.iter().map(elementary).collect();

        assert_eq!(ids.len(), ALL_ELEMENTARY.len());
        for id in ids {
            assert!(elementary_debug_tag(id).is_some(), "{id:?}");
        }
    }

    #[test]
    fn elementary_when_dint_then_dint_debug_tag() {
        assert_eq!(
            elementary_debug_tag(elementary(&ElementaryTypeName::DINT)),
            Some(iec_type_tag::DINT)
        );
    }

    #[test]
    fn elementary_debug_tag_when_allocated_id_then_none() {
        assert_eq!(
            elementary_debug_tag(TypeId::from_raw(FIRST_ALLOCATED)),
            None
        );
    }

    #[test]
    fn elementary_debug_tag_when_reserved_id_then_none() {
        assert_eq!(
            elementary_debug_tag(TypeId::from_raw(iec_type_tag::ARRAY.into())),
            None
        );
        assert_eq!(
            elementary_debug_tag(TypeId::from_raw(iec_type_tag::OTHER.into())),
            None
        );
    }

    #[test]
    fn environment_when_elementary_type_then_id_is_debug_tag() {
        let env = elementary_environment();

        let id = env.id_of(&TypeName::from("DINT")).unwrap();

        assert_eq!(elementary_debug_tag(id), Some(iec_type_tag::DINT));
    }

    #[test]
    fn environment_when_elementary_spellings_then_one_id_named_by_first() {
        let env = elementary_environment();

        let long = env.id_of(&TypeName::from("TIME_OF_DAY")).unwrap();
        let short = env.id_of(&TypeName::from("TOD")).unwrap();

        assert_eq!(long, short);
        assert_eq!(env.name_of(long), Some(&TypeName::from("time_of_day")));
    }

    #[test]
    fn environment_when_user_type_then_allocated_id_with_name() {
        let mut env = elementary_environment();
        let name = TypeName::from("POINT");
        env.insert_type(
            &name,
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Structure { fields: vec![] },
            ),
        );

        let id = env.id_of(&name).unwrap();

        assert!(id.raw() >= FIRST_ALLOCATED);
        assert_eq!(elementary_debug_tag(id), None);
        assert_eq!(env.name_of(id), Some(&name));
        assert_eq!(
            env.get_by_id(id).unwrap().representation,
            IntermediateType::Structure { fields: vec![] }
        );
    }

    #[test]
    fn environment_when_alias_then_own_id_and_name() {
        let mut env = elementary_environment();
        let alias = TypeName::from("MY_BYTE");
        env.insert_alias(&alias, &TypeName::from("BYTE")).unwrap();

        let alias_id = env.id_of(&alias).unwrap();

        assert_ne!(Some(alias_id), env.id_of(&TypeName::from("BYTE")));
        assert_eq!(env.name_of(alias_id), Some(&alias));
    }

    #[test]
    fn environment_when_id_not_allocated_then_no_name_or_type() {
        let env = elementary_environment();
        let unknown = TypeId::from_raw(FIRST_ALLOCATED + 1000);

        assert_eq!(env.name_of(unknown), None);
        assert!(env.get_by_id(unknown).is_none());
    }
}
