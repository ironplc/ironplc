use crate::ast::*;
use crate::dsl::*;

pub(crate) trait Visitable {
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V);
}

impl<X> Visitable for Vec<X>
where
    X: Visitable,
{
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V) {
        self.into_iter().map(|x| x.visit(visitor));
    }
}

impl<X> Visitable for Option<X>
where
    X: Visitable,
{
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V) {
        self.map(|x| x.visit(visitor));
    }
}

pub trait Visit {
    fn walk(&mut self, node: Library) {
        Visitable::visit(node.elems, self)
    }

    fn visit_enum_declaration(&mut self, enum_decl: EnumerationDeclaration) {}

    fn visit_function_block_declaration(&mut self, func_block_decl: FunctionBlockDeclaration) {
        Visitable::visit(func_block_decl.var_decls, self);
        self.visit_function_block_body(func_block_decl.body);
    }

    fn visit_function_declaration(&mut self, func_decl: FunctionDeclaration) {}

    fn visit_program_declaration(&mut self, prog_decl: ProgramDeclaration) {}

    fn visit_located_var_init(&mut self, var_init: LocatedVarInit) {}

    fn visit_var_init_decl(&mut self, var_init: VarInitDecl) {}

    fn visit_function_block_body(&mut self, body: FunctionBlockBody) {}

    fn visit_variable(&mut self, variable: Variable) {
            
    }
}

impl Visitable for LibraryElement {
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V) {
        match self {
            LibraryElement::ConfigurationDeclaration(config) => {}
            LibraryElement::DataTypeDeclaration(data_type_decl) => {
                Visitable::visit(data_type_decl, visitor);
            }
            LibraryElement::FunctionBlockDeclaration(func_block_decl) => {
                visitor.visit_function_block_declaration(func_block_decl);
            }
            LibraryElement::FunctionDeclaration(func_decl) => {
                visitor.visit_function_declaration(func_decl);
            }
            LibraryElement::ProgramDeclaration(prog_decl) => {
                visitor.visit_program_declaration(prog_decl);
            }
        }
    }
}

impl Visitable for EnumerationDeclaration {
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V) {
        visitor.visit_enum_declaration(self);
    }
}

impl Visitable for VarInitKind {
    fn visit<V: Visit + ?Sized>(self, visitor: &mut V) {
        match self {
            VarInitKind::VarInit(init) => {
                visitor.visit_var_init_decl(init);
            }
            VarInitKind::LocatedVarInit(located_var) => {
                visitor.visit_located_var_init(located_var);
            }
        }
    }
}
mod test {
    use std::collections::LinkedList;
    use crate::ast::*;
    use crate::dsl::*;
    use super::*;

    struct Descender {
        names: LinkedList<String>
    }
    impl Descender {
        fn new() -> Descender {
            Descender {
                names: LinkedList::new()
            }
        }
    }

    impl Visit for Descender {
        fn visit_variable(&mut self, variable: Variable) {

        }
    }

    fn visit_walks_tree() {
        let library = Library {
            elems: vec![LibraryElement::ProgramDeclaration(ProgramDeclaration {
                type_name: String::from("plc_prg"),
                var_declarations: vec![VarInitKind::VarInit(VarInitDecl::simple("Reset", "BOOL"))],
                body: FunctionBlockBody::Statements(vec![StmtKind::fb_assign(
                    "AverageVal",
                    vec!["Cnt1", "Cnt2"],
                    "_TMP_AverageVal17_OUT",
                )]),
            })],
        };

        let mut descender = Descender::new();

        descender.walk(library);

        assert_eq!(2, descender.names.len())
    }
}
