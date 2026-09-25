use bevy::prelude::Resource;
use tilt_ui_core::{ComponentId, ComponentMetadata};

/// Static generated component metadata and lookup functions compiled into an application.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ComponentCatalog {
    components: &'static [ComponentMetadata],
    component_id: fn(&str) -> Option<ComponentId>,
    component_metadata: fn(ComponentId) -> Option<&'static ComponentMetadata>,
}

impl ComponentCatalog {
    /// Creates a catalog backed by generated static component metadata and lookup functions.
    pub const fn new(
        components: &'static [ComponentMetadata],
        component_id: fn(&str) -> Option<ComponentId>,
        component_metadata: fn(ComponentId) -> Option<&'static ComponentMetadata>,
    ) -> Self {
        Self {
            components,
            component_id,
            component_metadata,
        }
    }

    /// Returns static metadata for every component compiled into the application.
    pub fn components(&self) -> &'static [ComponentMetadata] {
        self.components
    }

    /// Resolves a custom component tag through the generated lookup function.
    pub fn component_id(&self, name: &str) -> Option<ComponentId> {
        (self.component_id)(name)
    }

    /// Resolves component metadata through the generated identifier lookup function.
    pub fn component_metadata(&self, component: ComponentId) -> Option<&'static ComponentMetadata> {
        (self.component_metadata)(component)
    }
}

fn empty_component_id(_: &str) -> Option<ComponentId> {
    None
}

fn empty_component_metadata(_: ComponentId) -> Option<&'static ComponentMetadata> {
    None
}

impl Default for ComponentCatalog {
    fn default() -> Self {
        Self::new(&[], empty_component_id, empty_component_metadata)
    }
}
