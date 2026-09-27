//! Component routing through a layout-neutral `router-outlet`.

use std::collections::BTreeMap;

use bevy::{
    app::{App, Plugin, Update},
    ecs::{component::Component, entity::Entity, resource::Resource, world::World},
    prelude::{IntoScheduleConfigs, Visibility},
    ui::{Display, Node},
};
use tilt_ui_core::{ComponentId, ElementKind};

use super::{ComponentBoundary, PendingComponent, TiltElement};

/// Destination selected by a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteTarget {
    Component(ComponentId),
    Managed(RouteComponent),
    Redirect(String),
}

/// Route instance creation and retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteLifetime {
    Transient,
    Load,
    Lazy,
}

/// Component destination with its lifetime policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteComponent {
    pub component: ComponentId,
    pub lifetime: RouteLifetime,
}

impl RouteComponent {
    pub const fn load(component: ComponentId) -> Self {
        Self {
            component,
            lifetime: RouteLifetime::Load,
        }
    }

    pub const fn lazy(component: ComponentId) -> Self {
        Self {
            component,
            lifetime: RouteLifetime::Lazy,
        }
    }
}

impl From<ComponentId> for RouteComponent {
    fn from(component: ComponentId) -> Self {
        Self {
            component,
            lifetime: RouteLifetime::Transient,
        }
    }
}

/// Normalizes leading, trailing and repeated slashes in route paths.
pub fn normalize_path(path: &str) -> String {
    let mut normalized = String::new();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        normalized.push('/');
        normalized.push_str(segment);
    }
    if normalized.is_empty() {
        normalized.push('/');
    }
    normalized
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRoute {
    key: String,
    destination: RouteComponent,
}

/// Static route definitions built in Rust code.
#[derive(Debug, Clone, Default)]
pub struct Routes {
    entries: Vec<(String, RouteTarget)>,
    fallback: Option<RouteComponent>,
}

impl Routes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn route(mut self, path: impl AsRef<str>, component: impl Into<RouteComponent>) -> Self {
        let component = component.into();
        let target = if component.lifetime == RouteLifetime::Transient {
            RouteTarget::Component(component.component)
        } else {
            RouteTarget::Managed(component)
        };
        self.insert(path.as_ref(), target);
        self
    }

    pub fn redirect(mut self, path: impl AsRef<str>, destination: impl AsRef<str>) -> Self {
        self.insert(
            path.as_ref(),
            RouteTarget::Redirect(normalize_path(destination.as_ref())),
        );
        self
    }

    pub fn fallback(mut self, component: impl Into<RouteComponent>) -> Self {
        self.fallback = Some(component.into());
        self
    }

    /// Merge another table, giving its entries precedence for duplicate paths.
    pub fn merge(mut self, other: Self) -> Self {
        for (path, target) in other.entries {
            self.insert(&path, target);
        }
        if other.fallback.is_some() {
            self.fallback = other.fallback;
        }
        self
    }

    fn insert(&mut self, path: &str, target: RouteTarget) {
        let path = normalize_path(path);
        if let Some((_, existing)) = self.entries.iter_mut().find(|(entry, _)| *entry == path) {
            *existing = target;
        } else {
            self.entries.push((path, target));
        }
    }

    fn resolve(&self, path: &str) -> Option<ResolvedRoute> {
        let mut current = normalize_path(path);
        for _ in 0..=self.entries.len() {
            match self
                .entries
                .iter()
                .find(|(route, _)| *route == current)
                .map(|(_, target)| target)
            {
                Some(RouteTarget::Component(component)) => {
                    return Some(ResolvedRoute {
                        key: current,
                        destination: (*component).into(),
                    });
                }
                Some(RouteTarget::Managed(destination)) => {
                    return Some(ResolvedRoute {
                        key: current,
                        destination: *destination,
                    });
                }
                Some(RouteTarget::Redirect(destination)) => current = destination.clone(),
                None => {
                    return self.fallback.map(|destination| ResolvedRoute {
                        key: "\0fallback".into(),
                        destination,
                    });
                }
            }
        }
        self.fallback.map(|destination| ResolvedRoute {
            key: "\0fallback".into(),
            destination,
        })
    }
}

/// Registration emitted by `#[ui_routes]`.
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

    pub fn navigate(&mut self, path: impl AsRef<str>) {
        let path = normalize_path(path.as_ref());
        if self.path != path {
            self.path = path;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn target(&self) -> Option<ComponentId> {
        self.routes
            .resolve(&self.path)
            .map(|route| route.destination.component)
    }

    /// Replace route definitions while preserving the current path.
    pub fn set_routes(&mut self, routes: Routes) {
        self.routes = routes;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Add or override route definitions at runtime.
    pub fn merge(&mut self, routes: Routes) {
        self.routes = std::mem::take(&mut self.routes).merge(routes);
        self.revision = self.revision.wrapping_add(1);
    }
}

#[derive(Component, Debug, Clone, Default)]
struct RouterOutletState {
    instances: BTreeMap<String, (RouteComponent, Entity)>,
    parking: Option<Entity>,
    revision: u64,
}

/// Installs registered route tables and keeps outlets synchronized with `Router`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiltUiRouterPlugin;

impl Plugin for TiltUiRouterPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Router>() {
            let mut routes = Routes::new();
            for registration in inventory::iter::<RoutesRegistration> {
                routes = routes.merge((registration.build)());
            }
            app.insert_resource(Router::new(routes));
        }
        app.add_systems(
            Update,
            sync_outlets
                .after(super::TiltUiComponentRuntimeSet::Instantiate)
                .after(crate::style::TiltUiStyleRuntimeSet::Apply),
        );
    }
}

pub(crate) fn sync_outlets(world: &mut World) {
    let router = world.resource::<Router>().clone();
    let target = router.routes.resolve(&router.path);
    let outlets = {
        let mut query = world.query::<(Entity, &TiltElement, Option<&RouterOutletState>)>();
        query
            .iter(world)
            .filter_map(|(entity, element, state)| {
                (element.kind == ElementKind::RouterOutlet).then(|| (entity, state.cloned()))
            })
            .collect::<Vec<_>>()
    };
    for (outlet, previous) in outlets {
        let mut state = previous.unwrap_or_default();
        if state.revision == router.revision
            && state
                .parking
                .is_some_and(|entity| world.get_entity(entity).is_ok())
            && state
                .instances
                .values()
                .all(|(_, entity)| world.get_entity(*entity).is_ok())
        {
            continue;
        }
        let parking = state
            .parking
            .filter(|entity| world.get_entity(*entity).is_ok())
            .unwrap_or_else(|| {
                let entity = world
                    .spawn(Node {
                        display: Display::None,
                        ..Default::default()
                    })
                    .id();
                world.entity_mut(outlet).add_child(entity);
                entity
            });
        state.parking = Some(parking);
        for (key, (destination, entity)) in state.instances.clone() {
            let definition = if key == "\0fallback" {
                router.routes.fallback
            } else {
                router
                    .routes
                    .entries
                    .iter()
                    .find(|(path, _)| *path == key)
                    .and_then(|(_, target)| match target {
                        RouteTarget::Component(component) => Some((*component).into()),
                        RouteTarget::Managed(component) => Some(*component),
                        RouteTarget::Redirect(_) => None,
                    })
            };
            if definition != Some(destination) || world.get_entity(entity).is_err() {
                if world.get_entity(entity).is_ok() {
                    super::state::hide_before_despawn(world, entity);
                    world.despawn(entity);
                }
                state.instances.remove(&key);
            }
        }
        let active = target.as_ref().map(|route| route.key.as_str());
        for (key, (destination, entity)) in state.instances.clone() {
            if active == Some(key.as_str()) {
                world.entity_mut(outlet).add_child(entity);
                world.entity_mut(entity).insert(Visibility::Inherited);
            } else if destination.lifetime == RouteLifetime::Transient {
                super::state::hide_before_despawn(world, entity);
                world.despawn(entity);
                state.instances.remove(&key);
            } else {
                world.entity_mut(parking).add_child(entity);
                world.entity_mut(entity).insert(Visibility::Hidden);
            }
        }
        for (key, route) in &router.routes.entries {
            if let RouteTarget::Managed(destination) = route
                && destination.lifetime == RouteLifetime::Load
                && !state.instances.contains_key(key)
            {
                let entity = spawn_route(
                    world,
                    if active == Some(key) { outlet } else { parking },
                    *destination,
                    active == Some(key),
                );
                state.instances.insert(key.clone(), (*destination, entity));
            }
        }
        if let Some(destination) = router.routes.fallback
            && destination.lifetime == RouteLifetime::Load
            && !state.instances.contains_key("\0fallback")
        {
            let entity = spawn_route(
                world,
                if active == Some("\0fallback") {
                    outlet
                } else {
                    parking
                },
                destination,
                active == Some("\0fallback"),
            );
            state
                .instances
                .insert("\0fallback".into(), (destination, entity));
        }
        if let Some(route) = target.as_ref()
            && !state.instances.contains_key(&route.key)
        {
            let entity = spawn_route(world, outlet, route.destination, true);
            state
                .instances
                .insert(route.key.clone(), (route.destination, entity));
        }
        state.revision = router.revision;
        world.entity_mut(outlet).insert(state);
    }
}

fn spawn_route(world: &mut World, parent: Entity, route: RouteComponent, active: bool) -> Entity {
    let entity = world
        .spawn((
            ComponentBoundary::new(route.component),
            PendingComponent {
                component: route.component,
            },
            if active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ))
        .id();
    world.entity_mut(parent).add_child(entity);
    entity
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{hierarchy::ChildOf, message::Messages, world::World};

    use super::*;
    use crate::{UiLoadState, UiState, UiStateEvent, UiStateTarget};

    #[test]
    fn route_lifetimes_redirects_and_fallback() {
        let first = ComponentId(1);
        let second = ComponentId(2);
        let routes = Routes::new()
            .route("/", first)
            .route("/settings", RouteComponent::load(second))
            .route("/details", RouteComponent::lazy(ComponentId(3)))
            .redirect("/old", "/settings")
            .fallback(first);
        let mut world = World::new();
        world.insert_resource(Router::new(routes));
        world.init_resource::<Messages<UiStateEvent>>();
        let outlet = world
            .spawn(TiltElement {
                kind: ElementKind::RouterOutlet,
            })
            .id();
        sync_outlets(&mut world);
        let first_instance = world.get::<RouterOutletState>(outlet).unwrap().instances["/"].1;
        let loaded = world.get::<RouterOutletState>(outlet).unwrap().instances["/settings"].1;
        assert_eq!(world.get::<Visibility>(loaded), Some(&Visibility::Hidden));
        let visible_child = world.spawn(Visibility::Visible).id();
        world.entity_mut(loaded).add_child(visible_child);
        sync_outlets(&mut world);
        let parking = world
            .get::<RouterOutletState>(outlet)
            .unwrap()
            .parking
            .unwrap();
        assert_eq!(world.get::<Node>(parking).unwrap().display, Display::None);
        assert_eq!(world.get::<ChildOf>(loaded).unwrap().parent(), parking);
        assert_eq!(
            world.get::<Visibility>(visible_child),
            Some(&Visibility::Visible)
        );
        assert!(
            !world
                .get::<RouterOutletState>(outlet)
                .unwrap()
                .instances
                .contains_key("/details")
        );
        assert_eq!(
            world
                .get::<PendingComponent>(first_instance)
                .unwrap()
                .component,
            first
        );
        world.get_mut::<UiState>(first_instance).unwrap().load = UiLoadState::Ready;
        world.get_mut::<UiState>(first_instance).unwrap().visible = Some(true);

        world.resource_mut::<Router>().navigate("/old");
        sync_outlets(&mut world);
        assert_eq!(
            world
                .resource_mut::<Messages<UiStateEvent>>()
                .drain()
                .collect::<Vec<_>>(),
            vec![UiStateEvent::Hidden(UiStateTarget::Component(
                first_instance
            ))]
        );
        let second_instance =
            world.get::<RouterOutletState>(outlet).unwrap().instances["/settings"].1;
        assert_ne!(first_instance, second_instance);
        assert_eq!(second_instance, loaded);
        assert_eq!(world.get::<ChildOf>(loaded).unwrap().parent(), outlet);
        assert_eq!(
            world.get::<Visibility>(visible_child),
            Some(&Visibility::Visible)
        );
        assert!(world.get_entity(first_instance).is_err());
        assert_eq!(
            world
                .get::<PendingComponent>(second_instance)
                .unwrap()
                .component,
            second
        );

        world.resource_mut::<Router>().navigate("/details");
        sync_outlets(&mut world);
        let lazy = world.get::<RouterOutletState>(outlet).unwrap().instances["/details"].1;
        world.resource_mut::<Router>().navigate("/settings");
        sync_outlets(&mut world);
        assert_eq!(world.get::<Visibility>(lazy), Some(&Visibility::Hidden));
        world.resource_mut::<Router>().navigate("/details");
        sync_outlets(&mut world);
        assert_eq!(
            world.get::<RouterOutletState>(outlet).unwrap().instances["/details"].1,
            lazy
        );

        world.resource_mut::<Router>().navigate("/unknown");
        sync_outlets(&mut world);
        assert_eq!(world.resource::<Router>().target(), Some(first));
    }

    #[test]
    fn merged_routes_normalize_paths_and_replace_duplicates() {
        let routes = Routes::new()
            .route("//settings/", ComponentId(1))
            .redirect("old/", "//settings//")
            .merge(Routes::new().route("settings", ComponentId(2)));
        assert_eq!(
            routes.resolve("/old").unwrap().destination.component,
            ComponentId(2)
        );
        assert_eq!(normalize_path("settings//"), "/settings");
    }

    #[test]
    fn replacing_routes_removes_stale_cached_instances() {
        let old = ComponentId(1);
        let new = ComponentId(2);
        let mut world = World::new();
        world.insert_resource(Router::new(
            Routes::new().route("/", RouteComponent::load(old)),
        ));
        let outlet = world
            .spawn(TiltElement {
                kind: ElementKind::RouterOutlet,
            })
            .id();
        sync_outlets(&mut world);
        let old_entity = world.get::<RouterOutletState>(outlet).unwrap().instances["/"].1;
        world
            .resource_mut::<Router>()
            .set_routes(Routes::new().route("/", RouteComponent::lazy(new)));
        sync_outlets(&mut world);
        let new_entity = world.get::<RouterOutletState>(outlet).unwrap().instances["/"].1;
        assert_ne!(old_entity, new_entity);
        assert!(world.get_entity(old_entity).is_err());
        assert_eq!(
            world.get::<PendingComponent>(new_entity).unwrap().component,
            new
        );
    }
}
