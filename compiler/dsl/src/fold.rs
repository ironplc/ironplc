//! A set of traits and functions for folding all nodes in a library.
//!
//! Folding the library returns a new instance with changes to the
//! library defined based on the fold_* functions. The default behavior
//! returns an copy of input.
//!
//! To fold a library, define a struct and implement the Fold trait
//! for the struct. The implement fold_* functions from the trait to
//! customize the behavior.
use crate::common::*;
use crate::configuration::*;
use crate::core::*;
use crate::sfc::*;
use crate::textual::*;
use paste::paste;

/// Defines a macro for the Fold struct that dispatches folding
/// to a function. In other words, creates a function of the form:
///
/// ```ignore
/// fn visit_type_name<E>(&mut self, node: TypeName) -> Result<Fold::Value, E> {
///    visit_type_name(self, node)
/// }
/// ```
macro_rules! dispatch
{
    ($struct_name:ident) => {
        paste! {
            fn [<fold_ $struct_name:snake >](&mut self, node: $struct_name) -> Result<$struct_name, E> {
                [< fold_ $struct_name:snake >](self, node)
            }
        }
    };
}

macro_rules! leaf
{
    ($struct_name:ident) => {
        paste! {
            fn [<fold_ $struct_name:snake >](&mut self, node: $struct_name) -> Result<$struct_name, E> {
                Ok(node)
            }
        }
    };
}

// Defines an object as being able to be folded. That is, return a new
// folded version of itself.
pub trait Folder {
    type Mapped;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E>;
}

impl<X> Folder for Vec<X>
where
    X: Folder,
{
    type Mapped = Vec<X::Mapped>;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        self.into_iter().map(|x| x.fold(folder)).collect()
    }
}

impl<X> Folder for Option<X>
where
    X: Folder,
{
    type Mapped = Option<X::Mapped>;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        self.map(|x| x.fold(folder)).transpose()
    }
}

pub trait Fold<E> {
    fn fold_library(&mut self, node: Library) -> Result<Library, E> {
        Ok(Library {
            elements: Folder::fold(node.elements, self)?,
        })
    }

    dispatch!(SourceLoc);

    // 2.1.2.
    fn fold_id(&mut self, node: Id) -> Result<Id, E> {
        Ok(node)
    }

    fn fold_library_element_declaration(
        &mut self,
        node: LibraryElementKind,
    ) -> Result<LibraryElementKind, E> {
        match node {
            LibraryElementKind::DataTypeDeclaration(data_type) => {
                Ok(LibraryElementKind::DataTypeDeclaration(
                    self.fold_data_type_declaration_kind(data_type)?,
                ))
            }
            LibraryElementKind::FunctionBlockDeclaration(function_block_decl) => {
                Ok(LibraryElementKind::FunctionBlockDeclaration(
                    self.fold_function_block_declaration(function_block_decl)?,
                ))
            }
            LibraryElementKind::FunctionDeclaration(function_decl) => {
                Ok(LibraryElementKind::FunctionDeclaration(
                    self.fold_function_declaration(function_decl)?,
                ))
            }
            LibraryElementKind::ProgramDeclaration(program_decl) => {
                Ok(LibraryElementKind::ProgramDeclaration(
                    self.fold_program_declaration(program_decl)?,
                ))
            }
            _ => Ok(node),
        }
    }

    /// Fold data type declarations.
    ///
    /// See section 2.4.3.
    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, E> {
        Ok(node)
    }

    /// Fold variable declaration.
    ///
    /// See section 2.4.3.
    fn fold_variable_declaration(&mut self, node: VarDecl) -> Result<VarDecl, E> {
        Ok(VarDecl {
            identifier: node.identifier,
            var_type: node.var_type,
            qualifier: node.qualifier,
            initializer: Folder::fold(node.initializer, self)?,
            position: node.position,
        })
    }

    /// Fold an address assignment.
    ///
    /// See section 2.4.3.1.
    fn fold_address_assignment(&mut self, node: AddressAssignment) -> Result<AddressAssignment, E> {
        Ok(node)
    }

    /// Fold initial value assignments.
    ///
    /// See section 2.4.3.2.
    fn fold_initial_value_assignment(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, E> {
        Ok(node)
    }

    /// Fold function declarations.
    ///
    /// See section 2.5.1.
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, E> {
        Ok(FunctionDeclaration {
            name: node.name.clone(),
            return_type: node.return_type,
            variables: Folder::fold(node.variables, self)?,
            body: node.body,
        })
    }

    /// Fold function block declarations.
    ///
    /// See section 2.5.2.
    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, E> {
        Ok(FunctionBlockDeclaration {
            name: node.name,
            variables: Folder::fold(node.variables, self)?,
            body: node.body,
            position: node.position,
        })
    }

    /// Fold program declarations.
    ///
    /// See section 2.5.3.
    dispatch!(ProgramDeclaration);

    dispatch!(Sfc);

    dispatch!(Network);

    leaf!(Step);

    leaf!(Transition);

    leaf!(Action);

    // 2.7.1
    dispatch!(ResourceDeclaration);
    // 2.7.1
    dispatch!(ProgramConfiguration);
    // 2.7.2
    dispatch!(ConfigurationDeclaration);
    // 2.7.2
    dispatch!(TaskConfiguration);

    dispatch!(Statements);

    leaf!(Assignment);
    leaf!(FbCall);
    leaf!(If);
    leaf!(Case);
    leaf!(For);
    leaf!(While);
    leaf!(Repeat);
}

fn fold_source_loc<F: Fold<E> + ?Sized, E>(f: &mut F, node: SourceLoc) -> Result<SourceLoc, E> {
    Ok(SourceLoc {
        start: node.start,
        end: node.end,
        file_id: node.file_id,
    })
}

fn fold_program_declaration<F: Fold<E> + ?Sized, E>(
    f: &mut F,
    node: ProgramDeclaration,
) -> Result<ProgramDeclaration, E> {
    Ok(ProgramDeclaration {
        type_name: f.fold_id(node.type_name)?,
        variables: Folder::fold(node.variables, f)?,
        body: Folder::fold(node.body, f)?,
    })
}

fn fold_sfc<F: Fold<E> + ?Sized, E>(f: &mut F, node: Sfc) -> Result<Sfc, E> {
    Ok(Sfc {
        networks: Folder::fold(node.networks, f)?,
    })
}

fn fold_network<F: Fold<E> + ?Sized, E>(f: &mut F, node: Network) -> Result<Network, E> {
    Ok(Network {
        initial_step: f.fold_step(node.initial_step)?,
        elements: Folder::fold(node.elements, f)?,
    })
}

fn fold_resource_declaration<F: Fold<E> + ?Sized, E>(
    f: &mut F,
    node: ResourceDeclaration,
) -> Result<ResourceDeclaration, E> {
    Ok(ResourceDeclaration {
        name: f.fold_id(node.name)?,
        resource: f.fold_id(node.resource)?,
        global_vars: Folder::fold(node.global_vars, f)?,
        tasks: Folder::fold(node.tasks, f)?,
        programs: Folder::fold(node.programs, f)?,
    })
}

fn fold_program_configuration<F: Fold<E> + ?Sized, E>(
    f: &mut F,
    node: ProgramConfiguration,
) -> Result<ProgramConfiguration, E> {
    Ok(ProgramConfiguration {
        name: f.fold_id(node.name)?,
        task_name: Folder::fold(node.task_name, f)?,
        type_name: f.fold_id(node.type_name)?,
    })
}

fn fold_configuration_declaration<F: Fold<E> + ?Sized, E>(
    f: &mut F,
    node: ConfigurationDeclaration,
) -> Result<ConfigurationDeclaration, E> {
    Ok(ConfigurationDeclaration {
        name: f.fold_id(node.name)?,
        global_var: Folder::fold(node.global_var, f)?,
        resource_decl: Folder::fold(node.resource_decl, f)?,
    })
}

fn fold_task_configuration<F: Fold<E> + ?Sized, E>(
    f: &mut F,
    node: TaskConfiguration,
) -> Result<TaskConfiguration, E> {
    Ok(TaskConfiguration {
        name: f.fold_id(node.name)?,
        priority: node.priority,
        interval: node.interval,
    })
}

fn fold_statements<F: Fold<E> + ?Sized, E>(f: &mut F, node: Statements) -> Result<Statements, E> {
    Ok(Statements {
        body: Folder::fold(node.body, f)?,
    })
}

impl Folder for Id {
    type Mapped = Id;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_id(self)
    }
}

impl Folder for LibraryElementKind {
    type Mapped = LibraryElementKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        match self {
            LibraryElementKind::DataTypeDeclaration(data_type) => {
                Ok(LibraryElementKind::DataTypeDeclaration(
                    folder.fold_data_type_declaration_kind(data_type)?,
                ))
            }
            LibraryElementKind::FunctionBlockDeclaration(function_block_decl) => {
                Ok(LibraryElementKind::FunctionBlockDeclaration(
                    folder.fold_function_block_declaration(function_block_decl)?,
                ))
            }
            LibraryElementKind::FunctionDeclaration(function_decl) => {
                Ok(LibraryElementKind::FunctionDeclaration(
                    folder.fold_function_declaration(function_decl)?,
                ))
            }
            LibraryElementKind::ProgramDeclaration(program_decl) => {
                Ok(LibraryElementKind::ProgramDeclaration(
                    folder.fold_program_declaration(program_decl)?,
                ))
            }
            LibraryElementKind::ConfigurationDeclaration(config_decl) => {
                Ok(LibraryElementKind::ConfigurationDeclaration(
                    folder.fold_configuration_declaration(config_decl)?,
                ))
            }
        }
    }
}

impl Folder for VarDecl {
    type Mapped = VarDecl;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_variable_declaration(self)
    }
}

impl Folder for InitialValueAssignmentKind {
    type Mapped = InitialValueAssignmentKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_initial_value_assignment(self)
    }
}

impl Folder for AddressAssignment {
    type Mapped = AddressAssignment;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_address_assignment(self)
    }
}

/// See section 2.7.1.
impl Folder for ResourceDeclaration {
    type Mapped = ResourceDeclaration;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_resource_declaration(self)
    }
}

/// See section 2.7.1.
impl Folder for ProgramConfiguration {
    type Mapped = ProgramConfiguration;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_program_configuration(self)
    }
}

/// See section 2.7.2.
impl Folder for TaskConfiguration {
    type Mapped = TaskConfiguration;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_task_configuration(self)
    }
}

impl Folder for FunctionBlockBodyKind {
    type Mapped = FunctionBlockBodyKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        match self {
            FunctionBlockBodyKind::Sfc(network) => {
                Ok(FunctionBlockBodyKind::Sfc(folder.fold_sfc(network)?))
            }
            FunctionBlockBodyKind::Statements(stmts) => Ok(FunctionBlockBodyKind::Statements(
                folder.fold_statements(stmts)?,
            )),
            // TODO it isn't clear if visiting this is necessary
            FunctionBlockBodyKind::Empty() => Ok(FunctionBlockBodyKind::Empty()),
        }
    }
}

impl Folder for Network {
    type Mapped = Network;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        folder.fold_network(self)
    }
}

impl Folder for ElementKind {
    type Mapped = ElementKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        match self {
            ElementKind::Step(step) => Ok(ElementKind::Step(folder.fold_step(step)?)),
            ElementKind::Transition(transition) => {
                Ok(ElementKind::Transition(folder.fold_transition(transition)?))
            }
            ElementKind::Action(action) => Ok(ElementKind::Action(folder.fold_action(action)?)),
        }
    }
}

impl Folder for StmtKind {
    type Mapped = StmtKind;
    fn fold<F: Fold<E> + ?Sized, E>(self, folder: &mut F) -> Result<Self::Mapped, E> {
        match self {
            StmtKind::Assignment(node) => Ok(StmtKind::Assignment(folder.fold_assignment(node)?)),
            StmtKind::FbCall(node) => Ok(StmtKind::FbCall(folder.fold_fb_call(node)?)),
            StmtKind::If(node) => Ok(StmtKind::If(folder.fold_if(node)?)),
            StmtKind::Case(node) => Ok(StmtKind::Case(folder.fold_case(node)?)),
            // TODO this
            StmtKind::For(node) => Ok(StmtKind::For(folder.fold_for(node)?)),
            StmtKind::While(node) => Ok(StmtKind::While(folder.fold_while(node)?)),
            StmtKind::Repeat(node) => Ok(StmtKind::Repeat(folder.fold_repeat(node)?)),
            StmtKind::Return => Ok(StmtKind::Return),
            StmtKind::Exit => Ok(StmtKind::Exit),
        }
    }
}
