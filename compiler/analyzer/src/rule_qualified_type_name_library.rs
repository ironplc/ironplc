//! Semantic rule that a library prefix on a type name names the library
//! that declares the type.
//!
//! TwinCAT lets a type name carry the library it comes from
//! (`BROTLib.E_Blink`) and rejects a prefix that names any other library, or
//! none. A library here is a compatibility library the build activated or a
//! project loaded alongside (see `LibraryFiles`); the type's declaration
//! says which file it is in, and so which library. Names compare ignoring
//! case, as TwinCAT compares them.
//!
//! A type that no library declares has no prefix to name: a type IronPLC
//! provides itself (`TON`, so `Tc2_Standard.TON` is reported), and one
//! declared in a source that belongs to no library. A type that is not
//! declared at all is reported by the rules that resolve type names.
//!
//! Whether a prefix may be written at all is
//! `rule_qualified_type_name_allowed`'s to report.
//!
//! ## Passes (with `brot.st` in the project `BROTLib`)
//!
//! ```ignore
//! (* brot.st *)
//! TYPE E_Blink : (off, slow); END_TYPE
//!
//! (* main.st *)
//! PROGRAM main
//! VAR
//!     mode : BROTLib.E_Blink := brotlib.E_Blink.slow;
//! END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     mode : Nonsense.E_Blink;   (* no library named Nonsense *)
//!     t : Tc2_Standard.TON;      (* TON is in no library *)
//! END_VAR
//! END_PROGRAM
//! ```
use std::collections::HashMap;
use std::convert::Infallible;

use ironplc_dsl::{
    common::{Library, LibraryElementKind, TypeName},
    core::{FileId, Id},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleQualifiedTypeNameLibrary {
            context,
            declared_in: declaration_files(lib),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// The file that declares each type a source declares.
fn declaration_files(lib: &Library) -> HashMap<Id, FileId> {
    let mut files = HashMap::new();
    for element in &lib.elements {
        let name = match element {
            LibraryElementKind::DataTypeDeclaration(decl) => &decl.type_name().id,
            LibraryElementKind::FunctionBlockDeclaration(decl) => &decl.name.id,
            LibraryElementKind::InterfaceDeclaration(decl) => &decl.name,
            _ => continue,
        };
        files
            .entry(name.clone())
            .or_insert_with(|| name.span.file_id.clone());
    }
    files
}

struct RuleQualifiedTypeNameLibrary<'a> {
    context: &'a SemanticContext,
    declared_in: HashMap<Id, FileId>,
    diagnostics: Vec<Diagnostic>,
}

impl RuleQualifiedTypeNameLibrary<'_> {
    /// Whether `name` names a type that a source or IronPLC declares.
    fn is_declared(&self, name: &TypeName) -> bool {
        self.declared_in.contains_key(&name.id) || self.context.types().id_of(name).is_some()
    }

    /// The library that declares the type `name` names, if any library does.
    fn declaring_library(&self, name: &TypeName) -> Option<&Id> {
        let file = self.declared_in.get(&name.id)?;
        self.context.library_files().library_of(file)
    }
}

impl DiagnosticVisitor for RuleQualifiedTypeNameLibrary<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleQualifiedTypeNameLibrary<'_> {
    type Value = ();

    fn visit_type_name(&mut self, node: &TypeName) -> Result<Self::Value, Infallible> {
        if let Some(prefix) = &node.qualifier {
            if self.is_declared(node) {
                let library = self.declaring_library(node);
                if library != Some(prefix) {
                    let label = match library {
                        Some(_) => "Not the library that declares the type",
                        None => "No library declares the type",
                    };
                    let mut diagnostic = Diagnostic::problem(
                        Problem::QualifiedTypeNameLibraryMismatch,
                        Label::span(prefix.span.clone(), label),
                    )
                    .with_context_id("prefix", prefix)
                    .with_context_id("type", &node.id);
                    if let Some(library) = library {
                        diagnostic = diagnostic.with_context_id("library", library);
                    }
                    self.diagnostics.push(diagnostic);
                }
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests;
