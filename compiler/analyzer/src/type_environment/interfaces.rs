//! The members of interfaces, by the id of their type.
//!
//! An interface's type (`SemanticType::Interface`) lists none of its
//! members: the signatures of the method and property prototypes it
//! declares are recorded here, once, from the declaration whose type the
//! environment keeps (see [`TypeEnvironment::record_interface`]). The
//! interfaces it `EXTENDS` are in the supertype relation
//! ([`crate::supertypes`]), not here.

use ironplc_dsl::type_id::TypeId;

use super::methods::MethodSignature;
use super::properties::PropertySignature;
use super::TypeEnvironment;

/// What the environment records about an interface.
#[derive(Debug)]
pub(super) struct InterfaceRecord {
    /// The signatures of the method prototypes the interface declares, in
    /// the order it declares them; not those of the interfaces it extends.
    methods: Vec<MethodSignature>,
    /// The signatures of the property prototypes the interface declares, in
    /// the order it declares them; not those of the interfaces it extends.
    properties: Vec<PropertySignature>,
}

impl TypeEnvironment {
    /// Records the signatures of the method and property prototypes the
    /// interface `interface` declares.
    ///
    /// An interface is recorded once: a later call for the same interface,
    /// such as for a repeated declaration of its name, records nothing.
    /// Nothing is recorded for a type that is not an interface.
    pub(crate) fn record_interface(
        &mut self,
        interface: TypeId,
        methods: Vec<MethodSignature>,
        properties: Vec<PropertySignature>,
    ) {
        let is_interface = self
            .get_by_id(interface)
            .is_some_and(|attributes| attributes.representation.is_interface());
        if !is_interface || self.interfaces.contains_key(&interface) {
            return;
        }
        self.interfaces.insert(
            interface,
            InterfaceRecord {
                methods,
                properties,
            },
        );
    }

    /// The signatures of the method prototypes the interface `interface`
    /// declares; not those of the interfaces it extends. `None` when
    /// `interface` is not a recorded interface.
    pub(crate) fn interface_methods(&self, interface: TypeId) -> Option<&[MethodSignature]> {
        Some(&self.interfaces.get(&interface)?.methods)
    }

    /// The signatures of the property prototypes the interface `interface`
    /// declares; not those of the interfaces it extends. `None` when
    /// `interface` is not a recorded interface.
    pub(crate) fn interface_properties(&self, interface: TypeId) -> Option<&[PropertySignature]> {
        Some(&self.interfaces.get(&interface)?.properties)
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::{TypeName, VariableType};
    use ironplc_dsl::core::Id;

    use crate::semantic_context::SemanticContext;
    use crate::semantic_type::SemanticType;
    use crate::test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options};
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::methods::{MethodParameter, MethodResult};
    use crate::type_environment::properties::PropertySignature;
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};

    fn resolve(program: &str) -> SemanticContext {
        parse_and_resolve_types_with_options(program, &fb_inheritance_options()).1
    }

    fn id(types: &TypeEnvironment, name: &str) -> ironplc_dsl::type_id::TypeId {
        types.id_of(&TypeName::from(name)).unwrap()
    }

    const INTERFACES: &str = "
INTERFACE I_Base
METHOD Reset
END_METHOD
END_INTERFACE
INTERFACE I_Comm EXTENDS I_Base
METHOD Send : BOOL
VAR_INPUT data : INT; END_VAR
VAR_OUTPUT sent : UINT; END_VAR
END_METHOD
PROPERTY Ready : BOOL
GET END_GET
END_PROPERTY
END_INTERFACE
FUNCTION_BLOCK FB_Serial
END_FUNCTION_BLOCK";

    #[test]
    fn interface_methods_when_interface_declares_methods_then_their_signatures() {
        let context = resolve(INTERFACES);
        let types = context.types();

        let methods = types.interface_methods(id(types, "I_Comm")).unwrap();

        assert_eq!(1, methods.len());
        assert_eq!(Id::from("Send"), methods[0].name);
        assert_eq!(
            MethodResult::Value(Some(id(types, "BOOL"))),
            methods[0].result
        );
        assert_eq!(
            vec![
                MethodParameter {
                    name: Id::from("data"),
                    section: VariableType::Input,
                    type_id: Some(id(types, "INT")),
                },
                MethodParameter {
                    name: Id::from("sent"),
                    section: VariableType::Output,
                    type_id: Some(id(types, "UINT")),
                },
            ],
            methods[0].parameters
        );
    }

    #[test]
    fn interface_methods_when_block_declares_method_alike_then_same_signature() {
        let context = resolve(
            "
INTERFACE I_Store
METHOD Put : INT
VAR_INPUT key : DINT; END_VAR
VAR_IN_OUT value : BOOL; END_VAR
END_METHOD
END_INTERFACE
FUNCTION_BLOCK FB_Store
METHOD Put : INT
VAR_INPUT key : DINT; END_VAR
VAR_IN_OUT value : BOOL; END_VAR
VAR local : INT; END_VAR
  Put := 1;
END_METHOD
END_FUNCTION_BLOCK",
        );
        let types = context.types();

        let prototype = &types.interface_methods(id(types, "I_Store")).unwrap()[0];
        let (_, method) = types
            .method(id(types, "FB_Store"), &Id::from("Put"))
            .unwrap();

        assert_eq!(prototype, method);
    }

    #[test]
    fn interface_methods_when_extends_then_only_its_own() {
        let context = resolve(INTERFACES);
        let types = context.types();

        let methods = types.interface_methods(id(types, "I_Base")).unwrap();

        assert_eq!(1, methods.len());
        assert_eq!(Id::from("Reset"), methods[0].name);
        assert_eq!(MethodResult::None, methods[0].result);
    }

    #[test]
    fn interface_properties_when_interface_declares_property_then_its_signature() {
        let context = resolve(INTERFACES);
        let types = context.types();

        let properties = types.interface_properties(id(types, "I_Comm")).unwrap();

        assert_eq!(1, properties.len());
        assert_eq!(Id::from("Ready"), properties[0].name);
        assert_eq!(Some(id(types, "BOOL")), properties[0].property_type);
        assert!(properties[0].get);
        assert!(!properties[0].set);
    }

    #[test]
    fn record_interface_when_recorded_twice_then_first_kept() {
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let name = TypeName::from("I_X");
        env.insert_type(
            &name,
            TypeAttributes::new(SemanticType::Interface {
                name: "I_X".to_string(),
            }),
        );
        let interface = env.id_of(&name).unwrap();
        let kept = PropertySignature {
            name: Id::from("Kept"),
            property_type: None,
            get: true,
            set: false,
        };
        let dropped = PropertySignature {
            name: Id::from("Dropped"),
            ..kept.clone()
        };

        env.record_interface(interface, vec![], vec![kept.clone()]);
        env.record_interface(interface, vec![], vec![dropped]);

        assert_eq!(Some([kept].as_slice()), env.interface_properties(interface));
    }

    #[test]
    fn interface_methods_when_not_interface_then_none() {
        let context = resolve(INTERFACES);
        let types = context.types();

        assert!(types.interface_methods(id(types, "FB_Serial")).is_none());
        assert!(types.interface_properties(id(types, "FB_Serial")).is_none());
    }
}
