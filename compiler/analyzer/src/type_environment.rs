//! Type environment describes "what is needed to implement the type
//! as machine code". The type environment contains both types defined
//! by the language and user-defined types.
use std::collections::HashMap;

use ironplc_container::CharWidth;
use ironplc_dsl::{
    common::{
        ArrayElementType, ArraySubranges, ElementaryTypeName, ReferenceTarget, SpecificationKind,
        TypeName,
    },
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::{Expr, ExprType},
};
use ironplc_problems::Problem;

use crate::intermediates::{array, string, TypeResolution};
use crate::semantic_type::{ByteSized, SemanticType};
use crate::symbol_environment::duplicate_declaration;
use crate::type_id;
use ironplc_dsl::type_id::TypeId;

mod fields;
mod interfaces;
mod methods;
mod properties;
mod same_type;
pub use fields::BlockField;
pub(crate) use methods::{MethodParameter, MethodResult, MethodSignature};
pub(crate) use properties::PropertySignature;

static ELEMENTARY_TYPES_LOWER_CASE: [(&str, ElementaryTypeName, SemanticType); 29] = [
    // signed_integer_type_name
    (
        "sint",
        ElementaryTypeName::SINT,
        SemanticType::Int {
            size: ByteSized::B8,
        },
    ),
    (
        "int",
        ElementaryTypeName::INT,
        SemanticType::Int {
            size: ByteSized::B16,
        },
    ),
    (
        "dint",
        ElementaryTypeName::DINT,
        SemanticType::Int {
            size: ByteSized::B32,
        },
    ),
    (
        "lint",
        ElementaryTypeName::LINT,
        SemanticType::Int {
            size: ByteSized::B64,
        },
    ),
    // unsigned_integer_type_name
    (
        "usint",
        ElementaryTypeName::USINT,
        SemanticType::UInt {
            size: ByteSized::B8,
        },
    ),
    (
        "uint",
        ElementaryTypeName::UINT,
        SemanticType::UInt {
            size: ByteSized::B16,
        },
    ),
    (
        "udint",
        ElementaryTypeName::UDINT,
        SemanticType::UInt {
            size: ByteSized::B32,
        },
    ),
    (
        "ulint",
        ElementaryTypeName::ULINT,
        SemanticType::UInt {
            size: ByteSized::B64,
        },
    ),
    // real_type_name
    (
        "real",
        ElementaryTypeName::REAL,
        SemanticType::Real {
            size: ByteSized::B32,
        },
    ),
    (
        "lreal",
        ElementaryTypeName::LREAL,
        SemanticType::Real {
            size: ByteSized::B64,
        },
    ),
    // time_type_name — TIME must appear before TIME_OF_DAY so that
    // resolve_elementary_type_name() finds the canonical name first.
    (
        "time",
        ElementaryTypeName::TIME,
        SemanticType::Time {
            size: ByteSized::B32,
        },
    ),
    // LTIME is 64-bit (milliseconds) — IEC 61131-3 Edition 3 (2013)
    (
        "ltime",
        ElementaryTypeName::LTIME,
        SemanticType::Time {
            size: ByteSized::B64,
        },
    ),
    // date_type_name — DATE must appear before DATE_AND_TIME for the
    // same reason.
    (
        "date",
        ElementaryTypeName::DATE,
        SemanticType::Date {
            size: ByteSized::B32,
        },
    ),
    (
        "time_of_day",
        ElementaryTypeName::TimeOfDay,
        SemanticType::TimeOfDay {
            size: ByteSized::B32,
        },
    ),
    (
        "tod",
        ElementaryTypeName::TimeOfDay,
        SemanticType::TimeOfDay {
            size: ByteSized::B32,
        },
    ),
    (
        "date_and_time",
        ElementaryTypeName::DateAndTime,
        SemanticType::DateAndTime {
            size: ByteSized::B32,
        },
    ),
    (
        "dt",
        ElementaryTypeName::DateAndTime,
        SemanticType::DateAndTime {
            size: ByteSized::B32,
        },
    ),
    // IEC 61131-3 Edition 3 (2013) long date/time types
    (
        "ldate",
        ElementaryTypeName::LDATE,
        SemanticType::Date {
            size: ByteSized::B64,
        },
    ),
    (
        "ltime_of_day",
        ElementaryTypeName::LTimeOfDay,
        SemanticType::TimeOfDay {
            size: ByteSized::B64,
        },
    ),
    (
        "ltod",
        ElementaryTypeName::LTimeOfDay,
        SemanticType::TimeOfDay {
            size: ByteSized::B64,
        },
    ),
    (
        "ldate_and_time",
        ElementaryTypeName::LDateAndTime,
        SemanticType::DateAndTime {
            size: ByteSized::B64,
        },
    ),
    (
        "ldt",
        ElementaryTypeName::LDateAndTime,
        SemanticType::DateAndTime {
            size: ByteSized::B64,
        },
    ),
    // bit_string_type_name
    ("bool", ElementaryTypeName::BOOL, SemanticType::Bool),
    (
        "byte",
        ElementaryTypeName::BYTE,
        SemanticType::Bytes {
            size: ByteSized::B8,
        },
    ),
    (
        "word",
        ElementaryTypeName::WORD,
        SemanticType::Bytes {
            size: ByteSized::B16,
        },
    ),
    (
        "dword",
        ElementaryTypeName::DWORD,
        SemanticType::Bytes {
            size: ByteSized::B32,
        },
    ),
    (
        "lword",
        ElementaryTypeName::LWORD,
        SemanticType::Bytes {
            size: ByteSized::B64,
        },
    ),
    // remaining elementary_type_name
    (
        "string",
        ElementaryTypeName::STRING,
        SemanticType::String {
            max_len: None,
            char_width: CharWidth::Narrow,
        },
    ),
    (
        "wstring",
        ElementaryTypeName::WSTRING,
        SemanticType::String {
            max_len: None,
            char_width: CharWidth::Wide,
        },
    ),
];

/// Returns what an elementary type is, by name.
///
/// The elementary type table above is the compiler's statement of what each
/// elementary type is -- `INT` is a signed 16-bit integer. This reads that
/// statement without building a [`TypeEnvironment`], so that code outside the
/// analyzer works from it rather than from a copy of it.
///
/// Returns `None` for any name that is not an elementary type.
pub fn elementary_type(type_name: &TypeName) -> Option<&'static SemanticType> {
    ELEMENTARY_TYPES_LOWER_CASE
        .iter()
        .find(|(name, _, _)| type_name.name.lower_case == *name)
        .map(|(_, _, elem_type)| elem_type)
}

/// A type in the environment: what it is, and the name it was declared with.
#[derive(Debug)]
struct TypeEntry {
    /// The name the type was first entered under, the elementary type's name
    /// for a sized string (see [`TypeEnvironment::insert_sized_string`]), or
    /// `None` for an anonymous type (see [`TypeEnvironment::insert_anonymous`]).
    /// It is for people reading diagnostics and debug output -- a type's
    /// identity is its [`TypeId`], never its name.
    name: Option<TypeName>,
    attributes: crate::type_attributes::TypeAttributes,
}

/// Every type the program can use, each identified by a [`TypeId`].
///
/// Names map to ids. Most types have one name; the spellings of an
/// elementary type (`TIME_OF_DAY` and `TOD`) share the elementary type's id.
/// A type alias (`TYPE MyByte : BYTE`) is a type of its own with an id of its
/// own. See [`crate::type_id`] for how ids are numbered.
#[derive(Debug)]
pub struct TypeEnvironment {
    entries: HashMap<TypeId, TypeEntry>,
    names: HashMap<TypeName, TypeId>,
    /// The id the next type that is not elementary gets.
    next_id: u32,
    /// The reference type to each type, by the id of the type referenced:
    /// every `REF_TO T` without a name of its own is this one type.
    references: HashMap<TypeId, TypeId>,
    /// The type each reference type in `references` references.
    referenced: HashMap<TypeId, TypeId>,
    /// The element type of each array type whose element is not
    /// elementary, by the id of the array type. The representation of an
    /// array holds its element's structure but not its name, so an array of
    /// `Point` could not otherwise be traced back to `Point`.
    array_elements: HashMap<TypeId, TypeId>,
    /// The type each alias (`TYPE MyByte : BYTE`) was declared as an alias
    /// of, by the alias's id. The alias copies its base's representation,
    /// which says what the type is but not what its values start at.
    alias_bases: HashMap<TypeId, TypeId>,
    /// The declared type of each field of a structure type, by the id of
    /// the type, then the field's name. The representation of a structure
    /// holds each field's structure but not its type, so a field of an
    /// enumeration, a `WSTRING` or an array of `WSTRING` could not otherwise
    /// be traced back to it. See [`fields`].
    structures: HashMap<TypeId, HashMap<Id, TypeId>>,
    /// The fields of each function block, and the block it `EXTENDS`, by the
    /// block's id. A block's type lists none of its fields. See [`fields`].
    blocks: HashMap<TypeId, fields::BlockRecord>,
    /// The signatures of the members each interface declares, by the
    /// interface's id. An interface's type lists none of its members. See
    /// [`interfaces`].
    interfaces: HashMap<TypeId, interfaces::InterfaceRecord>,
    /// The repeated declarations met while populating, until the transform
    /// that populates the environment drains them with
    /// [`Self::take_duplicates`]. Recorded rather than returned so that a
    /// repeat does not abort the fold that met it: the first declaration is
    /// kept and every other declaration still resolves.
    duplicates: Vec<Diagnostic>,
    /// Which interfaces each function block and interface converts to.
    supertypes: crate::supertypes::Supertypes,
}

impl TypeEnvironment {
    /// Initializes a new instance of the type environment.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            names: HashMap::new(),
            next_id: type_id::FIRST_ALLOCATED,
            references: HashMap::new(),
            referenced: HashMap::new(),
            array_elements: HashMap::new(),
            alias_bases: HashMap::new(),
            structures: HashMap::new(),
            blocks: HashMap::new(),
            interfaces: HashMap::new(),
            duplicates: Vec::new(),
            supertypes: crate::supertypes::Supertypes::default(),
        }
    }

    /// Which interfaces each function block and interface converts to.
    pub(crate) fn supertypes(&self) -> &crate::supertypes::Supertypes {
        &self.supertypes
    }

    /// Records the direct supertypes of a function block or an interface,
    /// see [`crate::supertypes`].
    pub(crate) fn insert_supertypes(&mut self, name: &TypeName, supertypes: Vec<TypeName>) {
        self.supertypes.insert(name, supertypes);
    }

    /// Adds the type into the environment.
    ///
    /// A name already in the environment keeps its first type; the repeat is
    /// recorded as a diagnostic (`P2007`, or `P4013` when either declaration
    /// is a function block, which is a program organization unit as well as
    /// a type) and dropped. The environment owns this check for every kind
    /// it holds: data types, function blocks and interfaces.
    pub fn insert_type(
        &mut self,
        type_name: &TypeName,
        symbol: crate::type_attributes::TypeAttributes,
    ) {
        let Some(existing) = self.id_of(type_name).and_then(|id| self.entries.get(&id)) else {
            let id = self.allocate();
            self.bind(type_name, id, symbol);
            return;
        };
        let is_function_block = |attributes: &crate::type_attributes::TypeAttributes| {
            matches!(
                attributes.representation,
                SemanticType::FunctionBlock { .. }
            )
        };
        let problem = if is_function_block(&existing.attributes) || is_function_block(&symbol) {
            Problem::PouDeclNameDuplicated
        } else {
            Problem::TypeDeclNameDuplicated
        };
        self.duplicates.push(duplicate_declaration(
            problem,
            &type_name.name,
            existing
                .name
                .as_ref()
                .map(|name| name.span())
                .unwrap_or_default(),
        ));
    }

    /// Adds a type that has no name -- one spelled out where it is used,
    /// such as `a : ARRAY[1..2] OF DINT` -- and returns its id. Every call
    /// adds a new type: two declarations that spell the same shape declare
    /// two types.
    pub fn insert_anonymous(
        &mut self,
        attributes: crate::type_attributes::TypeAttributes,
    ) -> TypeId {
        self.insert_unbound(None, attributes)
    }

    /// Adds the sized string type `attributes`, such as the `STRING[10]` of
    /// `s : STRING[10]`, and returns its id. Its representation records its
    /// capacity (REQ-LOW-analyzer-053), and it has the name of the elementary
    /// string type `elementary`, so whatever asks a type's name sees the
    /// string type it is. The name is not entered in the environment: `id_of`
    /// of it is still `elementary`.
    pub(crate) fn insert_sized_string(
        &mut self,
        elementary: TypeId,
        attributes: crate::type_attributes::TypeAttributes,
    ) -> TypeId {
        let name = self.name_of(elementary).cloned();
        self.insert_unbound(name, attributes)
    }

    /// Adds a type under a new id, named `name`, without entering the name:
    /// no name maps to the type.
    fn insert_unbound(
        &mut self,
        name: Option<TypeName>,
        attributes: crate::type_attributes::TypeAttributes,
    ) -> TypeId {
        let id = self.allocate();
        self.entries.insert(id, TypeEntry { name, attributes });
        id
    }

    /// The reference type to the type `target` identifies: `REF_TO T` for
    /// every `REF_TO T` spelled out in place and every `REF(x)` of a `T`,
    /// entered the first time it is asked for. Unlike other anonymous types a
    /// reference type is one type however often it is spelled, so that
    /// `r := REF(x)` compares two values of the same type. `None` for an id
    /// this environment did not allocate.
    pub fn reference_to(&mut self, target: TypeId) -> Option<TypeId> {
        if let Some(reference) = self.references.get(&target) {
            return Some(*reference);
        }
        let target_type = self.get_by_id(target)?.representation.clone();
        let reference = self.insert_anonymous(crate::type_attributes::TypeAttributes::new(
            SemanticType::Reference {
                target_type: Box::new(target_type),
            },
        ));
        self.references.insert(target, reference);
        self.referenced.insert(reference, target);
        Some(reference)
    }

    /// The type a reference type references, when `reference` is one.
    ///
    /// A reference type from [`Self::reference_to`] knows its target's id. A
    /// named reference type (`TYPE R : REF_TO INT`) knows only its target's
    /// representation, which answers for an elementary target.
    pub fn referenced_type(&self, reference: TypeId) -> Option<TypeId> {
        if let Some(target) = self.referenced.get(&reference) {
            return Some(*target);
        }
        let target = self
            .get_by_id(reference)?
            .representation
            .referenced_type()?;
        self.id_of(&self.elementary_type_name_for(target)?)
    }

    /// Adds an elementary type under one of its spellings. The first
    /// spelling entered names the type; a later one is another name for it.
    fn insert_elementary(
        &mut self,
        type_name: &TypeName,
        elementary: &ElementaryTypeName,
        symbol: crate::type_attributes::TypeAttributes,
    ) {
        let id = type_id::elementary(elementary);
        if self.entries.contains_key(&id) {
            self.names.insert(type_name.clone(), id);
        } else {
            self.bind(type_name, id, symbol);
        }
    }

    /// Allocates the id of a type that is not elementary.
    fn allocate(&mut self) -> TypeId {
        let id = TypeId::from_raw(self.next_id);
        self.next_id += 1;
        id
    }

    /// Enters a new type under `type_name` with id `id`.
    fn bind(
        &mut self,
        type_name: &TypeName,
        id: TypeId,
        attributes: crate::type_attributes::TypeAttributes,
    ) {
        self.names.insert(type_name.clone(), id);
        self.entries.insert(
            id,
            TypeEntry {
                name: Some(type_name.clone()),
                attributes,
            },
        );
    }

    /// The repeated declarations recorded by [`Self::insert_type`] since the
    /// last call, in the order they were met.
    pub fn take_duplicates(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.duplicates)
    }

    /// Adds an alias type into the environment; a repeated name is recorded
    /// as for [`Self::insert_type`].
    ///
    /// Returns an error if the base type is not already in the type
    /// environment.
    pub fn insert_alias(
        &mut self,
        type_name: &TypeName,
        base_type_name: &TypeName,
    ) -> Result<(), Diagnostic> {
        let base_semantic_type = self.get(base_type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::AliasParentTypeNotDeclared,
                Label::span(type_name.span(), "Type alias"),
            )
            .with_secondary(Label::span(base_type_name.span(), "Base type"))
        })?;

        self.insert_type(type_name, base_semantic_type.clone());
        let element = self
            .id_of(base_type_name)
            .and_then(|base| self.array_elements.get(&base).copied());
        if let (Some(alias), Some(element)) = (self.id_of(type_name), element) {
            self.array_elements.entry(alias).or_insert(element);
        }
        if let (Some(alias), Some(base)) = (self.id_of(type_name), self.id_of(base_type_name)) {
            if alias != base {
                self.alias_bases.entry(alias).or_insert(base);
            }
        }
        Ok(())
    }

    /// The type the alias `alias` names (`BYTE` for `TYPE MyByte : BYTE`),
    /// or `None` when `alias` was not declared as an alias.
    pub fn alias_base(&self, alias: TypeId) -> Option<TypeId> {
        self.alias_bases.get(&alias).copied()
    }

    /// Gets the type from the environment.
    pub fn get(&self, type_name: &TypeName) -> Option<&crate::type_attributes::TypeAttributes> {
        self.get_by_id(self.id_of(type_name)?)
    }

    /// Records that the array type `array` has the element type
    /// `elements` spells: the element's own type, a type of its own for a
    /// sized string, which records its capacity as a sized string
    /// variable's does (REQ-LOW-analyzer-053), or the reference type to the
    /// element for `ARRAY[..] OF REF_TO T`. Nothing is recorded when the
    /// element type is not known.
    pub(crate) fn record_array_element(&mut self, array: TypeId, elements: &ArraySubranges) {
        let Some(element) = self.id_of(&elements.type_name.to_type_name()) else {
            return;
        };
        let element = if elements.ref_to.is_some() {
            match self.reference_to(element) {
                Some(reference) => reference,
                None => return,
            }
        } else {
            let sized = match &elements.type_name {
                ArrayElementType::String(spec) | ArrayElementType::WString(spec) => {
                    spec.length.as_ref().map(|_| string::from_spec(spec))
                }
                ArrayElementType::Named(_) => None,
            };
            match sized {
                Some(attributes) => self.insert_sized_string(element, attributes),
                None => element,
            }
        };
        self.array_elements.insert(array, element);
    }

    /// Records the element type of the array type named `array`, declared
    /// with `spec`. An array declared as an alias of another
    /// (`TYPE A : B; END_TYPE`) takes its element through
    /// [`Self::insert_alias`] instead.
    pub(crate) fn record_declared_array_element(
        &mut self,
        array: &TypeName,
        spec: &SpecificationKind<ArraySubranges>,
    ) {
        if let (Some(id), SpecificationKind::Inline(elements)) = (self.id_of(array), spec) {
            self.record_array_element(id, elements);
        }
    }

    /// The id of the element type of the array type `array`, or `None` when
    /// `array` is not an array type or its element type is not known.
    ///
    /// An element of elementary type is found from the array's
    /// representation; any other is the one recorded where the array type
    /// was declared.
    pub fn element_type(&self, array: TypeId) -> Option<TypeId> {
        if let Some(element) = self.array_elements.get(&array) {
            return Some(*element);
        }
        let SemanticType::Array { element_type, .. } = &self.get_by_id(array)?.representation
        else {
            return None;
        };
        self.id_of(&self.elementary_type_name_for(element_type)?)
    }

    /// The id of the type `type_name` names.
    pub fn id_of(&self, type_name: &TypeName) -> Option<TypeId> {
        self.names.get(type_name).copied()
    }

    /// Gets the type `id` identifies. `None` only for an id this environment
    /// did not allocate.
    pub fn get_by_id(&self, id: TypeId) -> Option<&crate::type_attributes::TypeAttributes> {
        self.entries.get(&id).map(|entry| &entry.attributes)
    }

    /// The name the type `id` identifies was declared with, for diagnostics
    /// and debugging. `None` for a type without a name, or an id this
    /// environment did not allocate.
    pub fn name_of(&self, id: TypeId) -> Option<&TypeName> {
        self.entries.get(&id)?.name.as_ref()
    }

    /// Resolves the type a `REF_TO` points at, whether it is named
    /// (`REF_TO INT`, `REF_TO ARR4`) or an inline array
    /// (`REF_TO ARRAY[0..3] OF INT`).
    ///
    /// `declaring` is the declaration that owns the target, used for the primary
    /// diagnostic label.
    pub fn resolve_reference_target(
        &self,
        declaring: &TypeName,
        target: &ReferenceTarget,
    ) -> Result<SemanticType, Diagnostic> {
        match target {
            ReferenceTarget::Named(referenced_type_name) => {
                let referenced_attrs = self.get(referenced_type_name).ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::ParentTypeNotDeclared,
                        Label::span(declaring.span(), "Reference type declaration"),
                    )
                    .with_secondary(Label::span(referenced_type_name.span(), "Referenced type"))
                })?;
                Ok(referenced_attrs.representation.clone())
            }
            ReferenceTarget::Array(array_subranges) => {
                // Resolve the inline array spec: REF_TO ARRAY[1..10] OF INT
                let array_spec = SpecificationKind::Inline(array_subranges.clone());
                let result = array::try_from(declaring, &array_spec, self)?;
                match result {
                    TypeResolution::Type(attrs) => Ok(attrs.representation),
                    TypeResolution::Alias(base_type_name) => Ok(self
                        .get(&base_type_name)
                        .ok_or_else(|| Diagnostic::internal_error())?
                        .representation
                        .clone()),
                }
            }
        }
    }

    /// The representation of the type of an expression's value, when the
    /// analyzer gave it a concrete one.
    ///
    /// An untyped literal is of a generic category (`ANY_INT`) and `NULL` of
    /// no one type, so both answer `None` rather than a representation.
    pub fn representation_of_expr(&self, expr: &Expr) -> Option<&SemanticType> {
        match expr.expr_type.as_ref()? {
            ExprType::Concrete(id) | ExprType::Inferred(id) => {
                Some(&self.get_by_id(*id)?.representation)
            }
            ExprType::Literal(_) | ExprType::Null => None,
        }
    }

    /// Returns if the type is an enumeration.
    pub fn is_enumeration(&self, name: &TypeName) -> bool {
        self.get(name)
            .map(|ty| ty.representation.is_enumeration())
            .unwrap_or(false)
    }

    /// Resolves a type name to its canonical elementary type name.
    ///
    /// Looks up the type, then scans the 24 elementary types to find one with
    /// a matching `SemanticType`. For aliases like `MyByte → BYTE`, returns
    /// `Some(TypeName::from("BYTE"))`. For complex types, returns `None`.
    pub fn resolve_elementary_type_name(&self, type_name: &TypeName) -> Option<TypeName> {
        let repr = &self.get(type_name)?.representation;
        ELEMENTARY_TYPES_LOWER_CASE
            .iter()
            .find(|(_, _, elem_type)| elem_type == repr)
            .map(|(name, _, _)| TypeName::from(name))
    }

    /// Maps an `SemanticType` back to its elementary `TypeName`.
    ///
    /// Scans the elementary type table to find one matching the given
    /// `SemanticType`. Returns `Some(TypeName::from("BOOL"))` for
    /// `SemanticType::Bool`, etc. Returns `None` for complex types
    /// (structs, arrays, enums).
    pub fn elementary_type_name_for(&self, it: &SemanticType) -> Option<TypeName> {
        ELEMENTARY_TYPES_LOWER_CASE
            .iter()
            .find(|(_, _, elem_type)| elem_type == it)
            .map(|(name, _, _)| TypeName::from(name))
    }

    /// Returns the semantic type for a named structure type.
    ///
    /// Returns `Some` with the `SemanticType::Structure` if the type is found
    /// and is a structure type, or `None` if the type is not found or is not a structure.
    pub fn resolve_struct_type(&self, type_name: &TypeName) -> Option<&SemanticType> {
        let attrs = self.get(type_name)?;
        match &attrs.representation {
            it @ SemanticType::Structure { .. } => Some(it),
            _ => None,
        }
    }

    /// An iterator for all types in the environment
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&TypeName, &crate::type_attributes::TypeAttributes)> {
        self.names
            .iter()
            .filter_map(|(name, id)| Some((name, self.get_by_id(*id)?)))
    }

    /// Every type in the environment by id, anonymous types included, in no
    /// particular order.
    pub fn iter_ids(
        &self,
    ) -> impl Iterator<Item = (TypeId, &crate::type_attributes::TypeAttributes)> {
        self.entries
            .iter()
            .map(|(id, entry)| (*id, &entry.attributes))
    }

    /// Every named type but the elementary types and the function blocks, in
    /// no particular order. An alias of an elementary type (`MyBool : BOOL`)
    /// and a sized string type (`MyStr : STRING[10]`) are types of their own,
    /// with ids of their own, so they are listed.
    pub fn iter_user_defined(
        &self,
    ) -> impl Iterator<Item = (&TypeName, &crate::type_attributes::TypeAttributes)> {
        self.names
            .iter()
            .filter(|(_, id)| type_id::elementary_debug_tag(**id).is_none())
            .filter_map(|(name, id)| Some((name, self.get_by_id(*id)?)))
            .filter(|(_, attrs)| !attrs.representation.is_function_block())
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TypeEnvironmentBuilder {
    has_elementary_types: bool,
    has_stdlib_function_blocks: bool,
}

impl TypeEnvironmentBuilder {
    /// Initializes a new instance of the type environment builder.
    pub fn new() -> Self {
        Self {
            has_elementary_types: false,
            has_stdlib_function_blocks: false,
        }
    }

    /// Adds the elementary types to the type environment.
    /// The elementary types are the types that are built into the language.
    pub fn with_elementary_types(mut self) -> Self {
        self.has_elementary_types = true;
        self
    }

    /// Adds the standard library function blocks to the type environment.
    ///
    /// This includes:
    /// - Bistable function blocks: SR, RS
    /// - Edge detection: R_TRIG, F_TRIG
    /// - Counters: CTU, CTD, CTUD
    /// - Timers: TON, TOF, TP
    pub fn with_stdlib_function_blocks(mut self) -> Self {
        self.has_stdlib_function_blocks = true;
        self
    }

    /// Builds the type environment.
    pub fn build(self) -> Result<TypeEnvironment, Diagnostic> {
        let mut env = TypeEnvironment::new();
        if self.has_elementary_types {
            for (name, elementary, representation) in ELEMENTARY_TYPES_LOWER_CASE.iter() {
                env.insert_elementary(
                    &TypeName::from(name),
                    elementary,
                    crate::type_attributes::TypeAttributes::new(representation.clone()),
                );
            }
        }
        if self.has_stdlib_function_blocks {
            for (name, block) in
                crate::intermediates::stdlib_function_block::get_all_stdlib_function_blocks()
            {
                let name = TypeName::from(name);
                env.insert_type(&name, block.attributes);
                env.record_standard_block(&name, &block.fields);
            }
        }
        Ok(env)
    }
}

impl Default for TypeEnvironmentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export TypeAttributes for use by other modules
pub use crate::type_attributes::TypeAttributes;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        semantic_type::{ArrayDimension, ByteSized, SemanticType},
        standard_function_block::StandardFunctionBlock,
        type_attributes::TypeAttributes,
    };
    use ironplc_dsl::common::{
        ArrayElementType, ArraySubranges, Integer, SignedInteger, SignedIntegerRef, Subrange,
    };
    use ironplc_dsl::core::SourceSpan;

    #[test]
    fn insert_type_when_type_already_exists_then_p2007_and_first_kept() {
        let mut env = TypeEnvironment::new();
        let first = TypeName::from_id(&Id::from("TYPE").with_position(SourceSpan::range(0, 4)));
        let second = TypeName::from_id(&Id::from("TYPE").with_position(SourceSpan::range(10, 14)));
        env.insert_type(&first, TypeAttributes::new(SemanticType::Bool));
        env.insert_type(
            &second,
            TypeAttributes::new(SemanticType::Int {
                size: ByteSized::B16,
            }),
        );

        let duplicates = env.take_duplicates();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].code, Problem::TypeDeclNameDuplicated.code());
        assert_eq!(duplicates[0].primary.location.start, 10);
        assert_eq!(duplicates[0].secondary[0].location.start, 0);
        assert_eq!(env.get(&first).unwrap().representation, SemanticType::Bool);
        assert!(env.take_duplicates().is_empty());
    }

    #[test]
    fn insert_type_when_function_block_repeated_then_p4013() {
        let mut env = TypeEnvironment::new();
        let block = || {
            TypeAttributes::new(SemanticType::FunctionBlock {
                name: "FB".to_string(),
            })
        };
        env.insert_type(&TypeName::from("FB"), block());
        env.insert_type(&TypeName::from("FB"), block());

        let duplicates = env.take_duplicates();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].code, Problem::PouDeclNameDuplicated.code());
    }

    #[test]
    fn insert_type_when_function_block_repeats_type_then_p4013() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("Shared"),
            TypeAttributes::new(SemanticType::Bool),
        );
        env.insert_type(
            &TypeName::from("Shared"),
            TypeAttributes::new(SemanticType::FunctionBlock {
                name: "Shared".to_string(),
            }),
        );

        assert_eq!(
            env.take_duplicates()[0].code,
            Problem::PouDeclNameDuplicated.code()
        );
    }

    #[test]
    fn insert_alias_when_type_already_exists_then_ok() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("TYPE"),
            TypeAttributes::new(SemanticType::Bool),
        );
        assert!(env
            .insert_alias(&TypeName::from("TYPE_ALIAS"), &TypeName::from("TYPE"))
            .is_ok());
    }

    #[test]
    fn insert_alias_when_type_doesnt_exist_then_error() {
        let mut env = TypeEnvironment::new();
        assert!(env
            .insert_alias(&TypeName::from("TYPE_ALIAS"), &TypeName::from("TYPE"))
            .is_err());
    }

    #[test]
    fn semantic_type_helper_methods_work_correctly() {
        // Test primitive types
        assert!(SemanticType::Bool.is_primitive());
        assert!(SemanticType::Int {
            size: ByteSized::B16
        }
        .is_primitive());
        assert!(SemanticType::UInt {
            size: ByteSized::B32
        }
        .is_primitive());
        assert!(SemanticType::Real {
            size: ByteSized::B64
        }
        .is_primitive());
        assert!(SemanticType::String {
            max_len: Some(10),
            char_width: CharWidth::Narrow,
        }
        .is_primitive());
        assert!(SemanticType::Time {
            size: ByteSized::B32
        }
        .is_primitive());
        assert!(SemanticType::Date {
            size: ByteSized::B32
        }
        .is_primitive());

        // Test non-primitive types
        assert!(!SemanticType::Enumeration {
            members: crate::enumeration_members::EnumerationMembers::default(),
            underlying_type: None,
        }
        .is_primitive());
        assert!(!SemanticType::Structure { fields: vec![] }.is_primitive());
        assert!(!SemanticType::Array {
            element_type: Box::new(SemanticType::Int {
                size: ByteSized::B16
            }),
            dimensions: vec![ArrayDimension {
                lower: 1,
                upper: 10
            }]
        }
        .is_primitive());

        // Test numeric types
        assert!(SemanticType::Int {
            size: ByteSized::B16
        }
        .is_numeric());
        assert!(SemanticType::UInt {
            size: ByteSized::B32
        }
        .is_numeric());
        assert!(SemanticType::Real {
            size: ByteSized::B64
        }
        .is_numeric());
        assert!(!SemanticType::Bool.is_numeric());
        assert!(!SemanticType::String {
            max_len: Some(10),
            char_width: CharWidth::Narrow,
        }
        .is_numeric());

        // Test subrange types
        let subrange = SemanticType::Subrange {
            base_type: Box::new(SemanticType::Int {
                size: ByteSized::B16,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert!(subrange.is_subrange());
        assert!(!subrange.is_primitive());

        // Test function block types
        let fb_type = SemanticType::FunctionBlock {
            name: "MyFB".to_string(),
        };
        assert!(fb_type.is_function_block());
        assert!(!fb_type.is_primitive());
    }

    #[test]
    fn type_environment_builder_with_elementary_types() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Check that elementary types are present
        assert!(env.get(&TypeName::from("bool")).is_some());
        assert!(env.get(&TypeName::from("int")).is_some());
        assert!(env.get(&TypeName::from("real")).is_some());
        assert!(env.get(&TypeName::from("string")).is_some());
        assert!(env.get(&TypeName::from("time")).is_some());
        assert!(env.get(&TypeName::from("date")).is_some());

        // Check specific type representations
        let int_type = env.get(&TypeName::from("int")).unwrap();
        assert!(matches!(
            &int_type.representation,
            SemanticType::Int {
                size: ByteSized::B16
            }
        ));

        let bool_type = env.get(&TypeName::from("bool")).unwrap();
        assert!(matches!(&bool_type.representation, SemanticType::Bool));
    }

    #[test]
    fn type_environment_is_enumeration_helper() {
        let mut env = TypeEnvironment::new();

        // Add an enumeration type
        env.insert_type(
            &TypeName::from("MY_ENUM"),
            TypeAttributes::new(SemanticType::Enumeration {
                members: crate::enumeration_members::EnumerationMembers::default(),
                underlying_type: None,
            }),
        );

        // Add a non-enumeration type
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes::new(SemanticType::Int {
                size: ByteSized::B16,
            }),
        );

        // Test the helper method
        assert!(env.is_enumeration(&TypeName::from("MY_ENUM")));
        assert!(!env.is_enumeration(&TypeName::from("MY_INT")));
        assert!(!env.is_enumeration(&TypeName::from("NONEXISTENT")));
    }

    #[test]
    fn type_environment_builder_with_stdlib_function_blocks_then_contains_all() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();

        // Check that all 10 stdlib function blocks are present
        assert!(env.get(&TypeName::from("ton")).is_some());
        assert!(env.get(&TypeName::from("tof")).is_some());
        assert!(env.get(&TypeName::from("tp")).is_some());
        assert!(env.get(&TypeName::from("ctu")).is_some());
        assert!(env.get(&TypeName::from("ctd")).is_some());
        assert!(env.get(&TypeName::from("ctud")).is_some());
        assert!(env.get(&TypeName::from("r_trig")).is_some());
        assert!(env.get(&TypeName::from("f_trig")).is_some());
        assert!(env.get(&TypeName::from("sr")).is_some());
        assert!(env.get(&TypeName::from("rs")).is_some());

        // Check that a stdlib FB has the correct type
        let ton = env.get(&TypeName::from("ton")).unwrap();
        assert!(ton.representation.is_function_block());
        assert_eq!(ton.standard_block, Some(StandardFunctionBlock::Ton));
    }

    #[test]
    fn type_environment_builder_with_stdlib_function_blocks_then_field_types_recorded() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();
        let field = |block: &str, field: &str| {
            env.field_type(env.id_of(&TypeName::from(block))?, &Id::from(field))
        };
        let id = |name: &str| env.id_of(&TypeName::from(name));
        assert_eq!(field("ton", "PT"), id("time"));
        assert_eq!(field("ton", "Q"), id("bool"));
        assert_eq!(field("ctu", "CV"), id("int"));
        assert_eq!(field("ctu_lint", "PV"), id("lint"));
        assert_eq!(field("ctu", "MISSING"), None);
    }

    #[test]
    fn resolve_struct_type_when_structure_then_returns_type() {
        let mut env = TypeEnvironment::new();
        let struct_type = SemanticType::Structure { fields: vec![] };
        env.insert_type(
            &TypeName::from("MY_STRUCT"),
            TypeAttributes::new(struct_type.clone()),
        );

        let result = env.resolve_struct_type(&TypeName::from("MY_STRUCT"));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &struct_type);
    }

    #[test]
    fn resolve_struct_type_when_not_structure_then_returns_none() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes::new(SemanticType::Bool),
        );
        env.insert_type(
            &TypeName::from("MY_FB"),
            TypeAttributes::new(SemanticType::FunctionBlock {
                name: "MyFB".to_string(),
            }),
        );

        assert!(env.resolve_struct_type(&TypeName::from("MY_INT")).is_none());
        assert!(env.resolve_struct_type(&TypeName::from("MY_FB")).is_none());
        assert!(env
            .resolve_struct_type(&TypeName::from("NOT_FOUND"))
            .is_none());
    }

    fn subranges(element: &str) -> ArraySubranges {
        ArraySubranges {
            ranges: vec![Subrange {
                start: SignedIntegerRef::Literal(SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 0,
                    },
                    is_neg: false,
                }),
                end: SignedIntegerRef::Literal(SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 3,
                    },
                    is_neg: false,
                }),
            }],
            type_name: ArrayElementType::Named(TypeName::from(element)),
            ref_to: None,
        }
    }

    #[test]
    fn resolve_reference_target_when_named_elementary_then_returns_elementary_type() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        let result = env.resolve_reference_target(
            &TypeName::from("INT_REF"),
            &ReferenceTarget::Named(TypeName::from("INT")),
        );

        assert!(matches!(
            result,
            Ok(SemanticType::Int {
                size: ByteSized::B16
            })
        ));
    }

    #[test]
    fn resolve_reference_target_when_named_array_then_returns_array_type() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("ARR4"),
            TypeAttributes::new(SemanticType::Array {
                element_type: Box::new(SemanticType::Int {
                    size: ByteSized::B16,
                }),
                dimensions: vec![ArrayDimension { lower: 0, upper: 3 }],
            }),
        );

        let result = env.resolve_reference_target(
            &TypeName::from("ARR_REF"),
            &ReferenceTarget::Named(TypeName::from("ARR4")),
        );

        assert!(matches!(result, Ok(SemanticType::Array { .. })));
    }

    #[test]
    fn resolve_reference_target_when_inline_array_then_returns_array_type() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        let result = env.resolve_reference_target(
            &TypeName::from("ARR_REF"),
            &ReferenceTarget::Array(subranges("INT")),
        );

        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(matches!(resolved, SemanticType::Array { .. }));
        assert_eq!(resolved.array_total_elements(), Some(4));
    }

    #[test]
    fn resolve_reference_target_when_named_target_undeclared_then_parent_type_not_declared() {
        let env = TypeEnvironment::new();

        let result = env.resolve_reference_target(
            &TypeName::from("BAD_REF"),
            &ReferenceTarget::Named(TypeName::from("MISSING")),
        );

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            Problem::ParentTypeNotDeclared.code()
        );
    }

    #[test]
    fn resolve_reference_target_when_inline_array_element_undeclared_then_error() {
        let env = TypeEnvironment::new();

        let result = env.resolve_reference_target(
            &TypeName::from("BAD_REF"),
            &ReferenceTarget::Array(subranges("MISSING")),
        );

        assert!(result.is_err());
    }
}
