//! Where each user function's, function block's and method's variables are,
//! which their bodies are compiled against: the layout built from the
//! lowered declarations gives one for each body ([`super::declarations`]).

use ironplc_container::{FunctionId, VarIndex};

use super::names::ScopeId;
use crate::compile::{OpType, StringReturnInfo};

/// Where a user function's variables are, which its body is compiled
/// against.
pub(crate) struct FunctionLayout {
    /// The function's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the function's variables are recorded under, which its
    /// body is compiled in.
    pub(crate) scope: ScopeId,
    /// How many slots the function's run takes.
    pub(crate) num_locals: u16,
    /// How many of them, at its start, hold the function's parameters.
    pub(crate) num_params: u16,
    /// The slot of the function's result.
    pub(crate) return_var_index: VarIndex,
    /// The type the result is loaded at: the declared return type, a
    /// subrange at its base type. A string or structure result lives in the
    /// data region, and its slot holds a data offset at the default type.
    pub(crate) return_op_type: OpType,
    /// Where in the data region the result is, when it is a string.
    pub(crate) return_string_info: Option<StringReturnInfo>,
}

/// Where a user function block's copy slots and its methods' variables
/// are, which their bodies are compiled against.
pub(crate) struct BlockLayout {
    /// The block body's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the block's copy slots are recorded under, which its body
    /// is compiled in.
    pub(crate) scope: ScopeId,
    /// How many copy slots the block takes, one for each field.
    pub(crate) num_locals: u16,
    /// The layout of each of the block's methods, in declaration order.
    pub(crate) methods: Vec<MethodLayout>,
}

/// Where a method's variables are, which its body is compiled against.
pub(crate) struct MethodLayout {
    /// The method's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the method's variables are recorded under, which its body
    /// is compiled in.
    pub(crate) scope: ScopeId,
    /// The slots the VM's frame for the method spans, from the first of
    /// its block's copy slots through the method's result: the copy slots,
    /// the earlier methods' slots and the method's own.
    pub(crate) num_locals: u16,
    /// How many input parameters the method takes.
    pub(crate) num_params: u16,
    /// The slot of the method's result, which a method without a return
    /// type has too.
    pub(crate) return_var_index: VarIndex,
    /// The type the result is loaded at.
    pub(crate) return_op_type: OpType,
    /// Whether the method declares a return type.
    pub(crate) has_return_value: bool,
}
