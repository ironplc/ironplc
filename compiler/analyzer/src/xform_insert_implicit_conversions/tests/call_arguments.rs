//! The arguments of the calls of one function in an analyzed program, each
//! as the type it is passed at.

use super::*;

/// The inputs of every call to `name` in `source`, which must be free of
/// diagnostics, each as [`describe`] shows it.
pub(super) fn call_arguments(source: &str, name: &str) -> Vec<String> {
    call_arguments_with(source, name, &CompilerOptions::default())
}

/// [`call_arguments`] under `options`.
pub(super) fn call_arguments_with(
    source: &str,
    name: &str,
    options: &CompilerOptions,
) -> Vec<String> {
    struct Arguments<'a> {
        types: &'a TypeEnvironment,
        name: &'a str,
        arguments: Vec<String>,
    }
    impl Visitor<Infallible> for Arguments<'_> {
        type Value = ();
        fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
            if node.name.original().eq_ignore_ascii_case(self.name) {
                let inputs = node.param_assignment.iter().filter_map(|p| p.input_expr());
                self.arguments
                    .extend(inputs.map(|e| describe(self.types, e)));
            }
            node.recurse_visit(self)
        }
    }
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze(&[&library], options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = Arguments {
        types: context.types(),
        name,
        arguments: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.arguments
}
