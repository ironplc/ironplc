use crate::{intermediate_type::IntermediateType, type_environment::TypeAttributes};

use ironplc_dsl::common::StringInitializer;
use ironplc_dsl::core::Located;

pub fn from(initializer: &StringInitializer) -> TypeAttributes {
    // String type with specific length: MY_STRING : STRING(10);
    TypeAttributes {
        span: initializer.span(),
        representation: IntermediateType::String {
            max_len: initializer.length.as_ref().map(|len| len.value),
        },
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::ParseOptions;

    use crate::{
        type_environment::TypeEnvironmentBuilder, xform_resolve_type_decl_environment::apply,
    };

    #[test]
    fn apply_when_string_type_declaration_then_creates_string_type() {
        let program = "
TYPE
MY_STRING : STRING(50) := 'hello';
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the string type was created

        // TODO
        //let my_string_type = env.get(&TypeName::from("MY_STRING")).unwrap();
        //let max_len = assert_string!(&my_string_type.representation);
        //assert_eq!(max_len, &Some(50));
    }
}
