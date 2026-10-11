//! Resolves the value a place starts with.
//!
//! A place is a variable, a field or an element, described by its type. The
//! one rule (see the module doc of `xform_resolve_initial_values`) is the
//! same for all of them: the initializer the program writes for the place,
//! else the default its type declares, else the type's implicit default,
//! recursively for every field and element.
//!
//! A type's default is computed once per [`TypeId`] and remembered. The
//! representation of a type in the [`TypeEnvironment`] says what the type is
//! but not what its values start at -- `TYPE MYINT : INT := 7` has the
//! representation of an `INT` -- so a declared default is read from the
//! type's declaration, which the resolver keeps by id.

use std::collections::{HashMap, HashSet};

use ironplc_container::{CharWidth, DEFAULT_STRING_MAX_LENGTH};
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::textual::{Expr, ExprKind};
use ironplc_dsl::type_id::TypeId;

use super::scalar;
use super::value::{Field, Value};
use crate::intermediates::string;
use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;

/// The type of a place: what it is, and the id of the type when it has
/// one, which is where a declared default is found.
#[derive(Clone, Debug)]
pub(super) struct Place {
    pub(super) representation: SemanticType,
    id: Option<TypeId>,
    /// The id of the element type, for an array whose own type has no id (a
    /// structure field spelled `ARRAY[1..3] OF MYINT`).
    element: Option<TypeId>,
}

/// The declarations a type's default comes from, by the type's id.
#[derive(Debug, Default)]
struct Declarations {
    /// The `TYPE` declaration that declares each named type.
    types: HashMap<TypeId, DataTypeDeclarationKind>,
    /// The variables of each function block, by the block's type id.
    blocks: HashMap<TypeId, Vec<VarDecl>>,
}

impl Declarations {
    fn of(types: &TypeEnvironment, library: &Library) -> Self {
        let mut declarations = Declarations::default();
        for element in &library.elements {
            match element {
                // A structure initialization declaration does not name the
                // type it declares: one the type resolver made from an
                // alias names the base, and the alias is recorded as one.
                LibraryElementKind::DataTypeDeclaration(
                    DataTypeDeclarationKind::StructureInitialization(_),
                ) => {}
                LibraryElementKind::DataTypeDeclaration(declaration) => {
                    if let Some(id) = types.id_of(declaration.type_name()) {
                        // A repeated name keeps its first declaration, as the
                        // type environment does.
                        declarations
                            .types
                            .entry(id)
                            .or_insert_with(|| declaration.clone());
                    }
                }
                LibraryElementKind::FunctionBlockDeclaration(block) => {
                    if let Some(id) = types.id_of(&block.name) {
                        declarations
                            .blocks
                            .entry(id)
                            .or_insert_with(|| block.variables.clone());
                    }
                }
                _ => {}
            }
        }
        declarations
    }

    /// The ids of every type a declaration declares, in id order.
    fn ids(&self) -> Vec<TypeId> {
        let mut ids: Vec<TypeId> = self
            .types
            .keys()
            .chain(self.blocks.keys())
            .copied()
            .collect();
        ids.sort();
        ids
    }
}

/// The default of every type a `TYPE` or `FUNCTION_BLOCK` declaration
/// declares, by the type's id, and the declarations they came from.
///
/// Taken from the declarations as the program wrote them, before the type
/// resolver rewrites an initializer's type name to the elementary type it
/// aliases: `b : MYINT` in a structure is `b : INT` afterwards, and the
/// default `MYINT` declares would be lost.
#[derive(Debug, Default)]
pub struct TypeDefaults {
    declarations: Declarations,
    defaults: HashMap<TypeId, Option<Value>>,
}

impl TypeDefaults {
    /// Resolves the default of every type `library` declares.
    pub(crate) fn of(types: &TypeEnvironment, library: &Library) -> Self {
        let declarations = Declarations::of(types, library);
        let defaults = {
            let mut resolver = Resolver {
                types,
                declarations: &declarations,
                defaults: HashMap::new(),
                resolving: HashSet::new(),
            };
            for id in declarations.ids() {
                resolver.type_default(id);
            }
            resolver.defaults
        };
        TypeDefaults {
            declarations,
            defaults,
        }
    }
}

/// Resolves starting values against one library's types.
pub(super) struct Resolver<'a> {
    types: &'a TypeEnvironment,
    declarations: &'a Declarations,
    /// The default of every type resolved so far: `None` for one that could
    /// not be resolved.
    defaults: HashMap<TypeId, Option<Value>>,
    /// The types whose default is being resolved, so that a type that
    /// contains itself (which a rule reports) cannot recurse forever.
    resolving: HashSet<TypeId>,
}

impl<'a> Resolver<'a> {
    /// A resolver for `types`, starting from the defaults of the declared
    /// types in `type_defaults`.
    pub(super) fn new(types: &'a TypeEnvironment, type_defaults: &'a TypeDefaults) -> Self {
        Self {
            types,
            declarations: &type_defaults.declarations,
            defaults: type_defaults.defaults.clone(),
            resolving: HashSet::new(),
        }
    }

    /// The value the variable `declaration` declares starts with. `None`
    /// for a `VAR_IN_OUT` or `VAR_EXTERNAL`, which names another variable
    /// rather than holding a value of its own.
    pub(super) fn declaration(&mut self, declaration: &VarDecl) -> Option<Value> {
        if matches!(
            declaration.var_type,
            VariableType::InOut | VariableType::External
        ) {
            return None;
        }
        let place = self.declaration_place(declaration)?;
        self.initializer(&declaration.initializer, &place)
    }

    /// The value the result of a function or method returning
    /// `return_type` starts with: the default of the type.
    pub(super) fn return_value(&mut self, return_type: &FunctionReturnType) -> Option<Value> {
        let place = match return_type {
            FunctionReturnType::Named(type_name) => self.place_of(self.types.id_of(type_name)?)?,
            FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
                let char_width = match spec.width {
                    StringType::String => CharWidth::Narrow,
                    StringType::WString => CharWidth::Wide,
                };
                Place::of(SemanticType::String {
                    max_len: declared_length(spec.length.as_ref())?,
                    char_width,
                })
            }
        };
        self.default_of(&place)
    }

    /// The place a variable declaration declares. A sized string has the
    /// unsized string's type id, so its length is read from its initializer.
    fn declaration_place(&self, declaration: &VarDecl) -> Option<Place> {
        match &declaration.initializer {
            InitialValueAssignmentKind::String(initializer) => {
                declared_length(initializer.length.as_ref())?;
                Some(Place::of(string::from(initializer).representation))
            }
            _ => self.place_of(declaration.type_id?),
        }
    }

    /// The place of a value of the type `id` identifies.
    fn place_of(&self, id: TypeId) -> Option<Place> {
        Some(Place {
            representation: self.types.get_by_id(id)?.representation.clone(),
            id: Some(id),
            element: self.types.element_type(id),
        })
    }

    /// The value a place starts with when nothing initializes it: its
    /// type's default.
    fn default_of(&mut self, place: &Place) -> Option<Value> {
        match place.id {
            Some(id) => self.type_default(id),
            None => self.implicit(place),
        }
    }

    /// The value a variable of the type `id` starts with when its
    /// declaration states none. Every part of it is synthesized: none of it
    /// is written where the variable is declared.
    fn type_default(&mut self, id: TypeId) -> Option<Value> {
        if let Some(value) = self.defaults.get(&id) {
            return value.clone();
        }
        if !self.resolving.insert(id) {
            return None;
        }
        let value = if let Some(declaration) = self.declarations.types.get(&id).cloned() {
            self.declared_default(&declaration, id)
        } else if self.declarations.blocks.contains_key(&id) {
            self.block_default(id)
        } else if let Some(base) = self.types.alias_base(id) {
            self.type_default(base)
        } else {
            self.place_of(id).and_then(|place| self.implicit(&place))
        };
        self.resolving.remove(&id);
        let value = value.map(Value::synthesized);
        self.defaults.insert(id, value.clone());
        value
    }

    /// The default the `TYPE` declaration of the type `id` gives it.
    fn declared_default(
        &mut self,
        declaration: &DataTypeDeclarationKind,
        id: TypeId,
    ) -> Option<Value> {
        let place = self.place_of(id)?;
        match declaration {
            DataTypeDeclarationKind::Simple(simple) => {
                self.alias_default(&simple.spec_and_init, &place)
            }
            DataTypeDeclarationKind::Subrange(subrange) => {
                let base = match &subrange.spec {
                    SpecificationKind::Named(base) => self.type_default(self.types.id_of(base)?),
                    SpecificationKind::Inline(_) => self.implicit(&place),
                };
                match &subrange.default {
                    Some(default) => self.signed_integer(default, &place),
                    None => base,
                }
            }
            DataTypeDeclarationKind::String(declaration) => match &declaration.init {
                Some(literal) => string_value(literal, &place),
                None => self.implicit(&place),
            },
            DataTypeDeclarationKind::Array(array) => {
                if !array.init.is_empty() {
                    return self.array(&place, &array.init);
                }
                match &array.spec {
                    SpecificationKind::Named(base) => self.type_default(self.types.id_of(base)?),
                    SpecificationKind::Inline(_) => self.implicit(&place),
                }
            }
            DataTypeDeclarationKind::Structure(structure) => {
                let mut fields = Vec::with_capacity(structure.elements.len());
                for element in &structure.elements {
                    let field = self.field_place(&place, &element.name)?;
                    fields.push(Field {
                        name: element.name.clone(),
                        value: self.initializer(&element.init, &field)?,
                    });
                }
                Some(Value::Structure(fields))
            }
            DataTypeDeclarationKind::Reference(_) => Some(Value::Reference(
                ReferenceInitialValue::Null(SourceSpan::synthesized()),
            )),
            // An enumeration's default is recorded with its members.
            DataTypeDeclarationKind::Enumeration(_)
            | DataTypeDeclarationKind::StructureInitialization(_)
            | DataTypeDeclarationKind::LateBound(_) => self.implicit(&place),
        }
    }

    /// The default an alias (`TYPE MYINT : INT := 7`) gives the type
    /// `place` is: its own initializer applied over the default of the type
    /// it names.
    fn alias_default(
        &mut self,
        spec_and_init: &InitialValueAssignmentKind,
        place: &Place,
    ) -> Option<Value> {
        match spec_and_init {
            InitialValueAssignmentKind::Simple(simple) => match &simple.initial_value {
                Some(constant) => self.constant(constant, place),
                None => self.type_default(self.types.id_of(&simple.type_name)?),
            },
            InitialValueAssignmentKind::String(initializer) => match &initializer.initial_value {
                Some(literal) => string_value(literal, place),
                None => self.implicit(place),
            },
            InitialValueAssignmentKind::FunctionBlock(block) => {
                let base = self.type_default(self.types.id_of(&block.type_name)?)?;
                self.members(base, place, &block.init)
            }
            InitialValueAssignmentKind::Structure(structure) => {
                let base = self.type_default(self.types.id_of(&structure.type_name)?)?;
                self.members(base, place, &structure.elements_init)
            }
            InitialValueAssignmentKind::Subrange(SubrangeInitialValueAssignment {
                spec: SpecificationKind::Named(base),
                ..
            }) => self.type_default(self.types.id_of(base)?),
            InitialValueAssignmentKind::Array(array) => {
                if !array.initial_values.is_empty() {
                    return self.array(place, &array.initial_values);
                }
                match &array.spec {
                    SpecificationKind::Named(base) => self.type_default(self.types.id_of(base)?),
                    SpecificationKind::Inline(_) => self.implicit(place),
                }
            }
            _ => self.implicit(place),
        }
    }

    /// The value of an instance of the function block `id` that nothing
    /// initializes: each of its inputs, outputs and internal variables at the
    /// value the block declares for it.
    fn block_default(&mut self, id: TypeId) -> Option<Value> {
        let variables = self.declarations.blocks.get(&id)?.clone();
        let mut fields = Vec::new();
        for variable in instance_variables(&variables) {
            fields.push(Field {
                name: variable.identifier.symbolic_id()?.clone(),
                value: self.declaration(variable)?,
            });
        }
        Some(Value::Structure(fields))
    }

    /// The value a place of `place`'s type starts with when neither the
    /// place nor its type declares one: `FALSE`, zero, an enumeration's
    /// default member, a subrange's lower bound, an empty string, `NULL`,
    /// and the defaults of every field and element of an aggregate.
    fn implicit(&mut self, place: &Place) -> Option<Value> {
        match &place.representation {
            SemanticType::String {
                max_len,
                char_width,
            } => {
                max_length(*max_len)?;
                Some(Value::Constant(ConstantKind::CharacterString(
                    CharacterStringLiteral {
                        value: vec![],
                        width: string_type(*char_width),
                        span: SourceSpan::synthesized(),
                    },
                )))
            }
            SemanticType::Reference { .. } => Some(Value::Reference(ReferenceInitialValue::Null(
                SourceSpan::synthesized(),
            ))),
            SemanticType::Structure { fields } | SemanticType::FunctionBlock { fields, .. } => {
                let names: Vec<Id> = fields.iter().map(|field| field.name.clone()).collect();
                let mut values = Vec::with_capacity(names.len());
                for name in names {
                    let field = self.field_place(place, &name)?;
                    values.push(Field {
                        name: name.with_position(SourceSpan::synthesized()),
                        value: self.default_of(&field)?,
                    });
                }
                Some(Value::Structure(values))
            }
            SemanticType::Array { .. } => self.array(place, &[]),
            SemanticType::Function { .. } => None,
            scalar => scalar::implicit_default(scalar),
        }
    }

    /// The value `initializer`, a declaration's initializer, gives `place`.
    fn initializer(
        &mut self,
        initializer: &InitialValueAssignmentKind,
        place: &Place,
    ) -> Option<Value> {
        match initializer {
            InitialValueAssignmentKind::None(_) => self.default_of(place),
            InitialValueAssignmentKind::Subrange(subrange) => match &subrange.initial_value {
                Some(value) => self.signed_integer(value, place),
                None => self.default_of(place),
            },
            InitialValueAssignmentKind::Simple(simple) => match &simple.initial_value {
                Some(constant) => self.constant(constant, place),
                None => self.default_of(place),
            },
            InitialValueAssignmentKind::String(string) => match &string.initial_value {
                Some(literal) => string_value(literal, place),
                None => self.default_of(place),
            },
            InitialValueAssignmentKind::EnumeratedValues(values) => match &values.initial_value {
                Some(value) => self.enumerated(value, place),
                None => self.default_of(place),
            },
            InitialValueAssignmentKind::EnumeratedType(enumerated) => {
                match &enumerated.initial_value {
                    Some(value) => self.enumerated(value, place),
                    None => self.default_of(place),
                }
            }
            InitialValueAssignmentKind::FunctionBlock(block) => {
                let base = self.default_of(place)?;
                self.members(base, place, &block.init)
            }
            InitialValueAssignmentKind::Structure(structure) => {
                let base = self.default_of(place)?;
                self.members(base, place, &structure.elements_init)
            }
            InitialValueAssignmentKind::Array(array) => match array.initial_values.is_empty() {
                true => self.default_of(place),
                false => self.array(place, &array.initial_values),
            },
            InitialValueAssignmentKind::Reference(reference) => match &reference.initial_value {
                Some(value) => Some(Value::Reference(value.clone())),
                None => self.default_of(place),
            },
            InitialValueAssignmentKind::LateResolvedType(late) if late.initial_value.is_none() => {
                self.default_of(place)
            }
            // The call-style initializer (`fb : FB(args)`) is refused by a
            // rule, and the type resolver and the initializer folder leave
            // none of the others behind.
            InitialValueAssignmentKind::FunctionBlockCall(_)
            | InitialValueAssignmentKind::LateResolvedType(_)
            | InitialValueAssignmentKind::SimpleExpr(_) => None,
        }
    }

    /// `base`, a structure or function block instance value of the type
    /// `place` is, with the fields `elements` names set to the values they
    /// give. Every other field keeps its value in `base`.
    fn members(
        &mut self,
        base: Value,
        place: &Place,
        elements: &[StructureElementInit],
    ) -> Option<Value> {
        let Value::Structure(mut fields) = base else {
            return None;
        };
        for element in elements {
            let index = fields.iter().position(|field| field.name == element.name)?;
            let field = self.field_place(place, &element.name)?;
            let current = fields[index].value.clone();
            fields[index] = Field {
                name: element.name.clone(),
                value: self.member(&element.init, &field, current)?,
            };
        }
        Some(Value::Structure(fields))
    }

    /// The value one member of a structure initializer gives the field
    /// `place`, whose value is otherwise `current`.
    fn member(
        &mut self,
        init: &StructInitialValueAssignmentKind,
        place: &Place,
        current: Value,
    ) -> Option<Value> {
        match init {
            StructInitialValueAssignmentKind::Constant(constant) => self.constant(constant, place),
            StructInitialValueAssignmentKind::EnumeratedValue(value) => {
                self.enumerated(value, place)
            }
            StructInitialValueAssignmentKind::Array(elements) => self.array(place, elements),
            StructInitialValueAssignmentKind::Structure(elements) => {
                self.members(current, place, elements)
            }
            StructInitialValueAssignmentKind::Expression(expr) => self.expression(expr, place),
            StructInitialValueAssignmentKind::LateBound(_) => None,
        }
    }

    /// The value an expression member gives `place`: the constant it is,
    /// once folded, or the expression itself to evaluate when the variable
    /// is initialized.
    fn expression(&mut self, expr: &Expr, place: &Place) -> Option<Value> {
        match &expr.kind {
            ExprKind::Const(constant) => self.constant(constant, place),
            ExprKind::EnumeratedValue(value) => self.enumerated(value, place),
            ExprKind::Expression(inner) => self.expression(inner, place),
            // The conversion is to the field's type, which the value is
            // converted to below anyway.
            ExprKind::ImplicitConversion(inner)
                if matches!(
                    inner.kind,
                    ExprKind::Const(_) | ExprKind::EnumeratedValue(_)
                ) =>
            {
                self.expression(inner, place)
            }
            ExprKind::Null(span)
                if matches!(place.representation, SemanticType::Reference { .. }) =>
            {
                Some(Value::Reference(ReferenceInitialValue::Null(span.clone())))
            }
            _ => Some(Value::Expression(expr.clone())),
        }
    }

    /// The array `place` with its leading elements set by `elements`, in
    /// storage order, and the rest at the element default. `None` when
    /// `elements` give more values than the array has, which a rule reports.
    ///
    /// The values are listed flat whatever the array's shape, so an array of
    /// arrays takes them element by element of its innermost arrays, and
    /// its value lists them so too.
    fn array(&mut self, place: &Place, elements: &[ArrayInitialElementKind]) -> Option<Value> {
        let mut leaf = place.clone();
        while matches!(leaf.representation, SemanticType::Array { .. }) {
            leaf = self.element_place(&leaf)?;
        }
        let defaults = self.element_defaults(place)?;
        let total = defaults.len();

        let mut values = Vec::with_capacity(total);
        for element in elements {
            self.expand(element, &leaf, &defaults, &mut values)?;
        }
        if values.len() > total {
            return None;
        }
        values.extend_from_slice(&defaults[values.len()..]);
        Some(Value::Array(values))
    }

    /// The value of every element of the array `place` when nothing
    /// initializes it, listed flat: the default of its element type, for
    /// each element. An element that is itself an array starts at the
    /// default of its own type (`ROW : ARRAY[1..2] OF INT := [8, 9]`), not
    /// at the default of the type of its elements.
    fn element_defaults(&mut self, place: &Place) -> Option<Vec<Value>> {
        let count = place.representation.array_total_elements()? as usize;
        let element = self.element_place(place)?;
        let default = match self.default_of(&element)? {
            Value::Array(values) => values,
            value => vec![value],
        };
        let total = count.checked_mul(default.len())?;
        Some(default.iter().cloned().cycle().take(total).collect())
    }

    /// Appends the values `element` gives, repetitions expanded, to
    /// `values`. A value the element leaves out (`n()`) is the default at
    /// its position in `defaults`. `None` once there are more values than
    /// `defaults` has.
    fn expand(
        &mut self,
        element: &ArrayInitialElementKind,
        leaf: &Place,
        defaults: &[Value],
        values: &mut Vec<Value>,
    ) -> Option<()> {
        let total = defaults.len();
        match element {
            ArrayInitialElementKind::Constant(constant) => {
                values.push(self.constant(constant, leaf)?)
            }
            ArrayInitialElementKind::EnumValue(value) => values.push(self.enumerated(value, leaf)?),
            ArrayInitialElementKind::Structure(elements) => {
                let base = defaults.get(values.len())?.clone();
                values.push(self.members(base, leaf, elements)?)
            }
            ArrayInitialElementKind::Expression(expr) => values.push(self.expression(expr, leaf)?),
            ArrayInitialElementKind::Repeated(repeated) => {
                let count = usize::try_from(repeated.size.value).ok()?;
                if count > total {
                    return None;
                }
                match repeated.init.as_ref() {
                    Some(inner) => {
                        let mut repetition = Vec::new();
                        let rest = defaults.get(values.len()..)?;
                        self.expand(inner, leaf, rest, &mut repetition)?;
                        for _ in 0..count {
                            values.extend_from_slice(&repetition);
                            if values.len() > total {
                                return None;
                            }
                        }
                    }
                    // `n()` is n elements at the element default.
                    None => {
                        for _ in 0..count {
                            values.push(defaults.get(values.len())?.clone());
                        }
                    }
                }
            }
        }
        (values.len() <= total).then_some(())
    }

    /// The value `constant` gives `place`.
    fn constant(&mut self, constant: &ConstantKind, place: &Place) -> Option<Value> {
        match constant {
            ConstantKind::CharacterString(literal) => string_value(literal, place),
            _ => scalar::from_constant(constant, &place.representation),
        }
    }

    /// The value the signed integer `value`, a subrange's value, gives
    /// `place`.
    fn signed_integer(&mut self, value: &SignedInteger, place: &Place) -> Option<Value> {
        let literal = ConstantKind::IntegerLiteral(IntegerLiteral {
            value: value.clone(),
            data_type: None,
        });
        self.constant(&literal, place)
    }

    /// The value the enumerated value `value` gives `place`.
    fn enumerated(&mut self, value: &EnumeratedValue, place: &Place) -> Option<Value> {
        scalar::from_enumerated_value(value, &place.representation)
    }

    /// The place of the field `name` of the structure or function block
    /// `place`. The field's type id comes from the declaration of the field
    /// when one is known, so that its own declared default applies.
    fn field_place(&self, place: &Place, name: &Id) -> Option<Place> {
        let representation = place
            .representation
            .member_fields()?
            .iter()
            .find(|field| field.name == *name)?
            .field_type
            .clone();
        let declared = place.id.and_then(|id| self.field_declaration(id, name));
        Some(match declared {
            Some(FieldDeclaration::Structure(init)) => {
                let (id, element) = self.declared_ids(init);
                self.typed_place(representation, id, element)
            }
            Some(FieldDeclaration::Block(variable)) => {
                self.typed_place(representation, variable.type_id, None)
            }
            None => Place::of(representation),
        })
    }

    /// The place of a value of `representation`, whose type is `id` and,
    /// for an array, whose element type is `element`.
    ///
    /// The id is kept only when it identifies a type of that very
    /// representation. A sized string (`STRING[10]`) has the unsized
    /// string's id, whose default has the default length, so it is
    /// described by its representation alone.
    fn typed_place(
        &self,
        representation: SemanticType,
        id: Option<TypeId>,
        element: Option<TypeId>,
    ) -> Place {
        let id = id.filter(|id| {
            self.types
                .get_by_id(*id)
                .is_some_and(|attributes| attributes.representation == representation)
        });
        let element = element.or_else(|| id.and_then(|id| self.types.element_type(id)));
        Place {
            representation,
            id,
            element,
        }
    }

    /// The declaration of the field `name` of the structure or function
    /// block type `id`, following aliases to the type they name.
    fn field_declaration(&self, id: TypeId, name: &Id) -> Option<FieldDeclaration<'a>> {
        let mut id = id;
        // An alias chain is as long as the declarations that make it, and
        // a cycle is a rule's to report; the bound only keeps one finite.
        for _ in 0..=self.declarations.types.len() {
            if let Some(DataTypeDeclarationKind::Structure(structure)) =
                self.declarations.types.get(&id)
            {
                return structure
                    .elements
                    .iter()
                    .find(|element| element.name == *name)
                    .map(|element| FieldDeclaration::Structure(&element.init));
            }
            if let Some(variables) = self.declarations.blocks.get(&id) {
                return instance_variables(variables)
                    .find(|variable| variable.identifier.symbolic_id() == Some(name))
                    .map(FieldDeclaration::Block);
            }
            id = self.alias_target(id)?;
        }
        None
    }

    /// The type the alias `id` names, from the environment or from the
    /// alias's own declaration.
    fn alias_target(&self, id: TypeId) -> Option<TypeId> {
        if let Some(base) = self.types.alias_base(id) {
            return Some(base);
        }
        match self.declarations.types.get(&id)? {
            DataTypeDeclarationKind::Simple(simple) => match &simple.spec_and_init {
                InitialValueAssignmentKind::Structure(structure) => {
                    self.types.id_of(&structure.type_name)
                }
                InitialValueAssignmentKind::FunctionBlock(block) => {
                    self.types.id_of(&block.type_name)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The type id and, for an array spelled in place, the element type id
    /// a structure field's declaration names.
    fn declared_ids(&self, init: &InitialValueAssignmentKind) -> (Option<TypeId>, Option<TypeId>) {
        match init {
            InitialValueAssignmentKind::Array(ArrayInitialValueAssignment {
                spec: SpecificationKind::Inline(subranges),
                ..
            }) => {
                let element = match subranges.ref_to {
                    Some(_) => None,
                    None => self.types.id_of(&subranges.type_name.to_type_name()),
                };
                (None, element)
            }
            _ => match init.type_reference() {
                TypeReference::Named(name) => (self.types.id_of(&name), None),
                _ => (None, None),
            },
        }
    }

    /// The place of an element of the array `place`.
    fn element_place(&self, place: &Place) -> Option<Place> {
        let SemanticType::Array { element_type, .. } = &place.representation else {
            return None;
        };
        let id = place
            .element
            .or_else(|| place.id.and_then(|id| self.types.element_type(id)));
        Some(self.typed_place(element_type.as_ref().clone(), id, None))
    }
}

impl Place {
    /// The place of a value of a type that has no id.
    fn of(representation: SemanticType) -> Self {
        Place {
            representation,
            id: None,
            element: None,
        }
    }
}

/// A field as its type declares it.
enum FieldDeclaration<'d> {
    /// A field of a structure, with the initializer its declaration states.
    Structure(&'d InitialValueAssignmentKind),
    /// An input, output or internal variable of a function block.
    Block(&'d VarDecl),
}

/// The variables of a function block that an instance holds: its inputs,
/// outputs and internal variables, in declaration order.
fn instance_variables(variables: &[VarDecl]) -> impl Iterator<Item = &VarDecl> {
    variables.iter().filter(|variable| {
        matches!(
            variable.var_type,
            VariableType::Input | VariableType::Output | VariableType::Var
        )
    })
}

/// The value the string literal `literal` gives `place`, which must be a
/// string: the literal's characters, up to the length the place holds, as
/// a literal of the place's width.
fn string_value(literal: &CharacterStringLiteral, place: &Place) -> Option<Value> {
    let SemanticType::String {
        max_len,
        char_width,
    } = &place.representation
    else {
        return None;
    };
    let max_length = max_length(*max_len)?;
    Some(Value::Constant(ConstantKind::CharacterString(
        CharacterStringLiteral {
            value: literal
                .value
                .iter()
                .take(usize::from(max_length))
                .copied()
                .collect(),
            width: string_type(*char_width),
            span: literal.span.clone(),
        },
    )))
}

/// The most characters a string declared with length `max_len` holds: the
/// length, or the default for a string declared without one. `None` for a
/// length a string slot cannot record, which a rule reports.
fn max_length(max_len: Option<u128>) -> Option<u16> {
    match max_len {
        Some(length) => u16::try_from(length).ok(),
        None => Some(DEFAULT_STRING_MAX_LENGTH),
    }
}

/// The declared length of a string, `Some(None)` when it declares none, and
/// `None` when the length is a constant that was never resolved, which a
/// rule reports.
fn declared_length(length: Option<&IntegerRef>) -> Option<Option<u128>> {
    match length {
        None => Some(None),
        Some(length) => Some(Some(length.as_integer()?.value)),
    }
}

fn string_type(char_width: CharWidth) -> StringType {
    match char_width {
        CharWidth::Narrow => StringType::String,
        CharWidth::Wide => StringType::WString,
    }
}
