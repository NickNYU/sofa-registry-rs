pub mod constants;
pub mod error;
pub mod model;
pub mod slot;

/// Generated protobuf types.
///
/// The module hierarchy mirrors the proto package structure so that
/// tonic-generated `super::` references resolve correctly:
///   sofa.registry         -> pb::sofa::registry
///   sofa.registry.meta    -> pb::sofa::registry::meta
///   sofa.registry.data    -> pb::sofa::registry::data
///   sofa.registry.session -> pb::sofa::registry::session
pub mod pb {
    pub mod sofa {
        pub mod registry {
            tonic::include_proto!("sofa.registry");

            pub mod meta {
                tonic::include_proto!("sofa.registry.meta");
            }
            pub mod data {
                tonic::include_proto!("sofa.registry.data");
            }
            pub mod session {
                tonic::include_proto!("sofa.registry.session");
            }
        }
    }
}
