//! The fields of structures and function blocks, by the id of their type.
//!
//! A structure's type lists its fields, each with its representation, so its
//! fields are recorded here only by the id of the type each is declared
//! with, which the representation does not keep. They are recorded once,
//! from the declaration whose type the environment keeps (see
//! [`TypeEnvironment::record_structure`]), so the two cannot come from two
//! declarations.
//!
//! A function block's type (`SemanticType::FunctionBlock`) lists none of its
//! fields: they are recorded here and nowhere else. A block's fields are its
//! variables, whose types are resolved only after the block's type is
//! entered, and by then its type has been copied into the types that use it,
//! such as an array of the block. A field list in the type would be in every
//! copy, as the copy was taken, so copies taken at different times would
//! differ. A type's representation never changes once it is entered; the
//! fields of a block are looked up by the block's id.
//!
//! A block's fields, the signatures of its properties and of its methods,
//! and the block it `EXTENDS` are recorded together, once (see
//! [`TypeEnvironment::record_block`]), so they cannot come from two
//! declarations. Everything that follows `EXTENDS` after the environment is
//! built follows the base recorded here.

use ironplc_dsl::common::{AddressAssignment, DeclarationQualifier, TypeName};
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

use super::methods::MethodSignature;
use super::properties::PropertySignature;
use super::TypeEnvironment;
use crate::intermediates::stdlib_function_block::StandardField;
use crate::semantic_type::FunctionBlockVarType;

/// A field of a function block instance: one of the block's `VAR_INPUT`,
/// `VAR_OUTPUT`, `VAR_IN_OUT` and `VAR` variables.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockField {
    /// The variable's name, as the block declares it.
    pub name: Id,
    /// Which section declares the variable.
    pub var_type: FunctionBlockVarType,
    /// The id of the type the variable is declared with; `None` when it did
    /// not resolve, which a rule reports. The variable is a field all the
    /// same, so that a call still names it.
    pub field_type: Option<TypeId>,
    /// The qualifier the variable is declared with, such as `RETAIN`;
    /// `Unspecified` for a field of a standard block.
    pub qualifier: DeclarationQualifier,
    /// The direct address the variable is declared at (`AT %I*`), if any.
    pub address: Option<AddressAssignment>,
}

/// What the environment records about a function block.
#[derive(Debug)]
pub(super) struct BlockRecord {
    /// The fields the block declares, in the order it declares them.
    fields: Vec<BlockField>,
    /// The signatures of the properties the block declares, in the order it
    /// declares them. A selection on an instance may name one as it names a
    /// field; property access is not implemented.
    properties: Vec<PropertySignature>,
    /// The signatures of the methods the block declares, in the order it
    /// declares them; not those it inherits.
    methods: Vec<MethodSignature>,
    /// The block it `EXTENDS`, whose fields, properties and methods an
    /// instance of it also has.
    base: Option<TypeId>,
}

impl TypeEnvironment {
    /// Records the id of the type each field of the structure `structure` is
    /// declared with, by the field's name. A field whose type did not
    /// resolve is left out; a rule reports it.
    ///
    /// A structure is recorded once, as a block is, so its fields are those
    /// of the declaration whose type the environment keeps: the first of a
    /// repeated name (P2007). A later declaration of the name records
    /// nothing, whatever it declares, and nothing is recorded for a type
    /// that is not a structure, such as a `TYPE Pair : INT` declared ahead
    /// of a structure `Pair`.
    pub(crate) fn record_structure(
        &mut self,
        structure: TypeId,
        fields: impl IntoIterator<Item = (Id, TypeId)>,
    ) {
        let is_structure = self
            .get_by_id(structure)
            .is_some_and(|attributes| attributes.representation.is_structure());
        if !is_structure || self.structures.contains_key(&structure) {
            return;
        }
        self.structures
            .insert(structure, fields.into_iter().collect());
    }

    /// Records the fields of the function block `block`, in the order it
    /// declares them, the signatures of its properties and of its methods,
    /// and the block it `EXTENDS`.
    ///
    /// A block is recorded once: a later call for the same block, such as
    /// for a repeated declaration of its name (P4013) or a library block
    /// named like a standard one, records nothing. Nothing is recorded for a
    /// type that is not a function block.
    ///
    /// Only a function block the library declares is recorded as a base. A
    /// structure has no fields a block inherits, and the program is
    /// rejected. A standard block such as `TON` is not a base the symbol
    /// environment or code generation inherits from, so typing its fields
    /// here would let a program pass analysis and fail later. A block that
    /// names itself is not recorded as its own base.
    pub(crate) fn record_block(
        &mut self,
        block: TypeId,
        fields: Vec<BlockField>,
        properties: Vec<PropertySignature>,
        methods: Vec<MethodSignature>,
        base: Option<TypeId>,
    ) {
        let is_block = self
            .get_by_id(block)
            .is_some_and(|attributes| attributes.representation.is_function_block());
        if !is_block || self.blocks.contains_key(&block) {
            return;
        }
        let base = base.filter(|base| *base != block && self.is_declared_block(*base));
        self.blocks.insert(
            block,
            BlockRecord {
                fields,
                properties,
                methods,
                base,
            },
        );
    }

    /// Records the fields of the standard function block `name`, each of
    /// which has an elementary type. A standard block has no properties and
    /// no methods.
    pub(super) fn record_standard_block(&mut self, name: &TypeName, fields: &[StandardField]) {
        let Some(block) = self.id_of(name) else {
            return;
        };
        let fields = fields
            .iter()
            .filter_map(|field| {
                let elementary = self.elementary_type_name_for(&field.field_type)?;
                Some(BlockField {
                    name: field.name.clone(),
                    var_type: field.var_type,
                    field_type: Some(self.id_of(&elementary)?),
                    qualifier: DeclarationQualifier::Unspecified,
                    address: None,
                })
            })
            .collect();
        self.record_block(block, fields, Vec::new(), Vec::new(), None);
    }

    /// The fields the function block `block` declares, in the order it
    /// declares them; not those it inherits. An alias of a block has the
    /// block's fields. `None` when `block` is not a function block whose
    /// fields are recorded.
    pub fn block_fields(&self, block: TypeId) -> Option<&[BlockField]> {
        Some(&self.block_record(block)?.fields)
    }

    /// The field `name` of an instance of the function block `block`: one
    /// the block declares, else one it inherits through `EXTENDS`.
    pub fn block_field(&self, block: TypeId, name: &Id) -> Option<&BlockField> {
        self.inherited_records(block)
            .find_map(|record| record.fields.iter().find(|field| field.name == *name))
    }

    /// Whether `name` is a property of an instance of the function block
    /// `block`: one the block declares, or one it inherits through
    /// `EXTENDS`.
    pub fn has_property(&self, block: TypeId, name: &Id) -> bool {
        self.property(block, name).is_some()
    }

    /// The property `name` of an instance of the function block `block`:
    /// one the block declares, else the one the nearest block it inherits
    /// from through `EXTENDS` declares.
    pub(crate) fn property(&self, block: TypeId, name: &Id) -> Option<&PropertySignature> {
        self.inherited_records(block).find_map(|record| {
            record
                .properties
                .iter()
                .find(|property| property.name == *name)
        })
    }

    /// The method `name` of an instance of the function block `block`, and
    /// the block that declares it: `block`, else the nearest block it
    /// inherits from through `EXTENDS` that declares a method of that name.
    pub(crate) fn method(&self, block: TypeId, name: &Id) -> Option<(TypeId, &MethodSignature)> {
        self.inheritance_chain(block).find_map(|declaring| {
            let method = self
                .blocks
                .get(&declaring)?
                .methods
                .iter()
                .find(|method| method.name == *name)?;
            Some((declaring, method))
        })
    }

    /// The block the function block `block` `EXTENDS`, as recorded: only a
    /// function block the library declares (see [`Self::record_block`]).
    /// `None` when `block` extends nothing, extends a name that is not such
    /// a block, or is not a recorded block itself.
    pub(crate) fn base_of(&self, block: TypeId) -> Option<TypeId> {
        self.blocks.get(&block)?.base
    }

    /// The names of the fields a selection on a value of the type `record`
    /// may name, or `None` when `record` is neither a structure nor a
    /// function block whose fields the environment records.
    ///
    /// A block's are the fields it declares, in the order it declares them,
    /// then those of each block it inherits from, nearest first. A
    /// structure's are the fields its type lists, in the order it declares
    /// them: the structure's record decides only that the type is a declared
    /// structure, because it leaves out a field whose type did not resolve
    /// and keeps no order. An interface is entered as an empty structure but
    /// never recorded as one, so the answer for it is `None`, not an empty
    /// list. An alias has the names of the type it names.
    pub fn field_names(&self, record: TypeId) -> Option<Vec<&Id>> {
        if self.recorded_block(record).is_some() {
            return Some(
                self.inherited_records(record)
                    .flat_map(|block| block.fields.iter().map(|field| &field.name))
                    .collect(),
            );
        }
        if self.structures.contains_key(&record) {
            let fields = self.get_by_id(record)?.representation.structure_fields()?;
            return Some(fields.iter().map(|field| &field.name).collect());
        }
        self.field_names(self.alias_base(record)?)
    }

    /// Every field of an instance of the function block `block`: those it
    /// inherits through `EXTENDS`, from the root of its chain down, then
    /// those it declares, each block's in the order it declares them. A
    /// name declared again down the chain (P4044) is listed once, where the
    /// nearer block declares it. `None` when `block` is not a function block
    /// whose fields are recorded.
    pub fn instance_fields(&self, block: TypeId) -> Option<Vec<&BlockField>> {
        let chain: Vec<&BlockRecord> = self.inherited_records(block).collect();
        if chain.is_empty() {
            return None;
        }
        let mut fields: Vec<&BlockField> = Vec::new();
        for record in chain.into_iter().rev() {
            for field in &record.fields {
                fields.retain(|listed| listed.name != field.name);
                fields.push(field);
            }
        }
        Some(fields)
    }

    /// The function blocks an instance of `block` takes its fields from,
    /// nearest first: `block`, the block it `EXTENDS`, that block's base,
    /// and so on. An alias of a block starts at the block it names. Empty
    /// when `block` is not a function block whose fields are recorded.
    pub fn inheritance_chain(&self, block: TypeId) -> impl Iterator<Item = TypeId> + '_ {
        // The `EXTENDS` graph is acyclic, which declaration sorting
        // enforces; the bound only keeps a walk finite whatever it meets.
        std::iter::successors(self.recorded_block(block), |block| {
            self.recorded_block(self.blocks.get(block)?.base?)
        })
        .take(self.blocks.len())
    }

    /// The records of the blocks of `block`'s [`Self::inheritance_chain`],
    /// nearest first.
    fn inherited_records(&self, block: TypeId) -> impl Iterator<Item = &BlockRecord> + '_ {
        self.inheritance_chain(block)
            .filter_map(|block| self.blocks.get(&block))
    }

    /// The id of the declared type of the field `field` of the structure or
    /// function block type `record`, or `None` when `record` has no such
    /// field or its type is not known. A field of an alias of a structure is
    /// the field of the structure it names, and a field a function block
    /// inherits through `EXTENDS` is the field of the block that declares
    /// it.
    pub fn field_type(&self, record: TypeId, field: &Id) -> Option<TypeId> {
        if let Some(field_type) = self
            .structures
            .get(&record)
            .and_then(|fields| fields.get(field))
        {
            return Some(*field_type);
        }
        if let Some(field) = self.block_field(record, field) {
            return field.field_type;
        }
        self.field_type(self.alias_base(record)?, field)
    }

    /// The record of the function block `block`, or of the block an alias
    /// of a block names.
    fn block_record(&self, block: TypeId) -> Option<&BlockRecord> {
        self.blocks.get(&self.recorded_block(block)?)
    }

    /// The function block whose record `block` has: `block`, or the block
    /// an alias of a block names. `None` when no block is recorded for it.
    fn recorded_block(&self, block: TypeId) -> Option<TypeId> {
        if self.blocks.contains_key(&block) {
            return Some(block);
        }
        self.recorded_block(self.alias_base(block)?)
    }

    /// Whether `id` is a function block the library declares, rather than a
    /// standard block or a type of another kind.
    fn is_declared_block(&self, id: TypeId) -> bool {
        self.get_by_id(id).is_some_and(|attributes| {
            attributes.representation.is_function_block() && attributes.standard_block.is_none()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_context::SemanticContext;
    use crate::semantic_type::SemanticType;
    use crate::test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options};
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::TypeEnvironmentBuilder;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// An environment with the elementary types and the standard blocks.
    fn environment() -> TypeEnvironment {
        TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap()
    }

    /// A function block entered under `name`, with no fields recorded.
    fn block(env: &mut TypeEnvironment, name: &str) -> TypeId {
        let type_name = TypeName::from(name);
        env.insert_type(
            &type_name,
            TypeAttributes::new(SemanticType::FunctionBlock {
                name: name.to_string(),
            }),
        );
        env.id_of(&type_name).unwrap()
    }

    /// A structure entered under `name`, with no fields recorded.
    fn structure(env: &mut TypeEnvironment, name: &str) -> TypeId {
        let type_name = TypeName::from(name);
        env.insert_type(
            &type_name,
            TypeAttributes::new(SemanticType::Structure { fields: vec![] }),
        );
        env.id_of(&type_name).unwrap()
    }

    fn id(env: &TypeEnvironment, name: &str) -> TypeId {
        env.id_of(&TypeName::from(name)).unwrap()
    }

    fn field(env: &TypeEnvironment, name: &str, var_type: FunctionBlockVarType) -> BlockField {
        BlockField {
            name: Id::from(name),
            var_type,
            field_type: env.id_of(&TypeName::from("INT")),
            qualifier: DeclarationQualifier::Unspecified,
            address: None,
        }
    }

    /// A property `name` whose type is not recorded, with a `GET` accessor.
    fn property(name: &str) -> PropertySignature {
        PropertySignature {
            name: Id::from(name),
            property_type: None,
            get: true,
            set: false,
        }
    }

    #[test]
    fn record_structure_when_recorded_twice_then_first_kept() {
        let mut env = environment();
        let pair = structure(&mut env, "Pair");
        let (int, string) = (id(&env, "INT"), id(&env, "STRING"));

        env.record_structure(pair, [(Id::from("f"), int)]);
        env.record_structure(pair, [(Id::from("f"), string), (Id::from("g"), string)]);

        assert_eq!(env.field_type(pair, &Id::from("f")), Some(int));
        assert_eq!(env.field_type(pair, &Id::from("g")), None);
    }

    #[test]
    fn record_structure_when_not_structure_then_nothing_recorded() {
        let mut env = environment();
        let block = block(&mut env, "Block");
        let int = id(&env, "INT");

        env.record_structure(int, [(Id::from("f"), int)]);
        env.record_structure(block, [(Id::from("f"), int)]);

        assert_eq!(env.field_type(int, &Id::from("f")), None);
        assert_eq!(env.field_type(block, &Id::from("f")), None);
    }

    #[test]
    fn record_block_when_recorded_twice_then_first_kept() {
        let mut env = environment();
        let derived = block(&mut env, "Derived");
        let first = block(&mut env, "First");
        let second = block(&mut env, "Second");
        let a = field(&env, "a", FunctionBlockVarType::Input);
        let b = field(&env, "b", FunctionBlockVarType::Output);

        env.record_block(
            derived,
            vec![a.clone()],
            vec![property("Kept")],
            vec![],
            Some(first),
        );
        env.record_block(
            derived,
            vec![b],
            vec![property("Dropped")],
            vec![],
            Some(second),
        );

        assert_eq!(env.block_fields(derived), Some([a].as_slice()));
        env.record_block(
            first,
            vec![field(&env, "x", FunctionBlockVarType::Internal)],
            vec![],
            vec![],
            None,
        );
        assert!(env.block_field(derived, &Id::from("x")).is_some());
        assert!(env.has_property(derived, &Id::from("Kept")));
        assert!(!env.has_property(derived, &Id::from("Dropped")));
    }

    #[test]
    fn record_block_when_not_function_block_then_nothing_recorded() {
        let mut env = environment();
        let int = env.id_of(&TypeName::from("INT")).unwrap();
        let a = field(&env, "a", FunctionBlockVarType::Input);

        env.record_block(int, vec![a], vec![], vec![], None);

        assert_eq!(env.block_fields(int), None);
    }

    #[test]
    fn record_block_when_base_names_itself_or_standard_block_then_no_base() {
        let mut env = environment();
        let own = block(&mut env, "Own");
        let derived = block(&mut env, "Derived");
        let ton = env.id_of(&TypeName::from("TON")).unwrap();

        env.record_block(own, vec![], vec![], vec![], Some(own));
        env.record_block(derived, vec![], vec![], vec![], Some(ton));

        assert_eq!(env.block_field(own, &Id::from("missing")), None);
        assert_eq!(env.field_type(derived, &Id::from("Q")), None);
    }

    /// The names of `fields`.
    fn names(fields: Option<Vec<&BlockField>>) -> Option<Vec<String>> {
        fields.map(|fields| fields.iter().map(|field| field.name.to_string()).collect())
    }

    #[test]
    fn instance_fields_when_block_extends_then_root_fields_first() {
        let mut env = environment();
        let (root, middle, leaf) = (
            block(&mut env, "Root"),
            block(&mut env, "Middle"),
            block(&mut env, "Leaf"),
        );
        let input = |name| field(&env, name, FunctionBlockVarType::Input);
        let (a, b, c, d) = (input("a"), input("b"), input("c"), input("d"));

        env.record_block(root, vec![a, b], vec![], vec![], None);
        env.record_block(middle, vec![c], vec![], vec![], Some(root));
        env.record_block(leaf, vec![d], vec![], vec![], Some(middle));

        assert_eq!(
            names(env.instance_fields(leaf)),
            Some(vec!["a".into(), "b".into(), "c".into(), "d".into()])
        );
        assert_eq!(
            names(env.instance_fields(root)),
            Some(vec!["a".into(), "b".into()])
        );
    }

    /// A derived block that declares a name it inherits (P4044) has one
    /// field of that name, the one it declares.
    #[test]
    fn instance_fields_when_inherited_name_redeclared_then_listed_once_as_derived() {
        let mut env = environment();
        let (base, derived) = (block(&mut env, "Base"), block(&mut env, "Derived"));
        let a = field(&env, "a", FunctionBlockVarType::Input);
        let b = field(&env, "b", FunctionBlockVarType::Input);
        let redeclared = field(&env, "a", FunctionBlockVarType::Output);

        env.record_block(base, vec![a, b], vec![], vec![], None);
        env.record_block(
            derived,
            vec![redeclared.clone()],
            vec![],
            vec![],
            Some(base),
        );

        let fields = env.instance_fields(derived).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1], &redeclared);
    }

    #[test]
    fn instance_fields_when_not_function_block_then_none() {
        let env = environment();
        assert_eq!(env.instance_fields(id(&env, "INT")), None);
    }

    #[test]
    fn block_fields_when_standard_block_then_its_fields_in_order() {
        let env = environment();
        let ton = env.id_of(&TypeName::from("TON")).unwrap();
        let names: Vec<String> = env
            .block_fields(ton)
            .unwrap()
            .iter()
            .map(|field| field.name.to_string())
            .collect();

        assert_eq!(names, vec!["IN", "PT", "Q", "ET"]);
        assert_eq!(
            env.field_type(ton, &Id::from("PT")),
            env.id_of(&TypeName::from("TIME"))
        );
    }

    /// An environment with the blocks `A`, `B EXTENDS A` and `C EXTENDS B`,
    /// recorded with no fields.
    fn three_level_chain() -> TypeEnvironment {
        let mut env = environment();
        let a = block(&mut env, "A");
        let b = block(&mut env, "B");
        let c = block(&mut env, "C");
        env.record_block(a, vec![], vec![], vec![], None);
        env.record_block(b, vec![], vec![], vec![], Some(a));
        env.record_block(c, vec![], vec![], vec![], Some(b));
        env
    }

    #[rstest]
    #[case::no_base("A", &["A"])]
    #[case::one_level("B", &["B", "A"])]
    #[case::two_levels("C", &["C", "B", "A"])]
    fn inheritance_chain_when_block_then_it_and_its_bases_nearest_first(
        #[case] start: &str,
        #[case] expected: &[&str],
    ) {
        let env = three_level_chain();
        let expected: Vec<TypeId> = expected.iter().map(|name| id(&env, name)).collect();

        let chain: Vec<TypeId> = env.inheritance_chain(id(&env, start)).collect();

        assert_eq!(chain, expected);
    }

    #[test]
    fn inheritance_chain_when_alias_of_block_then_starts_at_the_block() {
        let mut env = three_level_chain();
        env.insert_alias(&TypeName::from("AliasOfC"), &TypeName::from("C"))
            .unwrap();
        let expected = vec![id(&env, "C"), id(&env, "B"), id(&env, "A")];

        let chain: Vec<TypeId> = env.inheritance_chain(id(&env, "AliasOfC")).collect();

        assert_eq!(chain, expected);
    }

    #[test]
    fn inheritance_chain_when_no_block_recorded_then_empty() {
        let mut env = environment();
        let unrecorded = block(&mut env, "Unrecorded");
        let int = id(&env, "INT");

        assert_eq!(env.inheritance_chain(unrecorded).count(), 0);
        assert_eq!(env.inheritance_chain(int).count(), 0);
    }

    /// Declaration sorting rejects an `EXTENDS` cycle before the environment
    /// is built; the walk still ends if one is recorded.
    #[test]
    fn inheritance_chain_when_extends_cycle_then_walk_ends() {
        let mut env = environment();
        let a = block(&mut env, "A");
        let b = block(&mut env, "B");
        env.record_block(a, vec![], vec![], vec![], Some(b));
        env.record_block(b, vec![], vec![], vec![], Some(a));

        assert!(env.inheritance_chain(a).count() <= env.blocks.len());
    }

    /// A library with a structure whose fields are not in name order, an
    /// alias of it, an interface with a property, and a block `Derived` that
    /// extends `Base`, each declaring a field and a property.
    const SELECTIONS: &str = "
TYPE
  Point : STRUCT y : INT; x : INT; END_STRUCT;
  AliasOfPoint : Point;
END_TYPE
INTERFACE I_Axis
PROPERTY Position : REAL
GET
END_GET
END_PROPERTY
END_INTERFACE
FUNCTION_BLOCK Base
VAR_INPUT
  b : INT;
END_VAR
PROPERTY Speed : REAL
GET
  Speed := 1.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
VAR_OUTPUT
  d : BOOL;
END_VAR
VAR_INPUT
  c : INT;
END_VAR
PROPERTY Mode : INT
GET
  Mode := 1;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK";

    /// The context resolution builds for [`SELECTIONS`].
    fn selections() -> SemanticContext {
        let (_, context) =
            parse_and_resolve_types_with_options(SELECTIONS, &fb_inheritance_options());
        context
    }

    /// The names `field_names` gives for the type `name`, as written.
    fn field_names_of(env: &TypeEnvironment, name: &str) -> Option<Vec<String>> {
        let names = env.field_names(id(env, name))?;
        Some(names.iter().map(|name| name.to_string()).collect())
    }

    #[spec_test(REQ_ETR_analyzer_007)]
    #[rstest]
    #[case::structure_in_declaration_order("Point", Some(&["y", "x"][..]))]
    #[case::alias_of_structure("AliasOfPoint", Some(&["y", "x"][..]))]
    #[case::block_own_then_inherited("Derived", Some(&["d", "c", "b"][..]))]
    #[case::standard_block("TON", Some(&["IN", "PT", "Q", "ET"][..]))]
    #[case::interface("I_Axis", None)]
    #[case::elementary("INT", None)]
    fn field_names_when_type_then_names_a_selection_may_name(
        #[case] type_name: &str,
        #[case] expected: Option<&[&str]>,
    ) {
        let context = selections();
        let expected = expected.map(|names| names.iter().map(|name| name.to_string()).collect());

        assert_eq!(field_names_of(context.types(), type_name), expected);
    }

    #[spec_test(REQ_ETR_analyzer_007)]
    #[rstest]
    #[case::own("Derived", "Mode", true)]
    #[case::inherited("Derived", "Speed", true)]
    #[case::of_derived_block_only("Base", "Mode", false)]
    #[case::field_is_not_a_property("Derived", "d", false)]
    #[case::standard_block("TON", "Q", false)]
    fn has_property_when_name_then_whether_block_or_base_declares_property(
        #[case] block: &str,
        #[case] name: &str,
        #[case] expected: bool,
    ) {
        let context = selections();
        let env = context.types();

        assert_eq!(env.has_property(id(env, block), &Id::from(name)), expected);
    }

    #[rstest]
    #[case::own("Mode", "INT")]
    #[case::inherited("Speed", "REAL")]
    fn property_when_block_or_base_declares_it_then_its_type_and_accessors(
        #[case] name: &str,
        #[case] property_type: &str,
    ) {
        let context = selections();
        let env = context.types();

        assert_eq!(
            env.property(id(env, "Derived"), &Id::from(name)),
            Some(&PropertySignature {
                name: Id::from(name),
                property_type: Some(id(env, property_type)),
                get: true,
                set: false,
            })
        );
    }
}
