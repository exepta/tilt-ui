use bevy::{asset::Handle, prelude::Resource};
use tilt_ui_core::ComponentId;

use crate::{UiStyleSheetAsset, UiTemplateAsset};

/// Holds the template and stylesheet handles loaded for one component definition.
#[derive(Debug, Clone)]
pub struct LoadedComponentAssets {
    /// Handle to the component's parsed HTML template asset.
    pub template: Handle<UiTemplateAsset>,
    /// Handle to the component's parsed CSS stylesheet asset.
    pub stylesheet: Handle<UiStyleSheetAsset>,
}

/// Stores loaded component asset handles by deterministic `ComponentId` index.
#[derive(Resource, Debug, Clone, Default)]
pub struct ComponentAssetStore {
    assets: Vec<Option<LoadedComponentAssets>>,
}

impl ComponentAssetStore {
    /// Returns loaded handles for a component definition when they have been prepared.
    pub fn get(&self, component: ComponentId) -> Option<&LoadedComponentAssets> {
        self.assets
            .get(component.0 as usize)
            .and_then(Option::as_ref)
    }

    /// Stores handles under the supplied deterministic component identifier.
    pub fn insert(&mut self, component: ComponentId, assets: LoadedComponentAssets) {
        let index = component.0 as usize;
        if self.assets.len() <= index {
            self.assets.resize_with(index + 1, || None);
        }
        self.assets[index] = Some(assets);
    }
}
