use crate::ast::*;
use crate::dsl::*;
use crate::sfc::Network;

pub(crate) trait Visitable {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V);
}

impl<X> Visitable for Vec<X>
where
    X: Visitable,
{
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        self.into_iter().for_each(|x| x.visit(visitor));
    }
}

impl<X> Visitable for Option<X>
where
    X: Visitable,
{
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        match self.as_ref() {
            Some(x) => x.visit(visitor),
            None => {}
        };
    }
}

pub trait Visit {
    fn walk(&mut self, node: &Library) {
        Visitable::visit(&node.elems, self)
    }

    fn visit_enum_declaration(&mut self, enum_decl: &EnumerationDeclaration) {}

    fn visit_function_block_declaration(&mut self, func_block_decl: &FunctionBlockDeclaration) {
        Visitable::visit(&func_block_decl.var_decls, self);
        self.visit_function_block_body(&func_block_decl.body);
    }

    fn visit_function_declaration(&mut self, func_decl: &FunctionDeclaration) {
        Visitable::visit(&func_decl.var_decls, self);
        Visitable::visit(&func_decl.body, self);
    }

    fn visit_program_declaration(&mut self, prog_decl: &ProgramDeclaration) {
        Visitable::visit(&prog_decl.var_declarations, self);
        self.visit_function_block_body(&prog_decl.body);
    }

    fn visit_located_var_init(&mut self, var_init: &LocatedVarInit) {}

    fn visit_var_init_decl(&mut self, var_init: &VarInitDecl) {}

    fn visit_function_block_body(&mut self, body: &FunctionBlockBody) {
        match body {
            FunctionBlockBody::Sfc(network) => Visitable::visit(network, self),
            FunctionBlockBody::Statements(stmts) => Visitable::visit(stmts, self)
        }
    }

    fn visit_assignment(&mut self, assignment: &Assignment) {
        self.visit_variable(&assignment.target);
        // TODO others
    }

    fn visit_variable(&mut self, variable: &Variable) {}

    fn visit_fb_call(&mut self, fb_call: &FbCall) {}
}

impl Visitable for LibraryElement {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
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
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        visitor.visit_enum_declaration(self);
    }
}

impl Visitable for VarInitKind {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
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

impl Visitable for VarInitDecl {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        visitor.visit_var_init_decl(self);
    }
}

impl Visitable for StmtKind {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        match self {
            StmtKind::Assignment(assignment) => {
                visitor.visit_assignment(assignment);
            },
            StmtKind::If { expr, body, else_body } => {

            },
            StmtKind::FbCall(fb_call) => {
                visitor.visit_fb_call(fb_call);
            },
        }
    }
}

impl Visitable for Network {
    fn visit<V: Visit + ?Sized>(&self, visitor: &mut V) {
        // TODO
    }
}

mod test {
    use super::*;
    use crate::ast::*;
    use crate::dsl::*;
    use std::collections::LinkedList;

    struct Descender {
        names: LinkedList<String>,
    }
    impl Descender {
        fn new() -> Descender {
            Descender {
                names: LinkedList::new(),
            }
        }
    }

    impl Visit for Descender {
        fn visit_variable(&mut self, variable: &Variable) {
            let mut dst = &mut self.names;
            match variable {
                Variable::DirectVariable(dv) => dst.push_back(dv.to_string()),
                Variable::SymbolicVariable(sv) => dst.push_back(sv.clone()),
                Variable::MultiElementVariable(mev) => {},
            }
        }
        fn visit_fb_call(&mut self, fb_call: &FbCall) {
            let mut dst = &mut self.names;
            dst.push_back(fb_call.name.clone());
        }
    }

    #[test]
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

        descender.walk(&library);

        assert_eq!(1, descender.names.len())
    }
}
