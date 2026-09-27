use bevy::{
    asset::Handle,
    ecs::{bundle::Bundle, component::Component},
    ui::experimental::GhostNode,
};
use tilt_ui_core::ComponentId;

use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::{ComponentInstantiationError, UiState};

/// Identifies the compile-time component definition instantiated by an entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentInstance {
    /// Identifier of the component definition represented by this instance.
    pub component: ComponentId,
}

/// Marks the layout-neutral boundary entity that owns an instantiated TiltUI component tree.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentRoot {
    /// Identifier of the component definition rooted at this entity.
    pub component: ComponentId,
}

/// Bundles the layout-neutral ownership components of an instantiated component.
///
/// `GhostNode` is isolated here because it is an experimental Bevy UI API.
#[derive(Bundle)]
pub(crate) struct ComponentBoundary {
    instance: ComponentInstance,
    root: ComponentRoot,
    ghost: GhostNode,
    state: UiState,
}

impl ComponentBoundary {
    /// Creates a layout-neutral ownership boundary for a component instance.
    pub(crate) fn new(component: ComponentId) -> Self {
        Self {
            instance: ComponentInstance { component },
            root: ComponentRoot { component },
            ghost: GhostNode,
            state: UiState::default(),
        }
    }
}

/// Associates a component boundary with the assets used to create its tree.
#[derive(Component, Debug, Clone)]
pub struct ComponentAssetHandles {
    /// Handle to the parsed component template asset.
    pub template: Handle<UiTemplateAsset>,
    /// Handle to the parsed component stylesheet asset.
    pub stylesheet: Handle<UiStyleSheetAsset>,
    /// Further author stylesheets in cascade order.
    pub additional_stylesheets: Vec<Handle<UiStyleSheetAsset>>,
}

/// Marks a component boundary awaiting its template asset before instantiation.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingComponent {
    /// Identifier of the component awaiting instantiation.
    pub component: ComponentId,
}

/// Records a structured instantiation failure on a component boundary.
#[derive(Component, Debug, Clone)]
pub struct FailedComponentInstantiation {
    /// Error that prevented the component template from being instantiated.
    pub error: ComponentInstantiationError,
}

/// Marks a component instance whose stylesheet needs to be recomputed.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StyleDirty;
