//! Runtime component instances and semantic template instantiation.

mod assets;
mod binding;
pub(crate) mod binding_runtime;
mod content;
pub(crate) mod document;
mod element;
mod error;
mod expression;
pub(crate) mod flow;
mod handlers;
#[cfg(feature = "hot-reload")]
mod hot_reload;
mod inline_actions;
mod instance;
mod method;
mod plugin;
mod resolver;
mod router;
mod spawn;

pub use assets::{ComponentAssetStore, LoadedComponentAssets};
pub use binding::{
    SharedValueRegistration, UiBindingStore, UiSharedValues, UiStore, UiStoreRegistration,
};
pub use content::{InnerContentError, set_inner_bindings, set_inner_html, set_inner_text};
pub use document::UiDocumentInfo;
pub use element::{
    ComponentElementIds, ComponentStyleOwner, ElementClasses, ElementId, ElementState,
    EventBinding, EventBindings, PropertyBinding, PropertyBindings, StaticAttribute,
    StaticAttributes, TemplateNodeRef, TiltElement, TiltText,
};
pub(crate) use element::{
    TemplateImports, boolean_attribute_value, has_boolean_static_attribute, static_attribute_value,
};
pub use error::ComponentInstantiationError;
pub use handlers::{
    ComponentInitRegistration, ComponentUpdateRegistration, HtmlChange, HtmlClick, HtmlEvent,
    HtmlHandlerRegistration, HtmlSubmit, TiltUiCodePlugin,
};
pub(crate) use instance::ComponentBoundary;
pub use instance::{
    ComponentAssetHandles, ComponentInstance, ComponentRoot, FailedComponentInstantiation,
    PendingComponent, StyleDirty,
};
pub use method::{HtmlExpressionMethod, HtmlMethodRegistration, UiExpressionMethods};
pub use plugin::{TiltUiComponentRuntimePlugin, TiltUiComponentRuntimeSet, spawn_component};
pub use resolver::ComponentCatalog;
pub use router::{RouteTarget, Router, Routes, RoutesRegistration, TiltUiRouterPlugin};
pub use spawn::instantiate_component;
