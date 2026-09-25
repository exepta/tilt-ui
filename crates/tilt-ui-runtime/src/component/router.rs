//! Component routing through a layout-neutral `router-outlet`.

use bevy::{
    app::{App, Plugin, Update},
    ecs::{component::Component, entity::Entity, resource::Resource, world::World},
    prelude::IntoScheduleConfigs,
};
use tilt_ui_core::{ComponentId, ElementKind};

use super::{ComponentBoundary, PendingComponent, TiltElement};

/// Destination selected by a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteTarget {
    Component(ComponentId),
    Redirect(String),
}

/// Static route definitions built in Rust code.
#[derive(Debug, Clone, Default)]
pub struct Routes {
    entries: Vec<(String, RouteTarget)>,
    fallback: Option<ComponentId>,
}

impl Routes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(mut self, path: impl Into<String>, component: ComponentId) -> Self {
        self.entries
            .push((path.into(), RouteTarget::Component(component)));
        self
    }

    pub fn redirect(mut self, path: impl Into<String>, destination: impl Into<String>) -> Self {
        self.entries
            .push((path.into(), RouteTarget::Redirect(destination.into())));
        self
    }

    pub fn fallback(mut self, component: ComponentId) -> Self {
        self.fallback = Some(component);
        self
    }

    fn resolve(&self, path: &str) -> Option<ComponentId> {
        let mut current = path;
        for _ in 0..=self.entries.len() {
            match self
                .entries
                .iter()
                .find(|(route, _)| route == current)
                .map(|(_, target)| target)
            {
                Some(RouteTarget::Component(component)) => return Some(*component),
                Some(RouteTarget::Redirect(destination)) => current = destination,
                None => return self.fallback,
            }
        }
        self.fallback
    }
}

/// Registration emitted by `#[beu_routes]`.
pub struct RoutesRegistration {
    pub build: fn() -> Routes,
}

inventory::collect!(RoutesRegistration);

/// Current route path and route definitions.
#[derive(Resource, Debug, Clone)]
pub struct Router {
    routes: Routes,
    path: String,
    revision: u64,
}

impl Router {
    pub fn new(routes: Routes) -> Self {
        Self {
            routes,
            path: "/".into(),
            revision: 1,
        }
    }

    pub fn navigate(&mut self, path: impl Into<String>) {
        let path = path.into();
        if self.path != path {
            self.path = path;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn target(&self) -> Option<ComponentId> {
        self.routes.resolve(&self.path)
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct RouterOutletState {
    target: Option<ComponentId>,
    instance: Option<Entity>,
}

/// Installs registered route tables and keeps outlets synchronized with `Router`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiltUiRouterPlugin;

impl Plugin for TiltUiRouterPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Router>() {
            let mut routes = Routes::new();
            for registration in inventory::iter::<RoutesRegistration> {
                let registered = (registration.build)();
                routes.entries.extend(registered.entries);
                routes.fallback = registered.fallback.or(routes.fallback);
            }
            app.insert_resource(Router::new(routes));
        }
        app.add_systems(
            Update,
            sync_outlets.after(super::TiltUiComponentRuntimeSet::Instantiate),
        );
    }
}

fn sync_outlets(world: &mut World) {
    let target = world.resource::<Router>().target();
    let outlets = {
        let mut query = world.query::<(Entity, &TiltElement, Option<&RouterOutletState>)>();
        query
            .iter(world)
            .filter_map(|(entity, element, state)| {
                (element.kind == ElementKind::RouterOutlet).then_some((entity, state.copied()))
            })
            .collect::<Vec<_>>()
    };
    for (outlet, state) in outlets {
        if state.is_some_and(|state| {
            state.target == target
                && state
                    .instance
                    .is_none_or(|entity| world.get_entity(entity).is_ok())
        }) {
            continue;
        }
        if let Some(instance) = state.and_then(|state| state.instance) {
            world.despawn(instance);
        }
        let instance = target.map(|component| {
            let entity = world
                .spawn((
                    ComponentBoundary::new(component),
                    PendingComponent { component },
                ))
                .id();
            world.entity_mut(outlet).add_child(entity);
            entity
        });
        world
            .entity_mut(outlet)
            .insert(RouterOutletState { target, instance });
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;

    use super::*;

    #[test]
    fn redirects_fallbacks_and_route_changes_replace_outlet_content() {
        let first = ComponentId(1);
        let second = ComponentId(2);
        let routes = Routes::new()
            .route("/", first)
            .route("/settings", second)
            .redirect("/old", "/settings")
            .fallback(first);
        let mut world = World::new();
        world.insert_resource(Router::new(routes));
        let outlet = world
            .spawn(TiltElement {
                kind: ElementKind::RouterOutlet,
            })
            .id();
        sync_outlets(&mut world);
        let first_instance = world
            .get::<RouterOutletState>(outlet)
            .unwrap()
            .instance
            .unwrap();
        assert_eq!(
            world
                .get::<PendingComponent>(first_instance)
                .unwrap()
                .component,
            first
        );

        world.resource_mut::<Router>().navigate("/old");
        sync_outlets(&mut world);
        let second_instance = world
            .get::<RouterOutletState>(outlet)
            .unwrap()
            .instance
            .unwrap();
        assert_ne!(first_instance, second_instance);
        assert!(world.get_entity(first_instance).is_err());
        assert_eq!(
            world
                .get::<PendingComponent>(second_instance)
                .unwrap()
                .component,
            second
        );

        world.resource_mut::<Router>().navigate("/unknown");
        sync_outlets(&mut world);
        assert_eq!(
            world.get::<RouterOutletState>(outlet).unwrap().target,
            Some(first)
        );
    }
}
