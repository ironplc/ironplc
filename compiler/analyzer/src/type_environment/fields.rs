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
//! A block's fields and the block it `EXTENDS` are recorded together, once
//! (see [`TypeEnvironment::record_block`]), so they cannot come from two
//! declarations.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

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
}

/// What the environment records about a function block.
#[derive(Debug)]
pub(super) struct BlockRecord {
    /// The fields the block declares, in the order it declares them.
    fields: Vec<BlockField>,
    /// The block it `EXTENDS`, whose fields an instance of it also has.
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
    /// declares them, and the block it `EXTENDS`.
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
        base: Option<TypeId>,
    ) {
        let is_block = self
            .get_by_id(block)
            .is_some_and(|attributes| attributes.representation.is_function_block());
        if !is_block || self.blocks.contains_key(&block) {
            return;
        }
        let base = base.filter(|base| *base != block && self.is_declared_block(*base));
        self.blocks.insert(block, BlockRecord { fields, base });
    }

    /// Records the fields of the standard function block `name`, each of
    /// which has an elementary type.
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
                })
            })
            .collect();
        self.record_block(block, fields, None);
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
        let mut record = self.block_record(block)?;
        // The `EXTENDS` graph is acyclic, which declaration sorting
        // enforces; the bound only keeps a walk finite whatever it meets.
        for _ in 0..=self.blocks.len() {
            if let Some(field) = record.fields.iter().find(|field| field.name == *name) {
                return Some(field);
            }
            record = self.block_record(record.base?)?;
        }
        None
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
        match self.blocks.get(&block) {
            Some(record) => Some(record),
            None => self.block_record(self.alias_base(block)?),
        }
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
    use crate::semantic_type::SemanticType;
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::TypeEnvironmentBuilder;

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

        env.record_block(derived, vec![a.clone()], Some(first));
        env.record_block(derived, vec![b], Some(second));

        assert_eq!(env.block_fields(derived), Some([a].as_slice()));
        env.record_block(
            first,
            vec![field(&env, "x", FunctionBlockVarType::Internal)],
            None,
        );
        assert!(env.block_field(derived, &Id::from("x")).is_some());
    }

    #[test]
    fn record_block_when_not_function_block_then_nothing_recorded() {
        let mut env = environment();
        let int = env.id_of(&TypeName::from("INT")).unwrap();
        let a = field(&env, "a", FunctionBlockVarType::Input);

        env.record_block(int, vec![a], None);

        assert_eq!(env.block_fields(int), None);
    }

    #[test]
    fn record_block_when_base_names_itself_or_standard_block_then_no_base() {
        let mut env = environment();
        let own = block(&mut env, "Own");
        let derived = block(&mut env, "Derived");
        let ton = env.id_of(&TypeName::from("TON")).unwrap();

        env.record_block(own, vec![], Some(own));
        env.record_block(derived, vec![], Some(ton));

        assert_eq!(env.block_field(own, &Id::from("missing")), None);
        assert_eq!(env.field_type(derived, &Id::from("Q")), None);
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
}
