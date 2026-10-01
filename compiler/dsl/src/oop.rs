//! Object-oriented declarations on function blocks and interfaces
//! (IEC 61131-3:2013, CODESYS/TwinCAT): methods, the `EXTENDS`/
//! `IMPLEMENTS`/`ABSTRACT` facet of a function block, and interfaces.
//!
//! Re-exported from [`crate::common`], so these types are also reachable
//! as `common::MethodDeclaration` and so on.
use crate::common::*;
use crate::core::{Id, Located, SourceSpan};
use crate::extension::LanguageExtension;
use crate::fold::Fold;
use crate::member_qualifier::MemberQualifiers;
use crate::scope::ScopeBearing;
use crate::textual::*;
use crate::visitor::Visitor;
use dsl_macro_derive::{Located, Recurse};

/// `METHOD name (: return_type)? ... END_METHOD` (OOP extension).
///
/// Declared on a `FunctionBlockDeclaration`. Shaped like a
/// `FunctionDeclaration`, except `return_type` is optional: unlike a
/// function, a method with no return type is valid IEC 61131-3 and acts
/// like a procedure (see `FunctionSignature::return_type` in
/// `ironplc-analyzer` for the same modeling choice on the resolved side).
///
/// See ADR-0041 ("Staged Method/Property Dispatch and Interface Values"),
/// Phase 1: static dispatch. Resolution of calls against this declaration
/// (own methods first, then the `EXTENDS` chain) is implemented outside
/// the AST, in `ironplc-analyzer`.
#[derive(Clone, Debug, PartialEq, Recurse, Located)]
#[recurse(scope)]
pub struct MethodDeclaration {
    /// Qualifiers between `METHOD` and the name, in source order, such as
    /// `PRIVATE` or `PUBLIC FINAL`. Metadata only (ADR-0041).
    #[recurse(ignore)]
    pub qualifiers: MemberQualifiers,
    pub name: Id,
    pub return_type: Option<FunctionReturnType>,
    pub variables: Vec<VarDecl>,
    /// `R_EDGE`/`F_EDGE`-qualified `VAR` declarations (IEC 61131-3
    /// §2.4.3). Not a TwinCAT-specific or OOP-specific capability: the
    /// parser's `method_declaration()` rule reuses the exact same
    /// standard variable-declaration grammar (`io_var_declarations()`/
    /// `other_var_declarations()`) that `FunctionDeclaration` and
    /// `FunctionBlockDeclaration` already use, so edge variables are
    /// inherited for free, the same way a `VAR` block full stop is.
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: Vec<StmtKind>,
    #[located(position)]
    pub span: SourceSpan,
}

impl HasVariables for MethodDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

impl FunctionBlockDeclaration {
    /// Whether the function block is declared `ABSTRACT`.
    pub fn is_abstract(&self) -> bool {
        self.oop
            .as_ref()
            .is_some_and(|oop| oop.qualifiers.is_abstract())
    }
}

/// `PROPERTY name : type ... END_PROPERTY` (OOP extension).
///
/// Declared on a `FunctionBlockDeclaration`, with a `GET` accessor, a `SET`
/// accessor, or both. Each accessor is represented as the method it
/// behaves as, named after the property:
///
/// - `GET` is a method whose return type is the property type, so inside
///   it the property name is the result variable, as in any method.
/// - `SET` is a method with no return type and one implicit `VAR_INPUT`
///   named after the property and of the property type, which holds the
///   value being assigned. The input is always the first of its
///   `variables`; [`PropertyDeclaration::set_declared_variables`] gives
///   the ones written in the source.
///
/// Every scope-aware pass therefore handles an accessor body as a method
/// body, with no property-specific case. See ADR-0041 Phase 1.
#[derive(Clone, Debug, PartialEq, Recurse, Located)]
pub struct PropertyDeclaration {
    pub name: Id,
    pub property_type: FunctionReturnType,
    pub get: Option<MethodDeclaration>,
    pub set: Option<MethodDeclaration>,
    #[located(position)]
    pub span: SourceSpan,
}

impl PropertyDeclaration {
    /// Builds the `GET` accessor of property `name`.
    pub fn get_accessor(
        name: &Id,
        property_type: &FunctionReturnType,
        variables: Vec<VarDecl>,
        edge_variables: Vec<EdgeVarDecl>,
        body: Vec<StmtKind>,
        span: SourceSpan,
    ) -> MethodDeclaration {
        MethodDeclaration {
            qualifiers: MemberQualifiers::default(),
            name: name.clone(),
            return_type: Some(property_type.clone()),
            variables,
            edge_variables,
            body,
            span,
        }
    }

    /// Builds the `SET` accessor of property `name`, with the implicit
    /// input that holds the assigned value in front of `variables`.
    pub fn set_accessor(
        name: &Id,
        property_type: &FunctionReturnType,
        variables: Vec<VarDecl>,
        edge_variables: Vec<EdgeVarDecl>,
        body: Vec<StmtKind>,
        span: SourceSpan,
    ) -> MethodDeclaration {
        let initializer = match property_type {
            FunctionReturnType::Named(type_name) => InitialValueAssignmentKind::LateResolvedType(
                LateResolvedInitializer::bare(type_name.clone()),
            ),
            FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
                InitialValueAssignmentKind::String(StringInitializer {
                    length: spec.length.clone(),
                    width: spec.width.clone(),
                    initial_value: None,
                    keyword_span: spec.keyword_span.clone(),
                })
            }
        };
        let value = VarDecl {
            identifier: VariableIdentifier::Symbol(name.clone()),
            var_type: VariableType::Input,
            qualifier: DeclarationQualifier::Unspecified,
            initializer,
            block: next_block_id(),
            type_id: None,
        };
        let mut all_variables = Vec::with_capacity(variables.len() + 1);
        all_variables.push(value);
        all_variables.extend(variables);
        MethodDeclaration {
            qualifiers: MemberQualifiers::default(),
            name: name.clone(),
            return_type: None,
            variables: all_variables,
            edge_variables,
            body,
            span,
        }
    }

    /// The `SET` accessor's variables as written in the source, without the
    /// implicit input that [`PropertyDeclaration::set_accessor`] adds.
    pub fn set_declared_variables(&self) -> &[VarDecl] {
        match &self.set {
            Some(set) => &set.variables[1..],
            None => &[],
        }
    }
}

/// The object-oriented facet of a function block: the
/// `EXTENDS`/`IMPLEMENTS` clauses and the qualifiers (`ABSTRACT`, `FINAL`,
/// access specifiers). Present only when the function block uses any of
/// them, so an ordinary function block cannot carry any of this data.
/// Single home for OOP metadata. Note that `oop.is_some()` does not mean
/// the function block takes part in polymorphism: `FUNCTION_BLOCK PUBLIC`
/// alone also creates the facet.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct FunctionBlockOop {
    /// `EXTENDS base` — the single base function block, if any.
    ///
    /// Function blocks are single-inheritance (IEC 61131-3 and
    /// CODESYS/TwinCAT alike), so this is `Option`, not a list. Named
    /// `base` rather than `extends` so the single-base cardinality isn't
    /// hidden behind a name shared with `InterfaceDeclaration::extends`,
    /// which *is* a list.
    pub base: Option<TypeName>,
    /// `IMPLEMENTS i1, i2, ...` — interfaces this function block
    /// implements. Multiple allowed; empty `Vec` when the clause is
    /// absent.
    pub implements: Vec<TypeName>,
    /// Qualifiers between `FUNCTION_BLOCK` and the name, in source order.
    /// An `ABSTRACT` function block cannot be instantiated directly
    /// (enforced by `rule_abstract_not_instantiated`, P4045).
    #[recurse(ignore)]
    pub qualifiers: MemberQualifiers,
    /// Span of the OOP-related tokens, so diagnostics can point at the
    /// clause rather than the whole function block.
    pub span: SourceSpan,
}

/// Only the OOP facet is a dialect/edition extension — the rest of a
/// function block is standard IEC 61131-3. Implementing `LanguageExtension`
/// here (rather than on `FunctionBlockDeclaration`) means an ordinary
/// function block is not, and cannot claim to be, an extension.
impl LanguageExtension for FunctionBlockOop {
    fn extension_name(&self) -> &'static str {
        "EXTENDS/IMPLEMENTS/ABSTRACT clause"
    }

    fn extension_span(&self) -> SourceSpan {
        self.span.clone()
    }
}

/// `INTERFACE name (EXTENDS base_list)? END_INTERFACE` (OOP
/// extension).
///
/// Only the header is represented — method and property signatures are not
/// yet parsed (TwinCAT stores each as a separate `<Method>`/`<Property>` XML
/// element, silently ignored today; see
/// `specs/design/beckhoff-twincat-dialect.md` §1.3). This
/// is enough for an interface name to be recognized as a known type, so
/// that variables declared with an interface type resolve instead of
/// failing with "type not declared."
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct InterfaceDeclaration {
    pub name: Id,
    /// Interfaces this interface extends (an interface may extend more than
    /// one other interface, unlike a function block).
    pub extends: Vec<TypeName>,
}

impl Located for InterfaceDeclaration {
    /// Derived from the declaration's own located parts rather than stored:
    /// the name (always present) through the last extended interface, if any.
    /// This spans the declaration's identifiers rather than the surrounding
    /// `INTERFACE`/`END_INTERFACE` keywords.
    fn span(&self) -> SourceSpan {
        match self.extends.last() {
            Some(last) => SourceSpan::join(&self.name.span(), &last.span()),
            None => self.name.span(),
        }
    }
}

/// An `InterfaceDeclaration` is always an extension — unlike
/// `FunctionBlockDeclaration`, there is no standard-IEC-61131-3 meaning for
/// it.
impl LanguageExtension for InterfaceDeclaration {
    fn extension_name(&self) -> &'static str {
        "INTERFACE declaration"
    }

    fn extension_span(&self) -> SourceSpan {
        self.span()
    }
}
