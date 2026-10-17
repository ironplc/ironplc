//! The signatures of the properties of function blocks and interfaces.
//!
//! A property's signature is what a property that provides an interface's
//! must match: its type, by id, and which of the `GET` and `SET` accessors
//! it has. It is recorded with the block that declares the property (see
//! [`TypeEnvironment::record_block`]) or with the interface that declares
//! its prototype (see [`TypeEnvironment::record_interface`]).

use ironplc_dsl::common::{PropertyDeclaration, PropertyPrototype};
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

use super::TypeEnvironment;

/// The signature of a property a function block or an interface declares.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PropertySignature {
    /// The property's name, as the block or interface declares it.
    pub(crate) name: Id,
    /// The id of the property's type; `None` when it did not resolve, which
    /// a rule reports.
    pub(crate) property_type: Option<TypeId>,
    /// Whether the property has a `GET` accessor.
    pub(crate) get: bool,
    /// Whether the property has a `SET` accessor.
    pub(crate) set: bool,
}

impl TypeEnvironment {
    /// The signature of `property`. A type that is a string of a stated
    /// length enters a sized string type, as [`Self::result_type_id`] does
    /// for a method's result.
    pub(crate) fn property_signature(
        &mut self,
        property: &PropertyDeclaration,
    ) -> PropertySignature {
        PropertySignature {
            name: property.name.clone(),
            property_type: self.result_type_id(&property.property_type),
            get: property.get.is_some(),
            set: property.set.is_some(),
        }
    }

    /// Whether the property `property` provides the property prototype
    /// `prototype`: it has the same type, compared by
    /// [`Self::same_type_if_resolved`], and every accessor the prototype
    /// has. An extra accessor is allowed.
    pub(crate) fn provides_property(
        &self,
        property: &PropertySignature,
        prototype: &PropertySignature,
    ) -> bool {
        self.same_type_if_resolved(property.property_type, prototype.property_type)
            && (property.get || !prototype.get)
            && (property.set || !prototype.set)
    }

    /// The signature of the property prototype `prototype` an interface
    /// declares, as [`Self::property_signature`] gives a property's.
    pub(crate) fn property_prototype_signature(
        &mut self,
        prototype: &PropertyPrototype,
    ) -> PropertySignature {
        PropertySignature {
            name: prototype.name.clone(),
            property_type: self.result_type_id(&prototype.property_type),
            get: prototype.get.is_some(),
            set: prototype.set.is_some(),
        }
    }
}
