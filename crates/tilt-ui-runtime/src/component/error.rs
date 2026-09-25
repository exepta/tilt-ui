use thiserror::Error;
use tilt_ui_core::{ComponentId, NodeId};

/// Describes a failure while instantiating a TiltUI component template.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ComponentInstantiationError {
    /// A custom component tag could not be resolved by the generated manifest.
    #[error("unknown component `{name}`")]
    UnknownComponent {
        /// Unresolved custom component name.
        name: String,
    },

    /// Generated metadata did not contain the requested component identifier.
    #[error("component metadata is missing for component {component:?}")]
    ComponentMetadataMissing {
        /// Identifier missing from the static component metadata.
        component: ComponentId,
    },

    /// Asset handles have not been prepared for the requested component.
    #[error("component assets are unavailable for component {component:?}")]
    ComponentAssetsUnavailable {
        /// Component whose handles were unavailable.
        component: ComponentId,
    },

    /// The requested template asset is not ready in Bevy's asset storage.
    #[error("template asset is unavailable for component {component:?}")]
    TemplateAssetUnavailable {
        /// Component whose template asset was unavailable.
        component: ComponentId,
    },

    /// Bevy reported that the requested template asset failed to load.
    #[error("template asset failed to load for component {component:?}")]
    TemplateAssetFailed {
        /// Component whose template asset failed.
        component: ComponentId,
    },

    /// A template node identifier did not reference a node owned by its template.
    #[error("invalid template node reference {node:?}")]
    InvalidTemplateNode {
        /// Invalid template-local node identifier.
        node: NodeId,
    },

    /// A custom component reference formed an instantiation cycle.
    #[error("component cycle detected: {cycle:?}")]
    ComponentCycle {
        /// Component identifiers in cycle traversal order.
        cycle: Vec<ComponentId>,
    },
}
