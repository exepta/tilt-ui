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
    text::{EditableText as NativeEditableText, Font, FontSource, TextEdit, TextFont},
    ui::{
        AlignItems, BackgroundColor, BorderColor, BorderRadius, ComputedNode, FlexDirection,
        GlobalZIndex, Interaction, JustifyContent, Node, PositionType, UiRect, Val,
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
use crate::{ComponentElementIds, ComponentStyleOwner, EditableText};

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
        GlobalZIndex(20_000),
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

#[derive(SystemParam)]
pub(crate) struct MenuGeometry<'w, 's> {
    computed_nodes: Query<'w, 's, &'static ComputedNode>,
    theme_fonts: Option<Res<'w, crate::theme::DefaultThemeFonts>>,
    #[cfg(feature = "fluent")]
    localization: Option<Res<'w, crate::UiLocalization>>,
}

impl MenuGeometry<'_, '_> {
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
    let (left, top) = window.map_or((position.x, position.y), |window| {
        (
            position.x.min((window.width() - width - 6.0).max(0.0)),
            position.y.min((window.height() - height - 6.0).max(0.0)),
        )
    });
    let menu = commands
        .spawn((
            ContextMenu { target },
            GlobalZIndex(20_000),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left.max(0.0)),
                top: Val::Px(top.max(0.0)),
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
                                let state = selections.get(target).ok()?;
                                let range = state.range()?;
                                texts.get(target).ok()?.0.get(range).map(str::to_owned)
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
            if open.0.is_some_and(|menu| {
                authored.get(menu).is_ok() && is_inside(press.entity, menu, &parents)
            }) {
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
                .computed_nodes
                .get(entity)
                .ok()
                .map(ComputedNode::size)
                .filter(|size| size.x > 0.0 && size.y > 0.0)
                .unwrap_or(bevy::math::Vec2::new(180.0, 46.0));
            let (left, top) = windows
                .iter()
                .next()
                .map_or((position.x, position.y), |window| {
                    (
                        position.x.min((window.width() - size.x - 6.0).max(0.0)),
                        position.y.min((window.height() - size.y - 6.0).max(0.0)),
                    )
                });
            node.position_type = PositionType::Absolute;
            node.left = Val::Px(left.max(0.0));
            node.top = Val::Px(top.max(0.0));
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
        AuthoredContextMenu, ContextMenu, OpenContextMenu, context_menu_pointer_input,
        copied_input_value, remove_orphan_context_menus,
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
        press(app.world_mut(), copy, PointerButton::Primary);
        app.update();
        let result = app
            .world_mut()
            .resource_mut::<Clipboard>()
            .fetch_text()
            .poll_result()
            .unwrap()
            .unwrap();
        assert_eq!(result, "select");
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
        assert_eq!(node.left, Val::Px(214.0));
        assert_eq!(node.top, Val::Px(248.0));
    }
}
