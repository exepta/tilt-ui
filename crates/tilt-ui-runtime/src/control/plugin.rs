use bevy::{
    app::{App, Plugin, PostUpdate, Update},
    ecs::{
        hierarchy::ChildOf,
        lifecycle::{Add, Remove},
        message::MessageReader,
        observer::On,
        query::{Changed, With},
        schedule::SystemSet,
        system::{Commands, Query, Res, ResMut},
    },
    input::{
        ButtonState,
        keyboard::{KeyCode, KeyboardInput},
    },
    prelude::IntoScheduleConfigs,
    ui::{
        Interaction, InteractionDisabled, Pressed, UiSystems,
        widget::{scroll_editable_text, update_editable_text_layout},
    },
};
use bevy_input_focus::{
    FocusCause, FocusGained, FocusLost, InputFocus, InputFocusPlugin,
    tab_navigation::{TabIndex, TabNavigationPlugin},
};
use bevy_picking::{
    events::{Click, Pointer},
    pointer::PointerButton,
};

use crate::{ControlPart, ControlTabIndex, ElementState, TiltControl};

use super::{
    ControlActivated,
    editable::{
        animate_editable_cursor, animate_input_text, edit_focused_text, editable_focus_lost,
        editable_ime, editable_pointer_drag, editable_pointer_press, sync_native_edit,
        update_ime_candidate_position, update_ime_window,
    },
    options::{option_keyboard_input, process_option_activation},
    popup::dismiss_popups_on_press,
    range::{range_keyboard_input, range_pointer_input},
    resize::text_area_resize_input,
    selection::process_selectable_activation,
};

type InteractionControlQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static mut ElementState,
        Option<&'static InteractionDisabled>,
    ),
    (With<TiltControl>, Changed<Interaction>),
>;
type DisabledControlQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ElementState,
        Option<&'static InteractionDisabled>,
        Option<&'static mut TabIndex>,
        Option<&'static ControlTabIndex>,
    ),
    (With<TiltControl>, Changed<InteractionDisabled>),
>;
type TabControlQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ElementState,
        Option<&'static mut TabIndex>,
        Option<&'static ControlTabIndex>,
    ),
    With<TiltControl>,
>;
type ActivatableControlQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut ElementState,
        Option<&'static InteractionDisabled>,
        Option<&'static mut Pressed>,
        Option<&'static crate::EditableText>,
        Option<&'static crate::NumericRange>,
    ),
    With<TiltControl>,
>;

/// Orders shared TiltUI control processing.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiltControlSystems {
    /// Projects low-level pointer interaction into CSS selector state.
    Interaction,
    /// Projects input focus into CSS selector state.
    Focus,
    /// Processes semantic control activation gestures.
    Activation,
    /// Resolves selectable control values after activation gestures.
    Selection,
}

/// Registers shared control interaction, focus, activation, and tab-navigation behavior.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiltUiControlRuntimePlugin;

impl Plugin for TiltUiControlRuntimePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<TabNavigationPlugin>() {
            app.add_plugins(TabNavigationPlugin);
        }
        app.add_message::<ControlActivated>()
            .add_message::<super::ControlCheckedChanged>()
            .add_message::<super::OptionSelectionChanged>()
            .add_message::<crate::widgets::advanced::hyperlink::LinkActivated>()
            .add_message::<crate::widgets::advanced::date_picker::DatePickerChanged>()
            .add_message::<crate::widgets::advanced::color_picker::ColorPickerChanged>()
            .add_message::<crate::EditableTextChanged>()
            .add_message::<crate::EditableTextCommitted>()
            .add_message::<crate::NumericValueChanged>()
            .add_message::<crate::widgets::state::SliderChanged>()
            .add_message::<crate::widgets::state::SliderCommitted>()
            .add_message::<crate::widgets::controls::input::FileInputSelected>()
            .add_observer(project_focus_gained)
            .add_observer(project_focus_lost)
            .add_observer(editable_focus_lost)
            .add_observer(sync_native_edit)
            .add_observer(project_disabled_added)
            .add_observer(project_disabled_removed)
            .add_systems(
                Update,
                project_interaction.in_set(TiltControlSystems::Interaction),
            )
            .add_systems(Update, super::cursor::update_ui_cursor)
            .add_systems(
                Update,
                project_disabled.in_set(TiltControlSystems::Interaction),
            )
            .add_systems(
                Update,
                pointer_activation
                    .in_set(TiltControlSystems::Activation)
                    .after(TiltControlSystems::Interaction),
            )
            .add_systems(
                Update,
                keyboard_activation
                    .in_set(TiltControlSystems::Activation)
                    .after(TiltControlSystems::Focus),
            )
            .add_systems(
                Update,
                edit_focused_text
                    .in_set(TiltControlSystems::Activation)
                    .after(TiltControlSystems::Focus),
            )
            .add_systems(
                Update,
                (
                    dismiss_popups_on_press,
                    editable_pointer_press.after(crate::scroll::ScrollSystemSet::Pointer),
                    editable_pointer_drag.after(crate::scroll::ScrollSystemSet::Pointer),
                    editable_ime,
                    update_ime_window,
                    range_pointer_input,
                    crate::widgets::advanced::color_picker::color_pointer_input,
                    range_keyboard_input,
                    text_area_resize_input,
                )
                    .in_set(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                process_selectable_activation
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                process_option_activation
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                option_keyboard_input
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                crate::widgets::advanced::hyperlink::process_link_activation
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                crate::widgets::advanced::tooltip::tooltip_pointer_input
                    .in_set(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                crate::widgets::advanced::date_picker::process_date_activation
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            )
            .add_systems(
                Update,
                (
                    crate::widgets::advanced::color_picker::process_color_activation,
                    crate::widgets::advanced::color_picker::process_color_edits,
                    crate::widgets::advanced::color_picker::process_color_commits,
                )
                    .in_set(TiltControlSystems::Selection)
                    .after(TiltControlSystems::Activation),
            );
        app.init_resource::<crate::widgets::state::UiMotionSettings>()
            .init_resource::<super::text_selection::ActiveStaticTextSelection>()
            .add_systems(
                PostUpdate,
                crate::widgets::state::animate_widget_layout.before(UiSystems::Layout),
            );
        #[cfg(feature = "file-dialog")]
        app.add_systems(
            Update,
            (
                super::file::open_file_dialogs,
                super::file::finish_file_dialogs,
            )
                .chain()
                .in_set(TiltControlSystems::Selection)
                .after(TiltControlSystems::Activation),
        );
        app.add_systems(
            PostUpdate,
            update_ime_candidate_position
                .in_set(UiSystems::PostLayout)
                .after(update_editable_text_layout)
                .after(scroll_editable_text),
        );
        app.add_systems(
            PostUpdate,
            animate_editable_cursor.after(update_ime_candidate_position),
        );
        app.add_systems(
            PostUpdate,
            animate_input_text.after(update_editable_text_layout),
        );
        app.add_systems(
            Update,
            (
                super::text_selection::select_static_text,
                super::text_selection::copy_selected_text_shortcut,
            )
                .in_set(TiltControlSystems::Activation),
        );
        super::context_menu::init(app);
        app.add_systems(
            Update,
            (
                super::context_menu::context_menu_pointer_input
                    .before(super::text_selection::select_static_text),
                super::context_menu::context_menu_keyboard_input,
                super::context_menu::style_context_menu_items,
            )
                .in_set(TiltControlSystems::Activation),
        );
        app.add_systems(
            PostUpdate,
            super::text_selection::update_static_selection_highlight.after(UiSystems::Layout),
        );
        app.add_systems(
            PostUpdate,
            crate::widgets::advanced::tooltip::place_open_tooltips
                .in_set(UiSystems::PostLayout)
                .after(UiSystems::Layout),
        );
    }
}

/// Synchronizes Bevy interaction state with CSS pseudo-state data.
fn project_interaction(mut controls: InteractionControlQuery<'_, '_>) {
    for (interaction, mut state, disabled) in &mut controls {
        if disabled.is_some() {
            continue;
        }
        let (hovered, active) = match interaction {
            Interaction::None => (false, false),
            Interaction::Hovered => (true, false),
            Interaction::Pressed => (true, true),
        };
        if state.hovered != hovered {
            state.hovered = hovered;
        }
        if state.active != active {
            state.active = active;
        }
    }
}

fn project_disabled(mut controls: DisabledControlQuery<'_, '_>) {
    for (mut state, disabled, tab_index, normal_tab_index) in &mut controls {
        let disabled = disabled.is_some();
        if state.disabled != disabled {
            state.disabled = disabled;
        }
        if let Some(mut tab_index) = tab_index {
            tab_index.0 = if disabled {
                -1
            } else {
                normal_tab_index.map_or(0, |index| index.0)
            };
        }
    }
}

fn project_disabled_added(
    trigger: On<Add, InteractionDisabled>,
    mut controls: TabControlQuery<'_, '_>,
) {
    if let Ok((mut state, tab_index, _)) = controls.get_mut(trigger.entity) {
        state.disabled = true;
        if let Some(mut tab_index) = tab_index {
            tab_index.0 = -1;
        }
    }
}

fn project_disabled_removed(
    trigger: On<Remove, InteractionDisabled>,
    mut controls: TabControlQuery<'_, '_>,
) {
    if let Ok((mut state, tab_index, normal_tab_index)) = controls.get_mut(trigger.entity) {
        state.disabled = false;
        if let Some(mut tab_index) = tab_index {
            tab_index.0 = normal_tab_index.map_or(0, |index| index.0);
        }
    }
}

fn project_focus_gained(
    trigger: On<FocusGained>,
    mut controls: Query<&mut ElementState, With<TiltControl>>,
) {
    if let Ok(mut state) = controls.get_mut(trigger.entity) {
        state.focused = true;
    }
}

fn project_focus_lost(
    trigger: On<FocusLost>,
    mut controls: Query<&mut ElementState, With<TiltControl>>,
) {
    if let Ok(mut state) = controls.get_mut(trigger.entity) {
        state.focused = false;
    }
}

fn pointer_activation(
    mut clicks: Option<MessageReader<Pointer<Click>>>,
    parts: Query<&ControlPart>,
    controls: Query<Option<&InteractionDisabled>, With<TiltControl>>,
    parents: Query<&ChildOf>,
    mut focus: Option<ResMut<InputFocus>>,
    mut activated: bevy::ecs::message::MessageWriter<ControlActivated>,
) {
    let Some(clicks) = clicks.as_mut() else {
        return;
    };
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        if parts
            .get(click.entity)
            .is_ok_and(|part| part.kind.handles_own_pointer())
        {
            continue;
        }
        if let Some(entity) = nearest_control_root(click.entity, &controls, &parents)
            && controls
                .get(entity)
                .is_ok_and(|disabled| disabled.is_none())
        {
            if let Some(focus) = focus.as_deref_mut() {
                focus.set(entity, FocusCause::Pressed);
            }
            activated.write(ControlActivated { entity });
        }
    }
}

#[cfg(test)]
fn activate_pointer_target(
    target: bevy::ecs::entity::Entity,
    controls: &Query<Option<&InteractionDisabled>, With<TiltControl>>,
    parents: &Query<&ChildOf>,
    activated: &mut bevy::ecs::message::MessageWriter<ControlActivated>,
) {
    if let Some(entity) = nearest_control_root(target, controls, parents)
        && controls
            .get(entity)
            .is_ok_and(|disabled| disabled.is_none())
    {
        activated.write(ControlActivated { entity });
    }
}

fn nearest_control_root(
    target: bevy::ecs::entity::Entity,
    controls: &Query<Option<&InteractionDisabled>, With<TiltControl>>,
    parents: &Query<&ChildOf>,
) -> Option<bevy::ecs::entity::Entity> {
    let mut current = target;
    loop {
        if controls.contains(current) {
            return Some(current);
        }
        current = parents.get(current).ok()?.parent();
    }
}

fn keyboard_activation(
    mut keys: Option<MessageReader<KeyboardInput>>,
    focus: Option<Res<InputFocus>>,
    mut controls: ActivatableControlQuery<'_, '_>,
    mut commands: Commands,
    mut activated: bevy::ecs::message::MessageWriter<ControlActivated>,
) {
    let Some(entity) = focus.and_then(|focus| focus.get()) else {
        return;
    };
    let Ok((mut state, disabled, pressed, editable, numeric)) = controls.get_mut(entity) else {
        return;
    };
    if disabled.is_some()
        || editable.is_some_and(|state| state.input_type != tilt_ui_core::InputType::File)
        || numeric.is_some()
    {
        return;
    }
    let Some(keys) = keys.as_mut() else {
        return;
    };
    for key in keys.read() {
        match (key.key_code, key.state, key.repeat) {
            (KeyCode::Enter, ButtonState::Pressed, false) => {
                activated.write(ControlActivated { entity });
            }
            (KeyCode::Space, ButtonState::Pressed, false) => {
                state.active = true;
                if pressed.is_none() {
                    commands.entity(entity).insert(Pressed);
                }
            }
            (KeyCode::Space, ButtonState::Released, _) if state.active => {
                state.active = false;
                commands.entity(entity).remove::<Pressed>();
                activated.write(ControlActivated { entity });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        ecs::{
            hierarchy::ChildOf,
            message::{MessageCursor, MessageWriter, Messages},
            system::{In, Query, RunSystemOnce},
        },
        ui::{Interaction, InteractionDisabled},
    };

    use super::{TiltUiControlRuntimePlugin, activate_pointer_target, nearest_control_root};
    use crate::{ControlActivated, ControlPart, ControlPartKind, ElementState, TiltControl};

    fn resolve_control_root(
        In(target): In<bevy::ecs::entity::Entity>,
        controls: Query<Option<&InteractionDisabled>, bevy::ecs::query::With<TiltControl>>,
        parents: Query<&ChildOf>,
    ) -> Option<bevy::ecs::entity::Entity> {
        nearest_control_root(target, &controls, &parents)
    }

    fn activate_pointer_target_for_test(
        In(target): In<bevy::ecs::entity::Entity>,
        controls: Query<Option<&InteractionDisabled>, bevy::ecs::query::With<TiltControl>>,
        parents: Query<&ChildOf>,
        mut activated: MessageWriter<ControlActivated>,
    ) {
        activate_pointer_target(target, &controls, &parents, &mut activated);
    }

    #[test]
    fn interaction_projects_hover_and_active_state_without_a_control_scan() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let entity = app
            .world_mut()
            .spawn((TiltControl, ElementState::default(), Interaction::None))
            .id();

        app.update();
        app.world_mut()
            .get_mut::<Interaction>(entity)
            .unwrap()
            .clone_from(&Interaction::Hovered);
        app.update();
        assert!(app.world().get::<ElementState>(entity).unwrap().hovered);
        assert!(!app.world().get::<ElementState>(entity).unwrap().active);

        app.world_mut()
            .get_mut::<Interaction>(entity)
            .unwrap()
            .clone_from(&Interaction::Pressed);
        app.update();
        assert!(app.world().get::<ElementState>(entity).unwrap().active);

        app.world_mut()
            .get_mut::<Interaction>(entity)
            .unwrap()
            .clone_from(&Interaction::None);
        app.update();
        assert!(!app.world().get::<ElementState>(entity).unwrap().hovered);
        assert!(!app.world().get::<ElementState>(entity).unwrap().active);
    }

    #[test]
    fn text_and_control_parts_resolve_to_the_nearest_control_root() {
        let mut app = App::new();
        app.add_message::<ControlActivated>();
        let control = app.world_mut().spawn(TiltControl).id();
        let text = app.world_mut().spawn_empty().id();
        let indicator = app
            .world_mut()
            .spawn(ControlPart {
                owner: control,
                kind: ControlPartKind::Indicator,
            })
            .id();
        let track = app
            .world_mut()
            .spawn(ControlPart {
                owner: control,
                kind: ControlPartKind::Track,
            })
            .id();
        let thumb = app
            .world_mut()
            .spawn(ControlPart {
                owner: control,
                kind: ControlPartKind::Thumb,
            })
            .id();
        app.world_mut().entity_mut(control).add_child(text);
        app.world_mut().entity_mut(control).add_child(indicator);
        app.world_mut().entity_mut(control).add_child(track);
        app.world_mut().entity_mut(track).add_child(thumb);

        for target in [text, indicator, track, thumb] {
            assert_eq!(
                app.world_mut()
                    .run_system_once_with(resolve_control_root, target)
                    .expect("control root resolver runs"),
                Some(control),
            );
            app.world_mut()
                .run_system_once_with(activate_pointer_target_for_test, target)
                .expect("pointer target activation runs");
        }
        let mut cursor = MessageCursor::<ControlActivated>::default();
        let events = cursor
            .read(app.world().resource::<Messages<ControlActivated>>())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![ControlActivated { entity: control }; 4],
            "each child target produces exactly one root activation"
        );
    }

    #[test]
    fn target_resolution_stops_at_a_nested_control() {
        let mut app = App::new();
        let outer = app.world_mut().spawn(TiltControl).id();
        let inner = app.world_mut().spawn(TiltControl).id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(outer).add_child(inner);
        app.world_mut().entity_mut(inner).add_child(child);

        assert_eq!(
            app.world_mut()
                .run_system_once_with(resolve_control_root, child)
                .expect("control root resolver runs"),
            Some(inner),
        );
    }
}
