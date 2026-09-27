//! Explicit, read-only methods callable from component template expressions.

use std::collections::HashMap;

use bevy::{app::App, ecs::resource::Resource};
use serde_json::Value;

/// A pure calculation over a serialized binding value and evaluated arguments.
///
/// The callback has no access to the Bevy `World`. It should depend only on its
/// inputs so bindings update predictably when the underlying store changes.
pub type HtmlExpressionMethod = fn(&Value, &[Value]) -> Option<Value>;

/// Registration emitted by `#[html_method("component", "root.path.method")]`.
pub struct HtmlMethodRegistration {
    pub component: &'static str,
    pub path: &'static str,
    pub evaluate: HtmlExpressionMethod,
}

inventory::collect!(HtmlMethodRegistration);

/// Allowlist of methods exposed to expressions in each component template.
#[derive(Resource, Default)]
pub struct UiExpressionMethods(HashMap<(&'static str, &'static str), HtmlExpressionMethod>);

impl UiExpressionMethods {
    /// Registers a method for one component and one exact receiver path.
    /// Returns `false` if that path was already registered.
    pub fn register(
        &mut self,
        component: &'static str,
        path: &'static str,
        evaluate: HtmlExpressionMethod,
    ) -> bool {
        match self.0.entry((component, path)) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(evaluate);
                true
            }
            std::collections::hash_map::Entry::Occupied(_) => false,
        }
    }

    pub(crate) fn evaluate(
        &self,
        component: &str,
        path: &str,
        receiver: &Value,
        arguments: &[Value],
    ) -> Option<Value> {
        self.0.get(&(component, path))?(receiver, arguments)
    }
}

pub(super) fn install(app: &mut App) {
    let mut methods = UiExpressionMethods::default();
    for registration in inventory::iter::<HtmlMethodRegistration> {
        assert!(
            methods.register(
                registration.component,
                registration.path,
                registration.evaluate,
            ),
            "duplicate HTML expression method {} for component {}",
            registration.path,
            registration.component,
        );
    }
    app.insert_resource(methods);
}
