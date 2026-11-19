//! Transformation pass that records, on every variable declaration, the
//! [`TypeId`] of the type it declares.
//!
//! A declaration that names its type (`x : INT`, `p : Point`) takes the id of
//! that name. A declaration that spells its type out in place has no name to
//! look up, so this pass enters that type in the environment as an anonymous
//! type and records the new id:
//!
//! ```ignore
//! VAR
//!     a : ARRAY[1..2] OF DINT;   (* an anonymous array type *)
//!     e : (RED, GREEN);          (* an anonymous enumeration *)
//!     s : INT (0..10);            (* an anonymous subrange *)
//!     r : REF_TO INT;            (* an anonymous reference type *)
//!     t : STRING[8];             (* a string type of capacity 8 *)
//! END_VAR
//! ```
//!
//! Each such declaration declares a type of its own, so two declarations
//! that spell the same shape get two ids (ADR-0055). A sized string's type is
//! named by its elementary type, `STRING` or `WSTRING`, though `id_of` that
//! name is the unsized type.
//!
//! The pass also records the declared type of each field of a structure or
//! function block type in the environment
//! ([`TypeEnvironment::field_type`]): a field is declared as a variable is.
//! A function block's fields -- its `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`
//! and `VAR` variables, in order -- the names of its properties, the
//! signatures of its methods and the block it `EXTENDS` are recorded together
//! ([`TypeEnvironment::record_block`]), and the block's type lists none of
//! them.
//!
//! A declaration whose type cannot be resolved keeps `type_id: None`. The
//! rules that check declarations report why; this pass stays silent.
use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::type_id::TypeId;

use crate::intermediates::{array, enumeration, string, subrange, TypeResolution};
use crate::semantic_type::FunctionBlockVarType;
use crate::type_environment::{BlockField, TypeEnvironment};

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = DeclTypeResolver { type_environment };
    resolver.fold_library(lib).map_err(|e| vec![e])
}

struct DeclTypeResolver<'a> {
    type_environment: &'a mut TypeEnvironment,
}

/// The id of the type `init` declares, entering it in `type_environment` as
/// an anonymous type when it has no name. `name` labels the anonymous type's
/// diagnostics, which are discarded.
fn declared_type_id(
    type_environment: &mut TypeEnvironment,
    name: &TypeName,
    init: &InitialValueAssignmentKind,
) -> Option<TypeId> {
    let env = &*type_environment;
    let anonymous = match init {
        InitialValueAssignmentKind::None(_) => return None,
        InitialValueAssignmentKind::Simple(si) => return env.id_of(&si.type_name),
        // A sized string has a type of its own, which records its capacity;
        // an unsized one is its elementary type.
        InitialValueAssignmentKind::String(si) => {
            let elementary = env.id_of(&si.type_name())?;
            return match si.length {
                Some(_) => Some(type_environment.insert_sized_string(elementary, string::from(si))),
                None => Some(elementary),
            };
        }
        InitialValueAssignmentKind::EnumeratedType(e) => return env.id_of(&e.type_name),
        InitialValueAssignmentKind::FunctionBlock(fb) => return env.id_of(&fb.type_name),
        InitialValueAssignmentKind::Interface(itf) => return env.id_of(&itf.type_name),
        InitialValueAssignmentKind::FunctionBlockCall(fbc) => return env.id_of(&fbc.type_name),
        InitialValueAssignmentKind::Structure(s) => return env.id_of(&s.type_name),
        InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
            type_name, ..
        }) => return env.id_of(type_name),
        InitialValueAssignmentKind::SimpleExpr(se) => return env.id_of(&se.type_name),
        InitialValueAssignmentKind::StringExpr(se) => return env.id_of(&se.type_name()),
        InitialValueAssignmentKind::EnumeratedValues(values) => {
            enumeration::try_from_values(values, None, None).ok()?
        }
        InitialValueAssignmentKind::Subrange(subrange) => {
            match subrange::try_from(name, &subrange.spec, env).ok()? {
                TypeResolution::Type(attributes) => attributes,
                TypeResolution::Alias(alias) => return env.id_of(&alias),
            }
        }
        InitialValueAssignmentKind::Array(a) => match array::try_from(name, &a.spec, env).ok()? {
            TypeResolution::Type(attributes) => {
                let id = type_environment.insert_anonymous(attributes);
                if let SpecificationKind::Inline(elements) = &a.spec {
                    type_environment.record_array_element(id, elements);
                }
                return Some(id);
            }
            TypeResolution::Alias(alias) => return env.id_of(&alias),
        },
        // A reference type is one type however often it is spelled
        // (see `TypeEnvironment::reference_to`).
        InitialValueAssignmentKind::Reference(r) => {
            let target = match &r.target {
                ReferenceTarget::Array(subranges) => {
                    let spec = SpecificationKind::Inline(subranges.clone());
                    match array::try_from(name, &spec, env).ok()? {
                        TypeResolution::Type(attributes) => {
                            let id = type_environment.insert_anonymous(attributes);
                            type_environment.record_array_element(id, subranges);
                            id
                        }
                        TypeResolution::Alias(alias) => env.id_of(&alias)?,
                    }
                }
                // A named type, or a reference spelled in place
                // (`REF_TO REF_TO INT`), which is itself one type.
                ReferenceTarget::Named(_) | ReferenceTarget::Reference(_) => {
                    type_environment.reference_target_id(&r.target)?
                }
            };
            return type_environment.reference_to(target);
        }
    };
    Some(type_environment.insert_anonymous(anonymous))
}

/// The field of a function block's instance that the block's variable
/// `decl` is: a `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` or `VAR`.
fn block_field(decl: &VarDecl) -> Option<BlockField> {
    let var_type = match decl.var_type {
        VariableType::Input => FunctionBlockVarType::Input,
        VariableType::Output => FunctionBlockVarType::Output,
        VariableType::InOut => FunctionBlockVarType::InOut,
        VariableType::Var => FunctionBlockVarType::Internal,
        VariableType::VarTemp
        | VariableType::External
        | VariableType::Global
        | VariableType::Access => return None,
    };
    let address = match &decl.identifier {
        VariableIdentifier::Symbol(_) => None,
        VariableIdentifier::Direct(direct) => Some(direct.address_assignment.clone()),
    };
    Some(BlockField {
        name: decl.identifier.symbolic_id()?.clone(),
        var_type,
        field_type: decl.type_id,
        qualifier: decl.qualifier.clone(),
        address,
    })
}

impl Fold<Diagnostic> for DeclTypeResolver<'_> {
    fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        let name = match node.identifier.symbolic_id() {
            Some(id) => TypeName::from_id(id),
            None => TypeName::from("_"),
        };
        let type_id = declared_type_id(self.type_environment, &name, &node.initializer);
        Ok(VarDecl { type_id, ..node })
    }

    fn fold_structure_declaration(
        &mut self,
        node: StructureDeclaration,
    ) -> Result<StructureDeclaration, Diagnostic> {
        // A repeated type name keeps its first declaration (P2007): the
        // environment records a structure once, and nothing on a type of
        // another kind.
        if let Some(structure) = self.type_environment.id_of(&node.type_name) {
            let fields: Vec<(Id, TypeId)> = node
                .elements
                .iter()
                .filter_map(|element| {
                    let name = TypeName::from_id(&element.name);
                    let field_type = declared_type_id(self.type_environment, &name, &element.init)?;
                    Some((element.name.clone(), field_type))
                })
                .collect();
            self.type_environment.record_structure(structure, fields);
        }
        node.recurse_fold(self)
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        let node = node.recurse_fold(self)?;
        // A repeated block name keeps its first declaration (P4013), and a
        // block the library redeclares over a standard one keeps the
        // standard block: the environment records a block once.
        if let Some(function_block) = self.type_environment.id_of(&node.name) {
            let fields = node.variables.iter().filter_map(block_field).collect();
            let base = node
                .oop
                .as_ref()
                .and_then(|oop| oop.base.as_ref())
                .and_then(|base| self.type_environment.id_of(base));
            let properties = node
                .properties
                .iter()
                .map(|property| self.type_environment.property_signature(property))
                .collect();
            let methods = node
                .methods
                .iter()
                .map(|method| self.type_environment.method_signature(method))
                .collect();
            self.type_environment
                .record_block(function_block, fields, properties, methods, base);
        }
        Ok(node)
    }

    fn fold_interface_declaration(
        &mut self,
        node: InterfaceDeclaration,
    ) -> Result<InterfaceDeclaration, Diagnostic> {
        let node = node.recurse_fold(self)?;
        // A repeated interface name keeps its first declaration: the
        // environment records an interface once.
        if let Some(interface) = self.type_environment.id_of(&TypeName::from_id(&node.name)) {
            let methods = node
                .methods
                .iter()
                .map(|method| self.type_environment.prototype_signature(method))
                .collect();
            let properties = node
                .properties
                .iter()
                .map(|property| self.type_environment.property_prototype_signature(property))
                .collect();
            self.type_environment
                .record_interface(interface, methods, properties);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_context::SemanticContext;
    use crate::semantic_type::SemanticType;
    use crate::test_helpers::{declared_ids, parse_and_resolve_types_with_options};
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// Resolves `program` under edition 3, which has `REF_TO`.
    fn resolve(program: &str) -> (Library, SemanticContext) {
        parse_and_resolve_types_with_options(
            program,
            &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        )
    }

    const PROGRAM: &str = "
TYPE
  POINT : STRUCT x : DINT; END_STRUCT;
  ARR : ARRAY[1..2] OF DINT;
END_TYPE
PROGRAM main
VAR
  n : DINT;
  p : POINT;
  na : ARR;
  a : ARRAY[1..2] OF DINT;
  b : ARRAY[1..2] OF DINT;
  e : (RED, GREEN);
  r : REF_TO INT;
  t : STRING[8];
  u : STRING;
END_VAR
END_PROGRAM
";

    #[test]
    fn apply_when_named_type_then_id_of_name() {
        let (library, context) = resolve(PROGRAM);
        let ids = declared_ids(&library);
        let types = context.types();

        assert_eq!(ids["n"], types.id_of(&TypeName::from("DINT")));
        assert_eq!(ids["p"], types.id_of(&TypeName::from("POINT")));
        assert_eq!(ids["u"], types.id_of(&TypeName::from("STRING")));
    }

    /// REQ-LOW-analyzer-053: a sized string has a type of its own, which
    /// records its capacity and is named by its elementary type.
    #[test]
    fn apply_when_sized_string_then_type_of_its_capacity_named_by_elementary() {
        let (library, context) = resolve(PROGRAM);
        let id = declared_ids(&library)["t"].unwrap();
        let types = context.types();

        assert_ne!(Some(id), types.id_of(&TypeName::from("STRING")));
        assert_eq!(types.name_of(id), Some(&TypeName::from("STRING")));
        assert_eq!(
            types.get_by_id(id).unwrap().representation,
            SemanticType::String {
                max_len: Some(8),
                char_width: ironplc_container::CharWidth::Narrow,
            }
        );
    }

    /// REQ-LOW-analyzer-053: a sized string field of a structure or a
    /// function block has a type of its capacity, as a variable does.
    #[test]
    fn apply_when_sized_string_field_then_field_type_of_its_capacity() {
        let (_, context) = resolve(
            "TYPE Named : STRUCT n : WSTRING[4]; END_STRUCT; END_TYPE
             FUNCTION_BLOCK Block VAR_OUTPUT f : STRING[6]; END_VAR END_FUNCTION_BLOCK
             PROGRAM main END_PROGRAM",
        );
        let types = context.types();
        let capacity = |record: &str, field: &str| -> Option<u128> {
            let record = types.id_of(&TypeName::from(record))?;
            let field_type = types.field_type(record, &Id::from(field))?;
            let SemanticType::String { max_len, .. } = types.get_by_id(field_type)?.representation
            else {
                return None;
            };
            max_len
        };

        assert_eq!(capacity("Named", "n"), Some(4));
        assert_eq!(capacity("Block", "f"), Some(6));
    }

    /// REQ-LOW-analyzer-053: a sized string element of an array, inline or
    /// of a named array type, has a type of its capacity, named by its
    /// elementary type, as a variable does.
    #[test]
    fn apply_when_array_of_sized_string_then_element_type_of_its_capacity() {
        let (library, context) = resolve(
            "TYPE Names : ARRAY[1..2] OF WSTRING[4]; END_TYPE
             PROGRAM main VAR a : ARRAY[1..3] OF STRING[8]; n : Names; END_VAR END_PROGRAM",
        );
        let ids = declared_ids(&library);
        let types = context.types();
        let element = |array: TypeId| {
            let element = types.element_type(array).unwrap();
            let name = types.name_of(element).cloned();
            (
                name,
                types.get_by_id(element).unwrap().representation.clone(),
            )
        };

        assert_eq!(
            element(ids["a"].unwrap()),
            (
                Some(TypeName::from("STRING")),
                SemanticType::String {
                    max_len: Some(8),
                    char_width: ironplc_container::CharWidth::Narrow,
                }
            )
        );
        assert_eq!(
            element(ids["n"].unwrap()),
            (
                Some(TypeName::from("WSTRING")),
                SemanticType::String {
                    max_len: Some(4),
                    char_width: ironplc_container::CharWidth::Wide,
                }
            )
        );
    }

    #[test]
    fn apply_when_inline_array_then_anonymous_array_with_dimensions() {
        let (library, context) = resolve(PROGRAM);
        let id = declared_ids(&library)["a"].unwrap();

        assert_eq!(context.types().name_of(id), None);
        match &context.types().get_by_id(id).unwrap().representation {
            SemanticType::Array { dimensions, .. } => assert_eq!(dimensions.len(), 1),
            other => panic!("expected an array, got {other:?}"),
        }
    }

    #[test]
    fn apply_when_same_shape_twice_then_two_ids() {
        let (library, _) = resolve(PROGRAM);
        let ids = declared_ids(&library);

        assert_ne!(ids["a"].unwrap(), ids["b"].unwrap());
    }

    #[rstest::rstest]
    #[case::enumeration("e")]
    #[case::reference("r")]
    fn apply_when_inline_type_then_anonymous_id(#[case] variable: &str) {
        let (library, context) = resolve(PROGRAM);
        let id = declared_ids(&library)[variable].unwrap();

        assert_eq!(context.types().name_of(id), None);
        assert!(context.types().get_by_id(id).is_some());
    }

    #[test]
    fn apply_when_named_array_then_id_of_array_type() {
        let (library, context) = resolve(PROGRAM);

        assert_eq!(
            declared_ids(&library)["na"],
            context.types().id_of(&TypeName::from("ARR"))
        );
    }

    /// An inline subrange cannot be written in a declaration, but the
    /// initializer can hold one, so the pass has an answer for it.
    #[test]
    fn declared_type_id_when_inline_subrange_then_anonymous_subrange() {
        let (_, mut context) = resolve(PROGRAM);
        let bound = |value: &str| {
            SignedIntegerRef::Literal(
                SignedInteger::new(value, ironplc_dsl::core::SourceSpan::default()).unwrap(),
            )
        };
        let init = InitialValueAssignmentKind::Subrange(SubrangeInitialValueAssignment::bare(
            SpecificationKind::Inline(SubrangeSpecification {
                type_name: ElementaryTypeName::INT,
                subrange: Subrange {
                    start: bound("0"),
                    end: bound("10"),
                },
            }),
        ));
        let types = context.types_mut();

        let id = declared_type_id(types, &TypeName::from("s"), &init).unwrap();

        assert_eq!(types.name_of(id), None);
        assert!(types.get_by_id(id).unwrap().representation.is_subrange());
    }

    const ARRAYS: &str = "
TYPE
  POINT : STRUCT x : DINT; END_STRUCT;
  POINTS : ARRAY[1..2] OF POINT;
  MORE_POINTS : POINTS;
END_TYPE
PROGRAM main
VAR
  inline_points : ARRAY[1..2] OF POINT;
  named_points : POINTS;
  alias_points : MORE_POINTS;
  ints : ARRAY[1..2] OF INT;
  refs : ARRAY[1..2] OF REF_TO INT;
  ref_points : REF_TO ARRAY[1..2] OF POINT;
END_VAR
END_PROGRAM
";

    /// The element type of the array variable `name` declares.
    fn element_of(variable: &str) -> (Option<TypeId>, SemanticContext) {
        let (library, context) = resolve(ARRAYS);
        let array = declared_ids(&library)[variable].unwrap();
        (context.types().element_type(array), context)
    }

    #[rstest::rstest]
    #[case::inline("inline_points")]
    #[case::named("named_points")]
    #[case::alias("alias_points")]
    fn element_type_when_array_of_structure_then_structure(#[case] variable: &str) {
        let (element, context) = element_of(variable);
        assert_eq!(element, context.types().id_of(&TypeName::from("POINT")));
    }

    #[test]
    fn element_type_when_array_of_elementary_then_elementary() {
        let (element, context) = element_of("ints");
        assert_eq!(element, context.types().id_of(&TypeName::from("INT")));
    }

    #[test]
    fn element_type_when_array_of_references_then_reference_to_element() {
        let (element, context) = element_of("refs");
        let types = context.types();
        assert_eq!(
            types.referenced_type(element.unwrap()),
            types.id_of(&TypeName::from("INT"))
        );
    }

    #[test]
    fn element_type_when_reference_to_array_then_array_element() {
        let (library, context) = resolve(ARRAYS);
        let types = context.types();
        let reference = declared_ids(&library)["ref_points"].unwrap();
        let array = types.referenced_type(reference).unwrap();
        assert_eq!(
            types.element_type(array),
            types.id_of(&TypeName::from("POINT"))
        );
    }

    #[test]
    fn element_type_when_not_array_then_none() {
        let (_library, context) = resolve(ARRAYS);
        let types = context.types();
        let point = types.id_of(&TypeName::from("POINT")).unwrap();
        assert_eq!(types.element_type(point), None);
    }

    /// REQ-ETR-analyzer-002: the type environment records every variable an
    /// instance of a block has, whatever its type, in declaration order,
    /// each with its section and the type its declaration records; the
    /// block's type lists none of them.
    #[spec_test(REQ_ETR_analyzer_002)]
    #[test]
    fn apply_when_function_block_then_environment_records_every_variable_in_order() {
        let (library, context) = resolve(
            "TYPE Point : STRUCT x : DINT; END_STRUCT; END_TYPE
             FUNCTION_BLOCK Block
             VAR_INPUT n : INT; s : STRING[8]; END_VAR
             VAR_OUTPUT p : Point; END_VAR
             VAR_IN_OUT io : DINT; END_VAR
             VAR a : ARRAY[1..3] OF INT; r : REF_TO INT; END_VAR
             VAR_TEMP t : INT; END_VAR
             END_FUNCTION_BLOCK
             PROGRAM main END_PROGRAM",
        );
        let declared = declared_ids(&library);
        let types = context.types();
        let block = types.id_of(&TypeName::from("Block")).unwrap();

        let recorded: Vec<(String, FunctionBlockVarType, Option<TypeId>)> = types
            .block_fields(block)
            .unwrap()
            .iter()
            .map(|field| (field.name.to_string(), field.var_type, field.field_type))
            .collect();
        let expected = |name: &str, var_type| (name.to_string(), var_type, declared[name]);
        assert_eq!(
            recorded,
            vec![
                expected("n", FunctionBlockVarType::Input),
                expected("s", FunctionBlockVarType::Input),
                expected("p", FunctionBlockVarType::Output),
                expected("io", FunctionBlockVarType::InOut),
                expected("a", FunctionBlockVarType::Internal),
                expected("r", FunctionBlockVarType::Internal),
            ]
        );
        assert_eq!(
            types.get_by_id(block).unwrap().representation,
            SemanticType::FunctionBlock {
                name: "Block".to_string()
            }
        );
    }

    /// REQ-ETR-analyzer-003: a field a block inherits through `EXTENDS`,
    /// from its base or a base of its base, has the type the block that
    /// declares it gives it.
    #[spec_test(REQ_ETR_analyzer_003)]
    #[test]
    fn apply_when_function_block_extends_then_field_type_of_inherited_field() {
        let (_, context) = resolve(
            "FUNCTION_BLOCK Base VAR_INPUT count : INT; END_VAR END_FUNCTION_BLOCK
             FUNCTION_BLOCK Middle EXTENDS Base VAR label : STRING; END_VAR END_FUNCTION_BLOCK
             FUNCTION_BLOCK Derived EXTENDS Middle VAR speed : LREAL; END_VAR END_FUNCTION_BLOCK
             PROGRAM main END_PROGRAM",
        );
        let types = context.types();
        let derived = types.id_of(&TypeName::from("Derived")).unwrap();
        let field = |name: &str| types.field_type(derived, &Id::from(name));

        assert_eq!(field("count"), types.id_of(&TypeName::from("INT")));
        assert_eq!(field("label"), types.id_of(&TypeName::from("STRING")));
        assert_eq!(field("speed"), types.id_of(&TypeName::from("LREAL")));
        assert_eq!(field("missing"), None);
    }

    /// The type of the field `field` of the block `block` in `program`.
    fn block_field_type(program: &str, block: &str, field: &str) -> Option<TypeId> {
        let (_, context) = resolve(program);
        let types = context.types();
        types.field_type(types.id_of(&TypeName::from(block))?, &Id::from(field))
    }

    /// Only a function block the library declares is a base whose fields a
    /// block inherits: not a structure, which the program is rejected for,
    /// and not a standard block, whose fields neither the symbol environment
    /// nor code generation inherits.
    #[spec_test(REQ_ETR_analyzer_003)]
    #[rstest]
    #[case::structure(
        "TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE
         FUNCTION_BLOCK Derived EXTENDS Pair END_FUNCTION_BLOCK",
        "f"
    )]
    #[case::standard_block("FUNCTION_BLOCK Derived EXTENDS TON END_FUNCTION_BLOCK", "Q")]
    fn apply_when_base_not_declared_block_then_no_field_inherited(
        #[case] declarations: &str,
        #[case] field: &str,
    ) {
        let program = format!("{declarations}\nPROGRAM main END_PROGRAM");
        assert_eq!(block_field_type(&program, "Derived", field), None);
    }

    /// A repeated block name keeps its first declaration (P4013): its
    /// fields, the fields its type lists and the block it extends.
    #[test]
    fn apply_when_block_name_repeated_then_first_declaration_kept() {
        let program = "
FUNCTION_BLOCK First VAR_INPUT a : INT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Second VAR_INPUT b : INT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS First VAR x : INT; END_VAR END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Second VAR y : LREAL; END_VAR END_FUNCTION_BLOCK
PROGRAM main END_PROGRAM";
        let (_, context) = resolve(program);
        let types = context.types();
        let derived = types.id_of(&TypeName::from("Derived")).unwrap();
        let field = |name: &str| types.field_type(derived, &Id::from(name));
        let listed: Vec<String> = types
            .block_fields(derived)
            .unwrap()
            .iter()
            .map(|field| field.name.to_string())
            .collect();

        assert_eq!(field("a"), types.id_of(&TypeName::from("INT")));
        assert_eq!(field("x"), types.id_of(&TypeName::from("INT")));
        assert_eq!(field("b"), None);
        assert_eq!(field("y"), None);
        assert_eq!(listed, vec!["x".to_string()]);
    }

    /// REQ-ETR-analyzer-006: a repeated type name keeps its first
    /// declaration (P2007), and a structure's recorded fields are that
    /// declaration's, as its type's are: a later structure of the name adds
    /// no field and changes no field's type.
    #[spec_test(REQ_ETR_analyzer_006)]
    #[rstest]
    #[case::structure_repeated(
        "TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE
         TYPE Pair : STRUCT f : STRING; g : BOOL; END_STRUCT; END_TYPE",
        Some("INT")
    )]
    #[case::type_then_structure(
        "TYPE Pair : INT; END_TYPE
         TYPE Pair : STRUCT f : STRING; g : BOOL; END_STRUCT; END_TYPE",
        None
    )]
    fn apply_when_type_name_repeated_then_fields_of_first_declaration(
        #[case] declarations: &str,
        #[case] field_f: Option<&str>,
    ) {
        let (_, context) = resolve(&format!("{declarations}\nPROGRAM main END_PROGRAM"));
        let types = context.types();
        let pair = types.id_of(&TypeName::from("Pair")).unwrap();
        let listed: Vec<String> = types
            .get_by_id(pair)
            .unwrap()
            .representation
            .structure_fields()
            .map(|fields| fields.iter().map(|field| field.name.to_string()).collect())
            .unwrap_or_default();

        assert_eq!(
            types.field_type(pair, &Id::from("f")),
            field_f.and_then(|name| types.id_of(&TypeName::from(name)))
        );
        assert_eq!(types.field_type(pair, &Id::from("g")), None);
        let expected: &[&str] = if field_f.is_some() { &["f"] } else { &[] };
        assert_eq!(listed, expected);
    }

    /// A library that declares a block of a standard block's name leaves the
    /// standard block's fields as they are.
    #[test]
    fn apply_when_standard_block_redeclared_then_standard_fields_kept() {
        let program = "
FUNCTION_BLOCK TON VAR_INPUT extra : LREAL; END_VAR END_FUNCTION_BLOCK
PROGRAM main END_PROGRAM";
        let (_, context) = resolve(program);
        let types = context.types();
        let ton = types.id_of(&TypeName::from("TON")).unwrap();
        let listed: Vec<String> = types
            .block_fields(ton)
            .unwrap()
            .iter()
            .map(|field| field.name.to_string())
            .collect();

        assert_eq!(block_field_type(program, "TON", "extra"), None);
        assert_eq!(listed, vec!["IN", "PT", "Q", "ET"]);
    }
}
