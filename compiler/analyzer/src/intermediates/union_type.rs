//! Union type processing
//!
//! This module handles creating union types from union declarations.
//! Unlike structures where fields have sequential offsets, all union
//! fields share the same memory location (offset 0).

use crate::intermediate_type::{IntermediateStructField, IntermediateType};
use crate::intermediates::structure;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::*;

/// Try to create the intermediate type information from the union specification.
pub fn try_from(
    node_name: &TypeName,
    spec: &UnionDeclaration,
    type_environment: &TypeEnvironment,
) -> Result<TypeAttributes, Diagnostic> {
    // Resolve field types - all at offset 0 (overlapping memory)
    let mut fields = Vec::new();

    for element in &spec.elements {
        // Reuse structure's field type resolution
        let field_type = structure::resolve_field_type_for_union(&element.init, type_environment)?;

        let field = IntermediateStructField {
            name: element.name.clone(),
            field_type,
            offset: 0, // All union fields share the same memory location
            var_type: None,
            has_default: false,
        };

        fields.push(field);
    }

    Ok(TypeAttributes::new(
        node_name.span(),
        IntermediateType::Union { fields },
    ))
}

#[cfg(test)]
mod tests {
    use crate::intermediate_type::IntermediateType;
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};
    use crate::xform_resolve_type_decl_environment::apply;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::{FileId, Id};
    use ironplc_parser::options::ParseOptions;

    /// Helper function to parse a program and apply type resolution
    fn parse_and_apply(program: &str) -> TypeEnvironment {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);
        assert!(result.is_ok(), "Expected Ok, got error: {:?}", result.err());
        env
    }

    #[test]
    fn parse_simple_union_then_creates_union_type() {
        let program = "
TYPE
    IpAdrUnion :
    UNION
        ipadrIPStack : UDINT;
        ipadr : ARRAY[0..3] OF BYTE;
    END_UNION;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let union_type = env.get(&TypeName::from("IpAdrUnion")).unwrap();
        assert!(union_type.representation.is_union());

        let fields = match &union_type.representation {
            IntermediateType::Union { fields } => fields,
            _ => panic!("Expected Union type, got {:?}", union_type.representation),
        };

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, Id::from("ipadrIPStack"));
        assert_eq!(fields[1].name, Id::from("ipadr"));

        // All union fields should be at offset 0
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 0);
    }

    #[test]
    fn parse_union_with_different_sized_fields_then_all_at_offset_zero() {
        let program = "
TYPE
    MyUnion :
    UNION
        byte_val : BYTE;
        int_val : INT;
        dint_val : DINT;
    END_UNION;
END_TYPE
        ";
        let env = parse_and_apply(program);

        let union_type = env.get(&TypeName::from("MyUnion")).unwrap();
        assert!(union_type.representation.is_union());

        let fields = match &union_type.representation {
            IntermediateType::Union { fields } => fields,
            _ => panic!("Expected Union type, got {:?}", union_type.representation),
        };

        assert_eq!(fields.len(), 3);
        // All fields at offset 0
        for field in fields {
            assert_eq!(field.offset, 0);
        }
    }
}
