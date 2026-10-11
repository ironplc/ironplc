//! Transformation that records, on every declaration, the value it starts
//! with.
//!
//! What a variable starts with is a language rule, decided here once so that
//! every back end stores the same value (ADR-0056 states the principle).
//! The rule, for a variable of type `T`, applied in order:
//!
//! 1. the declaration's own initializer;
//! 2. else the default `T` declares, through every alias and subrange layer;
//! 3. else `T`'s implicit default: an enumeration's default member, a
//!    subrange's lower bound, an empty string, `NULL`, or zero;
//!
//! recursively for every field and element. A partial structure initializer
//! leaves the fields it does not name at their defaults; an array
//! initializer is expanded (`2(5)`, `3()`), and the elements it does not
//! reach take the element default. A function block instance starts with
//! the values the block declares for its variables, with the instance's own
//! initializer applied over them.
//!
//! ```ignore
//! TYPE
//!     MYINT : INT := 7;
//!     RNG : DINT(10..100);
//!     P : STRUCT x : RNG; y : MYINT; z : INT := 3; END_STRUCT;
//! END_TYPE
//! VAR
//!     m : MYINT;                        (* 7 *)
//!     r : RNG;                          (* 10 *)
//!     p : P := (z := 4);                (* (x := 10, y := 7, z := 4) *)
//!     a : ARRAY[1..4] OF MYINT := [1, 2(5)];   (* [1, 5, 5, 7] *)
//! END_VAR
//! ```
//!
//! The value replaces the declaration's initializer: the pass completes the
//! initializer the program wrote, in place, so that its value slot holds the
//! whole starting value, every scalar a literal of the kind its type stores
//! (an integer that initializes a `REAL` becomes a real literal) and every
//! field and element listed (an array flat, in storage order). The parts the
//! program did not write at the declaration have synthesized spans
//! ([`SourceSpan::synthesized`](ironplc_dsl::core::SourceSpan::synthesized)),
//! so a renderer that shows the program as written leaves them out. A
//! declaration holds one answer to what it starts with, and a back end reads
//! it without deciding anything.
//!
//! ```ignore
//! p : P := (z := 4);
//! (* becomes, with the synthesized parts in brackets: *)
//! p : P := ([x := 10,] [y := 7,] z := 4);
//! ```
//!
//! A function's or method's result, which the program declares only by its
//! return type, gets a [`VarDecl`] of its own
//! ([`FunctionDeclaration::result`]) with its initializer made the same way.
//!
//! The pass also records which variables start again on every call: a
//! function's or method's `VAR`, `VAR_TEMP` and result. A program's or
//! function block's variables are set once.
//!
//! A value the pass cannot resolve -- one outside its type, an array
//! initializer with too many values -- leaves the declaration as written; a
//! semantic rule reports why. The pass reports nothing itself.
//!
//! The pass is to run after implicit conversions are recorded, so that a
//! function block member initialized by an expression (an extension) keeps
//! the conversions the expression's compilation needs, and after the range
//! rule, which checks the literals the program wrote. `analyze` does not run
//! it yet: code generation still reads each initializer as the program wrote
//! it. The defaults of the declared types are resolved earlier, by
//! [`TypeDefaults::of`], from the declarations as the program wrote them,
//! and kept on the [`SemanticContext`].
//!
//! See `specs/design/initial-values.md`.

mod resolver;
mod scalar;
mod value;

pub use resolver::TypeDefaults;

use std::convert::Infallible;

use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::type_id::TypeId;

use crate::semantic_context::SemanticContext;
use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;
use resolver::Resolver;
use value::{Shape, Value};

/// Completes the initializer of every declaration in `library` with the
/// value the declaration starts with, makes the result variable of every
/// function and method, and marks the declarations that start again on
/// every call.
pub fn apply(library: Library, context: &SemanticContext) -> Library {
    let resolver = Resolver::new(context.types(), context.type_defaults());
    let mut folder = InitialValueFolder {
        types: context.types(),
        resolver,
    };
    let Ok(library) = folder.fold_library(library);
    library
}

struct InitialValueFolder<'a> {
    types: &'a TypeEnvironment,
    resolver: Resolver<'a>,
}

impl InitialValueFolder<'_> {
    /// Marks the variables of a function or method that start again on
    /// every call: its `VAR` and `VAR_TEMP` declarations.
    fn mark_reset_on_call(variables: &mut [VarDecl]) {
        for variable in variables {
            variable.reset_on_call = variable.var_type.is_local();
        }
    }

    /// The variable that holds the result of the function or method `name`,
    /// which returns `return_type`: it starts at the default of the type,
    /// again on every call. `None` when that default could not be resolved,
    /// which a rule reports.
    fn result_variable(&mut self, name: &Id, return_type: &FunctionReturnType) -> ResultVariable {
        let variable = self.resolver.return_value(return_type).and_then(|value| {
            let (initializer, type_id) = self.result_initializer(return_type, value)?;
            Some(Box::new(VarDecl {
                identifier: VariableIdentifier::Symbol(name.clone()),
                var_type: VariableType::Var,
                qualifier: DeclarationQualifier::Unspecified,
                initializer,
                block: next_block_id(),
                type_id,
                reset_on_call: true,
            }))
        });
        ResultVariable(variable)
    }

    /// The initializer of a result of the type `return_type` that starts
    /// with `value`, and the id of the type.
    fn result_initializer(
        &self,
        return_type: &FunctionReturnType,
        value: Value,
    ) -> Option<(InitialValueAssignmentKind, Option<TypeId>)> {
        match return_type {
            FunctionReturnType::Named(type_name) => {
                let id = self.types.id_of(type_name)?;
                let shape = self.shape_of(id)?;
                Some((value::initializer_of(type_name, shape, value)?, Some(id)))
            }
            FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
                let Value::Constant(ConstantKind::CharacterString(literal)) = value else {
                    return None;
                };
                let initializer = StringInitializer {
                    length: spec.length.clone(),
                    width: spec.width.clone(),
                    initial_value: Some(literal),
                    keyword_span: spec.keyword_span.clone(),
                };
                let id = self.types.id_of(&initializer.type_name());
                Some((InitialValueAssignmentKind::String(initializer), id))
            }
        }
    }

    /// The initializer of `node`, which states no value, rebuilt in the kind
    /// its type takes and holding `value`. A declaration the type resolver
    /// does not classify (a top-level `VAR_GLOBAL` of a structure type, an
    /// extension) keeps the kind the parser gave it, which has no slot for
    /// the value of a structure or an array.
    fn retyped(&self, node: &VarDecl, value: Value) -> Option<InitialValueAssignmentKind> {
        let type_name = match &node.initializer {
            InitialValueAssignmentKind::Simple(SimpleInitializer {
                type_name,
                initial_value: None,
            })
            | InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name,
                initial_value: None,
            }) => type_name,
            _ => return None,
        };
        let shape = self.shape_of(node.type_id?)?;
        value::initializer_of(type_name, shape, value)
    }

    /// Which kind of initializer a variable of the type `id` takes.
    fn shape_of(&self, id: TypeId) -> Option<Shape> {
        Some(match &self.types.get_by_id(id)?.representation {
            SemanticType::Structure { .. } => Shape::Structure,
            SemanticType::FunctionBlock { .. } => Shape::FunctionBlock,
            SemanticType::Array { .. } => Shape::Array,
            SemanticType::Enumeration { .. } => Shape::Enumerated,
            SemanticType::Subrange { .. } => Shape::Subrange,
            SemanticType::Reference { .. } => {
                let target = self.types.referenced_type(id)?;
                Shape::Reference(self.types.name_of(target)?.clone())
            }
            _ => Shape::Simple,
        })
    }
}

impl Fold<Infallible> for InitialValueFolder<'_> {
    fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, Infallible> {
        let initializer = self.resolver.declaration(&node).and_then(|value| {
            value::complete(&node.initializer, value.clone()).or_else(|| self.retyped(&node, value))
        });
        Ok(match initializer {
            Some(initializer) => VarDecl {
                initializer,
                ..node
            },
            None => node,
        })
    }

    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Infallible> {
        let mut node = node.recurse_fold(self)?;
        Self::mark_reset_on_call(&mut node.variables);
        node.result = self.result_variable(&node.name, &node.return_type);
        Ok(node)
    }

    fn fold_method_declaration(
        &mut self,
        node: MethodDeclaration,
    ) -> Result<MethodDeclaration, Infallible> {
        let mut node = node.recurse_fold(self)?;
        Self::mark_reset_on_call(&mut node.variables);
        if let Some(return_type) = &node.return_type {
            node.result = self.result_variable(&node.name, return_type);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests;
