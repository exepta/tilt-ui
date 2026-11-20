//! Per-node shared-root dependencies; lexical scopes and dynamic calls stay conservative.
use std::collections::BTreeSet;

use bevy::ecs::{entity::Entity, hierarchy::ChildOf, world::World};

use super::{binding::UiSharedValues, binding_runtime::LocalValues, content::TemplateParent};

#[derive(Clone, Default)]
pub(super) enum Dependencies {
    #[default]
    All,
    Roots(Vec<(String, u64)>),
}

impl Dependencies {
    pub(super) fn text(world: &World, entity: Entity, source: &str) -> Self {
        let mut expressions = Vec::new();
        let mut rest = source;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                return Self::All;
            };
            expressions.push(&after[..end]);
            rest = &after[end + 2..];
        }
        Self::capture(world, entity, expressions, &[])
    }

    pub(super) fn capture<'a>(
        world: &World,
        entity: Entity,
        expressions: impl IntoIterator<Item = &'a str>,
        ignored: &[&str],
    ) -> Self {
        if conservative_scope(world, entity) {
            return Self::All;
        }
        let mut roots = BTreeSet::new();
        for expression in expressions {
            let Some(dependencies) = super::expression::dependencies(expression) else {
                return Self::All;
            };
            roots.extend(dependencies);
        }
        if roots.contains("i18n") || roots.contains("$event") {
            return Self::All;
        }
        let shared = world.resource::<UiSharedValues>();
        Self::Roots(
            roots
                .into_iter()
                .filter(|root| !ignored.contains(&root.as_str()))
                .map(|root| {
                    let revision = shared.root_revision(&root);
                    (root, revision)
                })
                .collect(),
        )
    }

    pub(super) fn changed(&self, world: &World, entity: Entity) -> bool {
        match self {
            Self::All => true,
            Self::Roots(roots) => {
                if conservative_scope(world, entity) {
                    return true;
                }
                let shared = world.resource::<UiSharedValues>();
                roots
                    .iter()
                    .any(|(root, revision)| shared.root_revision(root) != *revision)
            }
        }
    }

    pub(super) fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Roots(mut roots), Self::Roots(other)) => {
                roots.extend(other);
                roots.sort_unstable();
                roots.dedup();
                Self::Roots(roots)
            }
            _ => Self::All,
        }
    }
}

fn conservative_scope(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    while let Some(node) = current {
        if world.get::<LocalValues>(node).is_some()
            || world
                .get::<super::TemplateImports>(node)
                .is_some_and(|imports| !imports.0.is_empty())
        {
            return true;
        }
        current = world
            .get::<TemplateParent>(node)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(node).map(ChildOf::parent));
    }
    false
}

#[cfg(test)]
#[derive(bevy::ecs::resource::Resource, Default)]
pub(super) struct Evaluations(pub std::collections::HashMap<Entity, usize>);

#[cfg(test)]
pub(super) fn record(world: &mut World, entity: Entity) {
    if let Some(mut evaluations) = world.get_resource_mut::<Evaluations>() {
        *evaluations.0.entry(entity).or_default() += 1;
    }
}
