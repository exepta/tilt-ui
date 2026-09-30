//! Default context actions for editable and selected static text.

use bevy::{
    asset::Handle,
    clipboard::Clipboard,
    color::Color,
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::MessageReader,
        resource::Resource,
        system::{Commands, Query, Res, ResMut, SystemParam},
    },
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::Visibility,
    text::{EditableText as NativeEditableText, Font, FontSource, TextColor, TextEdit, TextFont},
    ui::{
        AlignItems, BackgroundColor, BorderColor, BorderRadius, ComputedNode,
        ComputedUiRenderTargetInfo, FlexDirection, GlobalZIndex, Interaction, JustifyContent, Node,
        PositionType, UiGlobalTransform, UiRect, UiScale, Val,
        widget::{Button, Text},
    },
    window::{PrimaryWindow, Window},
};
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_picking::{
    events::{Pointer, Press},
    pointer::PointerButton,
};
use tilt_ui_core::{InputType, TemplateAttribute};

use super::text_selection::{ActiveStaticTextSelection, SelectableStaticText};
use crate::{
    ComponentElementIds, ComponentStyleOwner, EditableText, StaticAttributes, WidgetLayoutOverride,
};

/// Identifies a TiltUI context menu popup.
#[derive(Component, Debug, Clone, Copy)]
pub struct ContextMenu {
    /// Control or selected text that owns this menu.
    pub target: Entity,
}

#[derive(Component)]
pub(crate) struct AuthoredContextMenu {
    target_id: Option<String>,
    target: Option<Entity>,
    parent: Option<Entity>,
}

#[derive(Component, Debug, Clone, Copy)]
struct SubmenuTrigger(Entity);

#[derive(Component, Debug, Clone, Copy)]
struct SubmenuPanel {
    root: Entity,
    trigger: Entity,
}

/// The new position must pass through UI layout before this panel is shown.
#[derive(Component)]
struct SubmenuPlacementPending;

/// Translation that was used to position the submenu in the last UI layout.
#[derive(Component, Debug, Clone, Copy)]
struct SubmenuPlacement(bevy::math::Vec2);

const CONTEXT_MENU_Z_INDEX: i32 = 20_000;

pub(crate) fn materialize(
    world: &mut bevy::ecs::world::World,
    entity: Entity,
    attributes: &[TemplateAttribute],
) {
    world.entity_mut(entity).insert((
        AuthoredContextMenu {
            target_id: crate::component::static_attribute_value(attributes, "for")
                .map(str::to_owned),
            target: None,
            parent: None,
        },
        GlobalZIndex(CONTEXT_MENU_Z_INDEX),
        Visibility::Hidden,
    ));
}

pub(crate) fn resolve_targets(world: &mut bevy::ecs::world::World, scope: Entity) {
    let entries = {
        let mut query = world.query::<(Entity, &AuthoredContextMenu, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .map(|(entity, menu, _)| (entity, menu.target_id.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, target_id) in entries {
        let target = target_id
            .as_deref()
            .and_then(|id| world.get::<ComponentElementIds>(scope)?.get(id))
            .or_else(|| {
                target_id
                    .is_none()
                    .then(|| world.get::<ChildOf>(entity).map(ChildOf::parent))
                    .flatten()
            });
        if let Some(target) = target {
            let parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
            world.entity_mut(entity).insert(ContextMenu { target });
            let mut menu = world.get_mut::<AuthoredContextMenu>(entity).unwrap();
            menu.target = Some(target);
            menu.parent = parent;
        }
        resolve_submenus(world, scope, entity);
    }
}

fn is_inside_world(world: &bevy::ecs::world::World, mut child: Entity, ancestor: Entity) -> bool {
    loop {
        if child == ancestor {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(child) else {
            return false;
        };
        child = parent.parent();
    }
}

fn static_value<'a>(attributes: &'a StaticAttributes, name: &str) -> Option<&'a str> {
    attributes
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .map(|attribute| attribute.value.as_str())
}

fn resolve_submenus(world: &mut bevy::ecs::world::World, scope: Entity, root: Entity) {
    let mut pending = vec![root];
    let mut triggers = Vec::new();
    let mut panels = Vec::new();
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(entity) {
            pending.extend(children.iter().copied());
        }
        let Some(attributes) = world.get::<StaticAttributes>(entity) else {
            continue;
        };
        if world.get::<Button>(entity).is_some()
            && let Some(panel_id) = static_value(attributes, "submenu")
        {
            triggers.push((entity, panel_id.to_owned()));
        }
    }
    for (trigger, panel_id) in triggers {
        let Some(panel) = world
            .get::<ComponentElementIds>(scope)
            .and_then(|ids| ids.get(&panel_id))
        else {
            continue;
        };
        if !is_inside_world(world, panel, root)
            || !world
                .get::<StaticAttributes>(panel)
                .is_some_and(|attributes| static_value(attributes, "submenu-panel").is_some())
        {
            continue;
        }
        world.entity_mut(trigger).insert(SubmenuTrigger(panel));
        world
            .entity_mut(panel)
            .insert((SubmenuPanel { root, trigger }, Visibility::Hidden));
        panels.push(panel);
        let has_arrow = world
            .get::<bevy::ecs::hierarchy::Children>(trigger)
            .into_iter()
            .flatten()
            .any(|child| {
                world.get::<crate::ControlPart>(*child).is_some_and(|part| {
                    part.owner == trigger && part.kind == crate::ControlPartKind::Indicator
                })
            });
        if !has_arrow {
            let arrow = crate::widgets::controls::spawn_text_part(
                world,
                trigger,
                crate::ControlPartKind::Indicator,
                ">",
            );
            world.entity_mut(arrow).insert((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(9.0),
                    top: Val::Px(6.0),
                    width: Val::Px(12.0),
                    height: Val::Px(22.0),
                    ..Default::default()
                },
                TextFont {
                    font_size: bevy::text::FontSize::Px(16.0),
                    ..Default::default()
                },
                TextColor(Color::srgb(0.48, 0.43, 0.60)),
            ));
            world.entity_mut(trigger).add_child(arrow);
        }
    }
    // GlobalZIndex breaks panels out of their parent's draw order. Each nested
    // panel must paint after the menu and after the panel that opened it.
    for panel in panels {
        let mut depth = 1i32;
        let mut ancestor = panel;
        while let Some(parent) = world.get::<ChildOf>(ancestor) {
            ancestor = parent.parent();
            if ancestor == root {
                break;
            }
            if world.get::<SubmenuPanel>(ancestor).is_some() {
                depth = depth.saturating_add(1);
            }
        }
        world
            .entity_mut(panel)
            .insert(GlobalZIndex(CONTEXT_MENU_Z_INDEX.saturating_add(depth)));
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct ContextMenuItem(MenuAction);

#[derive(Debug, Clone, Copy)]
enum MenuAction {
    Copy,
    Paste,
    Clear,
}

#[derive(Resource, Default)]
pub(crate) struct OpenContextMenu(Option<Entity>);

fn ancestor_with<T: Component>(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    entries: &Query<&T>,
) -> Option<Entity> {
    loop {
        if entries.get(entity).is_ok() {
            return Some(entity);
        }
        entity = parents.get(entity).ok()?.parent();
    }
}

fn clicked_selected_text(mut clicked: Entity, selected: Entity, parents: &Query<&ChildOf>) -> bool {
    let selected_parent = parents.get(selected).ok().map(ChildOf::parent);
    loop {
        if clicked == selected || selected_parent == Some(clicked) {
            return true;
        }
        clicked = match parents.get(clicked) {
            Ok(parent) => parent.parent(),
            Err(_) => return false,
        };
    }
}

fn is_inside(mut hit: Entity, parent: Entity, parents: &Query<&ChildOf>) -> bool {
    loop {
        if hit == parent {
            return true;
        }
        hit = match parents.get(hit) {
            Ok(child) => child.parent(),
            Err(_) => return false,
        };
    }
}

fn popup_position(
    position: bevy::math::Vec2,
    size: bevy::math::Vec2,
    viewport: bevy::math::Vec2,
) -> bevy::math::Vec2 {
    let margin = 6.0;
    let x = if position.x + size.x > viewport.x - margin {
        position.x - size.x
    } else {
        position.x
    };
    let y = if position.y + size.y > viewport.y - margin {
        position.y - size.y
    } else {
        position.y
    };
    bevy::math::Vec2::new(
        x.clamp(margin, (viewport.x - size.x - margin).max(margin)),
        y.clamp(margin, (viewport.y - size.y - margin).max(margin)),
    )
}

fn submenu_position(
    anchor: bevy::math::Vec2,
    anchor_size: bevy::math::Vec2,
    size: bevy::math::Vec2,
    viewport: bevy::math::Vec2,
) -> bevy::math::Vec2 {
    let margin = 6.0;
    let right = anchor.x + anchor_size.x - 2.0;
    let left = anchor.x - size.x + 2.0;
    let x = if right + size.x <= viewport.x - margin {
        right
    } else if left >= margin || anchor.x > viewport.x * 0.5 {
        left
    } else {
        right
    };
    let down = anchor.y;
    let up = anchor.y + anchor_size.y - size.y;
    let y = if down + size.y <= viewport.y - margin {
        down
    } else if up >= margin {
        up
    } else {
        down
    };
    bevy::math::Vec2::new(
        x.clamp(margin, (viewport.x - size.x - margin).max(margin)),
        y.clamp(margin, (viewport.y - size.y - margin).max(margin)),
    )
}

#[derive(SystemParam)]
pub(crate) struct MenuGeometry<'w, 's> {
    computed_nodes: Query<'w, 's, &'static ComputedNode>,
    render_targets: Query<'w, 's, &'static ComputedUiRenderTargetInfo>,
    ui_scale: Option<Res<'w, UiScale>>,
    buttons: Query<'w, 's, &'static Button>,
    submenu_triggers: Query<'w, 's, &'static SubmenuTrigger>,
    theme_fonts: Option<Res<'w, crate::theme::DefaultThemeFonts>>,
    #[cfg(feature = "fluent")]
    localization: Option<Res<'w, crate::UiLocalization>>,
}

impl MenuGeometry<'_, '_> {
    fn logical_size(&self, entity: Entity) -> Option<bevy::math::Vec2> {
        let scale = self
            .render_targets
            .get(entity)
            .map_or(1.0, ComputedUiRenderTargetInfo::scale_factor)
            / self.ui_scale.as_ref().map_or(1.0, |scale| scale.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        self.computed_nodes
            .get(entity)
            .ok()
            .map(|node| node.size() / scale)
            .filter(|size| size.min_element() > 0.0)
    }

    fn label_for(&self, action: MenuAction) -> String {
        let (key, fallback) = match action {
            MenuAction::Copy => ("context-copy", "Copy"),
            MenuAction::Paste => ("context-paste", "Paste"),
            MenuAction::Clear => ("context-clear", "Clear"),
        };
        #[cfg(not(feature = "fluent"))]
        let _ = key;
        #[cfg(feature = "fluent")]
        if let Some(value) = self
            .localization
            .as_ref()
            .and_then(|locale| locale.translate(key, None))
        {
            return value;
        }
        fallback.to_owned()
    }
}

fn copied_input_value(input: &EditableText) -> Option<String> {
    if input.value.is_empty() || input.input_type == InputType::Password {
        return None;
    }
    let selected = input
        .selection_anchor
        .and_then(|anchor| {
            input
                .value
                .get(anchor.min(input.cursor)..anchor.max(input.cursor))
        })
        .filter(|text| !text.is_empty());
    Some(selected.unwrap_or(&input.value).to_owned())
}

fn copied_static_text_value(selection: &SelectableStaticText, text: &Text) -> Option<String> {
    text.0.get(selection.range()?).map(str::to_owned)
}

fn menu_item(
    commands: &mut Commands,
    parent: Entity,
    label: &str,
    action: MenuAction,
    font: Option<&Handle<Font>>,
) {
    let button = commands
        .spawn((
            Button,
            ContextMenuItem(action),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..Default::default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            TextFont {
                font: font.cloned().map(FontSource::Handle).unwrap_or_default(),
                font_size: bevy::text::FontSize::Px(14.0),
                ..Default::default()
            },
            bevy::text::TextColor(Color::srgb(0.16, 0.16, 0.29)),
        ))
        .id();
    commands.entity(button).add_child(text);
    commands.entity(parent).add_child(button);
}

fn spawn_menu(
    commands: &mut Commands,
    target: Entity,
    position: bevy::math::Vec2,
    window: Option<&Window>,
    actions: &[MenuAction],
    labels: &[String],
    font: Option<&Handle<Font>>,
) -> Entity {
    let width = 178.0;
    let height = actions.len() as f32 * 34.0 + 10.0;
    let placed = window.map_or(position, |window| {
        popup_position(
            position,
            bevy::math::Vec2::new(width, height),
            bevy::math::Vec2::new(window.width(), window.height()),
        )
    });
    let menu = commands
        .spawn((
            ContextMenu { target },
            GlobalZIndex(CONTEXT_MENU_Z_INDEX),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(placed.x.max(0.0)),
                top: Val::Px(placed.y.max(0.0)),
                width: Val::Px(width),
                padding: UiRect::all(Val::Px(5.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..Default::default()
            },
            BackgroundColor(Color::WHITE),
            BorderColor::all(Color::srgb(0.85, 0.84, 0.92)),
            Visibility::Visible,
        ))
        .id();
    for (action, label) in actions.iter().zip(labels) {
        menu_item(commands, menu, label, *action, font);
    }
    menu
}

fn close_menu(
    commands: &mut Commands,
    open: &mut OpenContextMenu,
    authored: &Query<(Entity, &AuthoredContextMenu, &mut Node)>,
) {
    if let Some(menu) = open.0.take() {
        if let Ok((_, authored_menu, _)) = authored.get(menu) {
            commands.entity(menu).insert(Visibility::Hidden);
            if let Some(parent) = authored_menu.parent {
                commands.entity(parent).add_child(menu);
            }
        } else {
            commands.entity(menu).despawn();
        }
    }
}

fn remove_orphan_context_menus(world: &mut bevy::ecs::world::World) {
    let stale = {
        let mut query = world.query::<(Entity, &AuthoredContextMenu, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(entity, _, owner)| {
                world.get::<ChildOf>(*entity).is_none() && world.get_entity(owner.0).is_err()
            })
            .map(|(entity, _, _)| entity)
            .collect::<Vec<_>>()
    };
    for entity in stale {
        if world.resource::<OpenContextMenu>().0 == Some(entity) {
            world.resource_mut::<OpenContextMenu>().0 = None;
        }
        world.entity_mut(entity).despawn();
    }
}

pub(crate) fn context_menu_pointer_input(
    mut commands: Commands,
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut open: ResMut<OpenContextMenu>,
    parents: Query<&ChildOf>,
    items: Query<&ContextMenuItem>,
    menus: Query<&ContextMenu>,
    mut authored: Query<(Entity, &AuthoredContextMenu, &mut Node)>,
    inputs: Query<&EditableText>,
    selections: Query<&SelectableStaticText>,
    texts: Query<&Text>,
    active: Res<ActiveStaticTextSelection>,
    mut native: Query<&mut NativeEditableText>,
    mut focus: Option<ResMut<InputFocus>>,
    mut clipboard: Option<ResMut<Clipboard>>,
    windows: Query<&Window, bevy::ecs::query::With<PrimaryWindow>>,
    geometry: MenuGeometry,
) {
    let Some(presses) = presses.as_mut() else {
        return;
    };
    'events: for press in presses.read() {
        if press.button == PointerButton::Primary {
            if let Some(item) = ancestor_with(press.entity, &parents, &items)
                && let Some(menu) = open.0.and_then(|entity| menus.get(entity).ok())
            {
                let target = menu.target;
                match items.get(item).unwrap().0 {
                    MenuAction::Copy => {
                        let value = inputs
                            .get(target)
                            .ok()
                            .and_then(|input| {
                                if input.input_type == InputType::Password {
                                    return None;
                                }
                                let Ok(editor) = native.get_mut(target) else {
                                    return copied_input_value(input);
                                };
                                let value = editor.value().to_string();
                                if value.is_empty() {
                                    return None;
                                }
                                let selected = value
                                    .get(editor.editor().raw_selection().text_range())
                                    .filter(|selection| !selection.is_empty());
                                Some(selected.unwrap_or(&value).to_owned())
                            })
                            .or_else(|| {
                                copied_static_text_value(
                                    selections.get(target).ok()?,
                                    texts.get(target).ok()?,
                                )
                            });
                        if let (Some(value), Some(clipboard)) = (value, clipboard.as_deref_mut()) {
                            let _ = clipboard.set_text(value);
                        }
                    }
                    MenuAction::Paste | MenuAction::Clear => {
                        if inputs.get(target).is_ok_and(|input| !input.readonly)
                            && let Ok(mut editor) = native.get_mut(target)
                        {
                            if let Some(focus) = focus.as_deref_mut() {
                                focus.set(target, FocusCause::Pressed);
                            }
                            match items.get(item).unwrap().0 {
                                MenuAction::Paste => editor.queue_edit(TextEdit::Paste),
                                MenuAction::Clear => {
                                    editor.queue_edit(TextEdit::SelectAll);
                                    editor.queue_edit(TextEdit::Backspace);
                                }
                                MenuAction::Copy => unreachable!(),
                            }
                        }
                    }
                }
                close_menu(&mut commands, &mut open, &authored);
                continue;
            }
            if let Some(menu) = open.0
                && authored.get(menu).is_ok()
                && is_inside(press.entity, menu, &parents)
            {
                if let Some(button) = ancestor_with(press.entity, &parents, &geometry.buttons)
                    && is_inside(button, menu, &parents)
                {
                    if geometry.submenu_triggers.get(button).is_ok() {
                        continue;
                    }
                    close_menu(&mut commands, &mut open, &authored);
                }
                continue;
            }
            close_menu(&mut commands, &mut open, &authored);
            continue;
        }
        if press.button != PointerButton::Secondary {
            continue;
        }
        close_menu(&mut commands, &mut open, &authored);
        for (entity, menu, mut node) in &mut authored {
            let Some(target) = menu.target else {
                continue;
            };
            if !is_inside(press.entity, target, &parents) {
                continue;
            }
            let position = press.pointer_location.position;
            let size = geometry
                .logical_size(entity)
                .unwrap_or(bevy::math::Vec2::new(180.0, 46.0));
            let placed = windows.iter().next().map_or(position, |window| {
                popup_position(
                    position,
                    size,
                    bevy::math::Vec2::new(window.width(), window.height()),
                )
            });
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(placed.x.max(0.0));
            node.top = Val::Px(placed.y.max(0.0));
            commands.entity(entity).insert(WidgetLayoutOverride {
                left: Some(node.left),
                top: Some(node.top),
                ..Default::default()
            });
            commands.entity(entity).remove::<ChildOf>();
            commands.entity(entity).insert(Visibility::Visible);
            open.0 = Some(entity);
            continue 'events;
        }
        let input = ancestor_with(press.entity, &parents, &inputs);
        let selected = active.entity.filter(|entity| {
            clicked_selected_text(press.entity, *entity, &parents)
                && selections
                    .get(*entity)
                    .is_ok_and(|selection| selection.range().is_some())
        });
        let (target, actions) = if let Some(input) = input {
            let state = inputs.get(input).unwrap();
            if state.input_type == InputType::File {
                continue;
            }
            let mut actions = Vec::new();
            if copied_input_value(state).is_some() {
                actions.push(MenuAction::Copy);
            }
            if !state.readonly {
                actions.push(MenuAction::Paste);
                if !state.value.is_empty() {
                    actions.push(MenuAction::Clear);
                }
            }
            (input, actions)
        } else if let Some(selected) = selected {
            (selected, vec![MenuAction::Copy])
        } else {
            continue;
        };
        if actions.is_empty() {
            continue;
        }
        let labels = actions
            .iter()
            .map(|action| geometry.label_for(*action))
            .collect::<Vec<_>>();
        open.0 = Some(spawn_menu(
            &mut commands,
            target,
            press.pointer_location.position,
            windows.iter().next(),
            &actions,
            &labels,
            geometry.theme_fonts.as_ref().map(|fonts| &fonts.regular),
        ));
    }
}

fn position_submenu_panel(
    world: &mut bevy::ecs::world::World,
    panel: Entity,
    viewport: bevy::math::Vec2,
) -> Option<bool> {
    let Some(info) = world.get::<SubmenuPanel>(panel).copied() else {
        return None;
    };
    let Some(parent) = world.get::<ChildOf>(panel).map(ChildOf::parent) else {
        return None;
    };
    let (Some(anchor_transform), Some(anchor_node), Some(parent_transform)) = (
        world.get::<UiGlobalTransform>(info.trigger),
        world.get::<ComputedNode>(info.trigger),
        world.get::<UiGlobalTransform>(parent),
    ) else {
        return None;
    };
    let Some(inverse) = parent_transform.try_inverse() else {
        return None;
    };
    if anchor_node.size().min_element() <= 0.0 {
        return None;
    }
    let scale = world
        .get::<ComputedUiRenderTargetInfo>(panel)
        .map_or(1.0, |target| target.scale_factor())
        / world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = world.get::<ComputedNode>(panel).map(ComputedNode::size)?;
    if size.min_element() <= 0.0 {
        return None;
    }
    let anchor = anchor_transform.affine().translation - anchor_node.size() * 0.5;
    let placed = submenu_position(anchor, anchor_node.size(), size, viewport * scale);
    let current_center = world.get::<UiGlobalTransform>(panel)?.affine().translation;
    let correction = inverse.transform_vector2(placed + size * 0.5 - current_center) / scale;
    let old_translation = world.get::<SubmenuPlacement>(panel).map_or_else(
        || {
            world
                .get::<bevy::ui::UiTransform>(panel)
                .map_or(bevy::math::Vec2::ZERO, |transform| {
                    let px = |value| match value {
                        Val::Px(value) => value,
                        _ => 0.0,
                    };
                    bevy::math::Vec2::new(px(transform.translation.x), px(transform.translation.y))
                })
        },
        |placement| placement.0,
    );
    let position_changed = correction.length_squared() > 0.25;
    if !position_changed && world.get::<SubmenuPlacement>(panel).is_none() {
        return Some(false);
    }
    let translation = if position_changed {
        old_translation + correction
    } else {
        old_translation
    };
    if let Some(mut node) = world.get_mut::<Node>(panel)
        && node.position_type != PositionType::Absolute
    {
        node.position_type = PositionType::Absolute;
    }
    if let Some(mut transform) = world.get_mut::<bevy::ui::UiTransform>(panel) {
        let next_x = Val::Px(translation.x);
        let next_y = Val::Px(translation.y);
        if transform.translation.x != next_x || transform.translation.y != next_y {
            transform.translation.x = next_x;
            transform.translation.y = next_y;
        }
    }
    if world
        .get::<SubmenuPlacement>(panel)
        .is_none_or(|old| old.0 != translation)
    {
        world
            .entity_mut(panel)
            .insert(SubmenuPlacement(translation));
    }
    Some(position_changed)
}

fn hide_submenu_panel(world: &mut bevy::ecs::world::World, panel: Entity) {
    if world.get::<Visibility>(panel) != Some(&Visibility::Hidden) {
        world.entity_mut(panel).insert(Visibility::Hidden);
    }
    if world.get::<SubmenuPlacement>(panel).is_some() {
        if let Some(mut transform) = world.get_mut::<bevy::ui::UiTransform>(panel) {
            transform.translation.x = Val::Px(0.0);
            transform.translation.y = Val::Px(0.0);
        }
        world.entity_mut(panel).remove::<SubmenuPlacement>();
    }
    if world.get::<SubmenuPlacementPending>(panel).is_some() {
        world.entity_mut(panel).remove::<SubmenuPlacementPending>();
    }
}

pub(crate) fn update_context_submenus(world: &mut bevy::ecs::world::World) {
    let root = world.resource::<OpenContextMenu>().0;
    let panels = {
        let mut query = world.query::<(Entity, &SubmenuPanel)>();
        query
            .iter(world)
            .map(|(entity, info)| (entity, *info))
            .collect::<Vec<_>>()
    };
    let Some(root) = root.filter(|root| world.get::<AuthoredContextMenu>(*root).is_some()) else {
        for (panel, _) in panels {
            hide_submenu_panel(world, panel);
        }
        return;
    };
    let hovered = {
        let mut query = world.query::<(Entity, &Interaction, &Button)>();
        query
            .iter(world)
            .filter(|(entity, interaction, _)| {
                **interaction != Interaction::None && is_inside_world(world, *entity, root)
            })
            .max_by_key(|(entity, _, _)| {
                let mut depth = 0;
                let mut current = *entity;
                while let Some(parent) = world.get::<ChildOf>(current) {
                    depth += 1;
                    current = parent.parent();
                    if current == root {
                        break;
                    }
                }
                depth
            })
            .map(|(entity, _, _)| entity)
    };
    let Some(hovered) = hovered else {
        return;
    };
    let mut visible = Vec::new();
    let mut current = hovered;
    while current != root {
        if world.get::<SubmenuPanel>(current).is_some() {
            visible.push(current);
        }
        let Some(parent) = world.get::<ChildOf>(current) else {
            break;
        };
        current = parent.parent();
    }
    if let Some(trigger) = world.get::<SubmenuTrigger>(hovered) {
        visible.push(trigger.0);
    }
    let viewport = {
        let mut windows = world.query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>();
        windows
            .iter(world)
            .next()
            .map_or(bevy::math::Vec2::new(1920.0, 1080.0), |window| {
                bevy::math::Vec2::new(window.width(), window.height())
            })
    };
    for (panel, info) in panels {
        if info.root != root || !visible.contains(&panel) {
            hide_submenu_panel(world, panel);
            continue;
        }
        let Some(position_changed) = position_submenu_panel(world, panel, viewport) else {
            continue;
        };
        let already_visible = world.get::<Visibility>(panel) == Some(&Visibility::Visible);
        if !already_visible && position_changed {
            world.entity_mut(panel).insert(SubmenuPlacementPending);
            continue;
        }
        if !already_visible {
            world.entity_mut(panel).remove::<SubmenuPlacementPending>();
            world.entity_mut(panel).insert(Visibility::Visible);
        }
    }
}

pub(crate) fn context_menu_keyboard_input(
    mut commands: Commands,
    mut keys: Option<MessageReader<KeyboardInput>>,
    mut open: ResMut<OpenContextMenu>,
    authored: Query<(Entity, &AuthoredContextMenu, &mut Node)>,
) {
    let Some(keys) = keys.as_mut() else {
        return;
    };
    if keys
        .read()
        .any(|key| key.state == ButtonState::Pressed && key.logical_key == Key::Escape)
    {
        close_menu(&mut commands, &mut open, &authored);
    }
}

pub(crate) fn style_context_menu_items(
    mut items: Query<
        (&Interaction, &mut BackgroundColor),
        (
            bevy::ecs::query::With<ContextMenuItem>,
            bevy::ecs::query::Changed<Interaction>,
        ),
    >,
) {
    for (interaction, mut background) in &mut items {
        *background = BackgroundColor(match interaction {
            Interaction::Hovered => Color::srgb(0.95, 0.91, 0.99),
            Interaction::Pressed => Color::srgb(0.90, 0.82, 0.97),
            Interaction::None => Color::NONE,
        });
    }
}

pub(crate) fn init(app: &mut bevy::app::App) {
    app.init_resource::<OpenContextMenu>();
    app.add_systems(bevy::app::Update, remove_orphan_context_menus);
}

#[cfg(test)]
mod tests {
    use super::{
        AuthoredContextMenu, ContextMenu, ContextMenuItem, MenuAction, OpenContextMenu,
        context_menu_pointer_input, copied_input_value, copied_static_text_value,
        remove_orphan_context_menus,
    };
    use crate::{
        ComponentStyleOwner, EditableText,
        control::text_selection::{ActiveStaticTextSelection, SelectableStaticText},
    };
    use bevy::{
        app::{App, Update},
        camera::NormalizedRenderTarget,
        clipboard::Clipboard,
        ecs::{
            hierarchy::{ChildOf, Children},
            message::Messages,
        },
        math::Vec2,
        prelude::Visibility,
        text::{EditableText as NativeEditableText, TextEdit},
        ui::{
            Node, Val,
            widget::{Button, Text},
        },
        window::{PrimaryWindow, Window},
    };
    use bevy_picking::{
        backend::HitData,
        events::{Pointer, Press},
        pointer::{Location, PointerButton, PointerId},
    };
    use tilt_ui_core::InputType;

    fn press(
        world: &mut bevy::ecs::world::World,
        entity: bevy::ecs::entity::Entity,
        button: PointerButton,
    ) {
        press_at(world, entity, button, Vec2::new(40.0, 50.0));
    }

    fn press_at(
        world: &mut bevy::ecs::world::World,
        entity: bevy::ecs::entity::Entity,
        button: PointerButton,
        position: Vec2,
    ) {
        world
            .resource_mut::<Messages<Pointer<Press>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 400,
                        height: 300,
                    },
                    position,
                },
                Press {
                    button,
                    hit: HitData::new(entity, 0.0, None, None),
                    count: 1,
                },
                entity,
            ));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<Pointer<Press>>()
            .init_resource::<OpenContextMenu>()
            .init_resource::<ActiveStaticTextSelection>()
            .insert_resource(Clipboard::default())
            .add_systems(Update, context_menu_pointer_input);
        app
    }

    #[test]
    fn copy_uses_selection_or_complete_nonempty_value() {
        let mut input = EditableText::new("alpha beta".into(), InputType::Text, false, false);
        assert_eq!(copied_input_value(&input).as_deref(), Some("alpha beta"));
        input.selection_anchor = Some(0);
        input.cursor = 5;
        assert_eq!(copied_input_value(&input).as_deref(), Some("alpha"));
        input.value.clear();
        assert_eq!(copied_input_value(&input), None);
    }

    #[test]
    fn input_menu_offers_copy_paste_clear_and_clear_queues_native_edits() {
        let mut app = app();
        let input = app
            .world_mut()
            .spawn((
                EditableText::new("hello".into(), InputType::Text, false, false),
                NativeEditableText::new("hello"),
            ))
            .id();
        press(app.world_mut(), input, PointerButton::Secondary);
        app.update();
        let menu = app.world().resource::<OpenContextMenu>().0.unwrap();
        let children = app.world().get::<Children>(menu).unwrap();
        assert_eq!(children.len(), 3);
        let clear = children[2];
        press(app.world_mut(), clear, PointerButton::Primary);
        app.update();
        assert!(app.world().resource::<OpenContextMenu>().0.is_none());
        let edits = &app
            .world()
            .get::<NativeEditableText>(input)
            .unwrap()
            .pending_edits;
        assert!(edits.ends_with(&[TextEdit::SelectAll, TextEdit::Backspace]));
    }

    #[test]
    fn selected_readonly_text_gets_only_copy() {
        let mut app = app();
        let text = app
            .world_mut()
            .spawn((
                Text::new("selection"),
                SelectableStaticText {
                    anchor: 0,
                    focus: 6,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<ActiveStaticTextSelection>()
            .entity = Some(text);
        press(app.world_mut(), text, PointerButton::Secondary);
        app.update();
        let menu = app.world().resource::<OpenContextMenu>().0.unwrap();
        let children = app.world().get::<Children>(menu).unwrap();
        assert_eq!(children.len(), 1);
        let copy = children[0];
        assert!(matches!(
            app.world().get::<ContextMenuItem>(copy),
            Some(ContextMenuItem(MenuAction::Copy))
        ));
        assert_eq!(
            copied_static_text_value(
                app.world().get::<SelectableStaticText>(text).unwrap(),
                app.world().get::<Text>(text).unwrap(),
            )
            .as_deref(),
            Some("select")
        );
        press(app.world_mut(), copy, PointerButton::Primary);
        app.update();
        assert!(app.world().resource::<OpenContextMenu>().0.is_none());
    }

    #[test]
    fn authored_button_menu_uses_pointer_coordinates_and_restores_parent() {
        let mut app = app();
        let parent = app.world_mut().spawn(Node::default()).id();
        let button = app.world_mut().spawn(Button).id();
        let menu = app
            .world_mut()
            .spawn((
                ContextMenu { target: button },
                AuthoredContextMenu {
                    target_id: Some("button".into()),
                    target: Some(button),
                    parent: Some(parent),
                },
                Node::default(),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .entity_mut(parent)
            .add_children(&[button, menu]);

        press(app.world_mut(), button, PointerButton::Secondary);
        app.update();

        assert_eq!(app.world().resource::<OpenContextMenu>().0, Some(menu));
        assert!(app.world().get::<ChildOf>(menu).is_none());
        let node = app.world().get::<Node>(menu).unwrap();
        assert_eq!(node.left, Val::Px(40.0));
        assert_eq!(node.top, Val::Px(50.0));
        assert_eq!(
            app.world()
                .get::<crate::WidgetLayoutOverride>(menu)
                .unwrap()
                .left,
            Some(Val::Px(40.0))
        );
        assert_eq!(
            app.world().get::<Visibility>(menu),
            Some(&Visibility::Visible)
        );

        press(app.world_mut(), parent, PointerButton::Primary);
        app.update();

        assert!(app.world().resource::<OpenContextMenu>().0.is_none());
        assert_eq!(
            app.world().get::<ChildOf>(menu).map(ChildOf::parent),
            Some(parent)
        );
        assert_eq!(
            app.world().get::<Visibility>(menu),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn detached_menu_is_removed_with_its_component_owner() {
        let mut app = app();
        app.add_systems(Update, remove_orphan_context_menus);
        let scope = app.world_mut().spawn_empty().id();
        let parent = app.world_mut().spawn(Node::default()).id();
        let button = app.world_mut().spawn(Button).id();
        let menu = app
            .world_mut()
            .spawn((
                ContextMenu { target: button },
                AuthoredContextMenu {
                    target_id: None,
                    target: Some(button),
                    parent: Some(parent),
                },
                ComponentStyleOwner(scope),
                Node::default(),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .entity_mut(parent)
            .add_children(&[button, menu]);

        press(app.world_mut(), button, PointerButton::Secondary);
        app.update();
        app.world_mut().entity_mut(scope).despawn();
        app.update();

        assert!(app.world().get_entity(menu).is_err());
        assert!(app.world().resource::<OpenContextMenu>().0.is_none());
    }

    #[test]
    fn authored_menu_stays_inside_window_at_pointer_edge() {
        let mut app = app();
        app.world_mut().spawn((
            Window {
                resolution: (400, 300).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ));
        let parent = app.world_mut().spawn(Node::default()).id();
        let button = app.world_mut().spawn(Button).id();
        let menu = app
            .world_mut()
            .spawn((
                ContextMenu { target: button },
                AuthoredContextMenu {
                    target_id: None,
                    target: Some(button),
                    parent: Some(parent),
                },
                Node::default(),
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .entity_mut(parent)
            .add_children(&[button, menu]);

        press_at(
            app.world_mut(),
            button,
            PointerButton::Secondary,
            Vec2::new(390.0, 290.0),
        );
        app.update();

        let node = app.world().get::<Node>(menu).unwrap();
        assert_eq!(node.left, Val::Px(210.0));
        assert_eq!(node.top, Val::Px(244.0));
    }

    #[test]
    fn authored_menu_uses_logical_size_on_scaled_windows() {
        use bevy::ui::{ComputedNode, UiScale};

        let mut app = app();
        app.world_mut().insert_resource(UiScale(0.5));
        app.world_mut().spawn((
            Window {
                resolution: (400, 300).into(),
                ..Default::default()
            },
            PrimaryWindow,
        ));
        let parent = app.world_mut().spawn(Node::default()).id();
        let button = app.world_mut().spawn(Button).id();
        let menu = app
            .world_mut()
            .spawn((
                ContextMenu { target: button },
                AuthoredContextMenu {
                    target_id: None,
                    target: Some(button),
                    parent: Some(parent),
                },
                Node::default(),
                ComputedNode {
                    size: Vec2::new(360.0, 160.0),
                    ..Default::default()
                },
                Visibility::Hidden,
            ))
            .id();
        app.world_mut()
            .entity_mut(parent)
            .add_children(&[button, menu]);

        press_at(
            app.world_mut(),
            button,
            PointerButton::Secondary,
            Vec2::new(390.0, 290.0),
        );
        app.update();

        let node = app.world().get::<Node>(menu).unwrap();
        assert_eq!(node.left, Val::Px(210.0));
        assert_eq!(node.top, Val::Px(210.0));
    }

    #[test]
    fn submenu_flips_left_and_up_near_window_edges() {
        let viewport = Vec2::new(400.0, 300.0);
        let placed = super::submenu_position(
            Vec2::new(360.0, 265.0),
            Vec2::new(30.0, 30.0),
            Vec2::new(180.0, 120.0),
            viewport,
        );
        assert_eq!(placed, Vec2::new(182.0, 174.0));
        let roomy = super::submenu_position(
            Vec2::new(40.0, 40.0),
            Vec2::new(80.0, 30.0),
            Vec2::new(180.0, 120.0),
            viewport,
        );
        assert_eq!(roomy, Vec2::new(118.0, 40.0));
    }

    #[test]
    fn nested_submenus_paint_above_their_parent_menus() {
        use super::resolve_submenus;
        use crate::{ComponentElementIds, StaticAttribute, StaticAttributes};
        use bevy::ui::GlobalZIndex;

        let mut app = app();
        let scope = app.world_mut().spawn(ComponentElementIds::default()).id();
        let root = app
            .world_mut()
            .spawn((Node::default(), GlobalZIndex(super::CONTEXT_MENU_Z_INDEX)))
            .id();
        let trigger = app
            .world_mut()
            .spawn((
                Button,
                Node::default(),
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu".into(),
                        value: "first".into(),
                    }],
                },
            ))
            .id();
        let panel = app
            .world_mut()
            .spawn((
                Node::default(),
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu-panel".into(),
                        value: "true".into(),
                    }],
                },
            ))
            .id();
        let nested_trigger = app
            .world_mut()
            .spawn((
                Button,
                Node::default(),
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu".into(),
                        value: "second".into(),
                    }],
                },
            ))
            .id();
        let nested_panel = app
            .world_mut()
            .spawn((
                Node::default(),
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu-panel".into(),
                        value: "true".into(),
                    }],
                },
            ))
            .id();
        app.world_mut().entity_mut(scope).add_child(root);
        app.world_mut()
            .entity_mut(root)
            .add_children(&[trigger, panel]);
        app.world_mut()
            .entity_mut(panel)
            .add_children(&[nested_trigger, nested_panel]);
        let mut ids = app
            .world_mut()
            .get_mut::<ComponentElementIds>(scope)
            .unwrap();
        ids.insert("first".into(), panel);
        ids.insert("second".into(), nested_panel);

        resolve_submenus(app.world_mut(), scope, root);

        let z = |entity| app.world().get::<GlobalZIndex>(entity).unwrap().0;
        assert!(z(root) < z(panel));
        assert!(z(panel) < z(nested_panel));
    }

    #[test]
    fn authored_submenu_opens_on_hover_and_stays_open_over_its_items() {
        use super::{
            SubmenuPanel, SubmenuPlacement, SubmenuPlacementPending, SubmenuTrigger,
            resolve_submenus, update_context_submenus,
        };
        use crate::{ComponentElementIds, StaticAttribute, StaticAttributes};
        use bevy::ui::{ComputedNode, Interaction, UiGlobalTransform, UiTransform};

        let mut app = app();
        let scope = app.world_mut().spawn(ComponentElementIds::default()).id();
        let target = app.world_mut().spawn(Button).id();
        let root = app
            .world_mut()
            .spawn((
                ContextMenu { target },
                AuthoredContextMenu {
                    target_id: None,
                    target: Some(target),
                    parent: Some(scope),
                },
                Node::default(),
                ComputedNode {
                    size: Vec2::new(180.0, 200.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(190.0, 150.0),
            ))
            .id();
        let trigger = app
            .world_mut()
            .spawn((
                Button,
                Node::default(),
                ComputedNode {
                    size: Vec2::new(180.0, 34.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(190.0, 90.0),
                Interaction::None,
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu".into(),
                        value: "more".into(),
                    }],
                },
            ))
            .id();
        let regular = app
            .world_mut()
            .spawn((Button, Node::default(), Interaction::None))
            .id();
        let panel = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(180.0, 120.0),
                    ..Default::default()
                },
                StaticAttributes {
                    attributes: vec![StaticAttribute {
                        name: "submenu-panel".into(),
                        value: String::new(),
                    }],
                },
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((Button, Node::default(), Interaction::None))
            .id();
        app.world_mut().entity_mut(scope).add_child(root);
        app.world_mut()
            .entity_mut(root)
            .add_children(&[trigger, panel, regular]);
        app.world_mut().entity_mut(panel).add_child(child);
        app.world_mut()
            .get_mut::<ComponentElementIds>(scope)
            .unwrap()
            .insert("more".into(), panel);
        resolve_submenus(app.world_mut(), scope, root);
        assert_eq!(app.world().get::<SubmenuTrigger>(trigger).unwrap().0, panel);
        assert_eq!(app.world().get::<SubmenuPanel>(panel).unwrap().root, root);
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
        app.world_mut().resource_mut::<OpenContextMenu>().0 = Some(root);
        app.world_mut()
            .entity_mut(trigger)
            .insert(Interaction::Hovered);
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
        assert!(app.world().get::<SubmenuPlacementPending>(panel).is_some());
        let saved = *app.world().get::<SubmenuPlacement>(panel).unwrap();
        assert!(saved.0.length() > 0.0);
        // Simulate Bevy applying the new UI translation in its layout pass.
        let previous = app
            .world()
            .get::<UiGlobalTransform>(panel)
            .unwrap()
            .affine()
            .translation;
        app.world_mut()
            .entity_mut(panel)
            .insert(UiGlobalTransform::from_xy(
                previous.x + saved.0.x,
                previous.y + saved.0.y,
            ));
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Visible)
        );
        app.world_mut()
            .entity_mut(panel)
            .insert(UiTransform::IDENTITY);
        update_context_submenus(app.world_mut());
        let transform = app.world().get::<UiTransform>(panel).unwrap();
        assert_eq!(transform.translation.x, Val::Px(saved.0.x));
        assert_eq!(transform.translation.y, Val::Px(saved.0.y));

        app.world_mut()
            .entity_mut(trigger)
            .insert(Interaction::None);
        app.world_mut()
            .entity_mut(child)
            .insert(Interaction::Hovered);
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Visible)
        );

        app.world_mut().entity_mut(child).insert(Interaction::None);
        app.world_mut()
            .entity_mut(regular)
            .insert(Interaction::Hovered);
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
        assert!(app.world().get::<SubmenuPlacement>(panel).is_none());
        app.world_mut()
            .entity_mut(panel)
            .insert(UiGlobalTransform::from_xy(previous.x, previous.y));
        app.world_mut()
            .entity_mut(regular)
            .insert(Interaction::None);
        app.world_mut()
            .entity_mut(trigger)
            .insert(Interaction::Hovered);
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Hidden)
        );
        let saved = *app.world().get::<SubmenuPlacement>(panel).unwrap();
        app.world_mut()
            .entity_mut(panel)
            .insert(UiGlobalTransform::from_xy(
                previous.x + saved.0.x,
                previous.y + saved.0.y,
            ));
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Visible)
        );
        press(app.world_mut(), trigger, PointerButton::Primary);
        app.update();
        assert_eq!(app.world().resource::<OpenContextMenu>().0, Some(root));
        app.world_mut()
            .entity_mut(regular)
            .insert(Interaction::None);
        app.world_mut()
            .entity_mut(trigger)
            .insert(Interaction::Pressed);
        update_context_submenus(app.world_mut());
        assert_eq!(
            app.world().get::<Visibility>(panel),
            Some(&Visibility::Visible)
        );

        press(app.world_mut(), child, PointerButton::Primary);
        app.update();
        assert!(app.world().resource::<OpenContextMenu>().0.is_none());
    }
}
