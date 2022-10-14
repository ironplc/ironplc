use crate::ast::*;
use crate::dsl::*;


pub trait LibraryVisitor<T> {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) -> T;
    fn visit_data_type_declaration(&mut self, l: &Vec<EnumerationDeclaration>) -> T;
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) -> T;
    fn visit_function_block_declaration(&mut self, l: &FunctionBlockDeclaration) -> T;
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) -> T;
}

pub trait Visitor {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) {}
    fn visit_data_type_declaration(&mut self, l: &Vec<EnumerationDeclaration>) {}
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) {}
    fn visit_function_block_declaration(&mut self, l: &FunctionBlockDeclaration) {}
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) {}
}

pub fn walk(visitor: &mut dyn Visitor, library: &mut Library) {}

pub fn walk_library<T>(visitor: &mut dyn LibraryVisitor<T>, l: &Library) {
    for le in &l.elems {
        match le {
            LibraryElement::ConfigurationDeclaration(c) => {}
            LibraryElement::DataTypeDeclaration(d) => {}
            LibraryElement::FunctionBlockDeclaration(f) => {}
            LibraryElement::FunctionDeclaration(f) => {}
            LibraryElement::ProgramDeclaration(p) => {}
        }
    }
}

/*struct Interpreter;
impl Visitor<i64> for Interpreter {
    fn visit_name(&mut self, n:&str) -> i64 {
        0
    }
}*/

/*pub fn walk_expr<T>(visitor: &mut dyn Visitor<T>, e: &ExprKind) {
    match *e {
        ExprKind::Compare(_, ref terms) => {},
        Exprind::Add(ref lsh, ref rhs) => {
            visitor.visit_expr(lhs),
            visitor.visit_expr(rhs),
        }
    }
}*/
