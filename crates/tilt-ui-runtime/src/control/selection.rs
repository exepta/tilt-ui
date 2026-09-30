use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::{Message, MessageReader, MessageWriter},
        system::{Commands, Query, SystemParam},
        world::World,
    },
    reflect::Reflect,
    ui::{Checked, InteractionDisabled},
};
use tilt_ui_core::{ElementKind, FieldKind, FieldMode, TemplateAttribute};

use crate::{ElementState, TiltElement};

use super::{ControlActivated, TiltCheckbox, TiltRadioButton, TiltSwitchButton, TiltToggleButton};

/// Stores the boolean semantic value of a selectable control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub struct ControlChecked(pub bool);

/// Defines the selection semantics of a materialized field set.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSetSelection {
    kind: FieldKind,
    mode: FieldMode,
    allow_none: bool,
}

impl Default for FieldSetSelection {
    fn default() -> Self {
        Self {
            kind: FieldKind::Radio,
            mode: FieldMode::Single,
            allow_none: false,
        }
    }
}

impl FieldSetSelection {
    /// Builds field-set selection semantics from static template attributes.
    pub(crate) fn from_attributes(attributes: &[TemplateAttribute]) -> Self {
        let mut selection = Self::default();
        for attribute in attributes {
            let TemplateAttribute::Static { name, value } = attribute else {
                continue;
            };
            match (name.as_str(), value.as_str()) {
                ("kind" | "field-kind", "radio") => selection.kind = FieldKind::Radio,
                ("kind" | "field-kind", "toggle") => selection.kind = FieldKind::Toggle,
                ("mode" | "field-mode", "multi") => selection.mode = FieldMode::Multi,
                ("mode" | "field-mode", "single") => selection.mode = FieldMode::Single,
                ("allow-none", "true" | "") => selection.allow_none = true,
                ("allow-none", "false") => selection.allow_none = false,
                ("mode" | "field-mode", value)
                    if value.starts_with("count(") && value.ends_with(')') =>
                {
                    if let Ok(count) = value[6..value.len() - 1].parse::<u8>() {
                        selection.mode = FieldMode::Count(count);
                    }
                }
                ("mode" | "field-mode", "count") => selection.mode = FieldMode::Count(0),
                _ => {}
            }
        }
        selection
    }

    fn is_radio_single(self) -> bool {
        self.kind == FieldKind::Radio && self.mode == FieldMode::Single
    }
}

/// Notification emitted when user interaction changes a control's checked state.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct ControlCheckedChanged {
    /// Entity whose semantic value changed.
    pub entity: Entity,
    /// New semantic checked value.
    pub checked: bool,
}

/// Identifies an internal visual part owned by a TiltUI control.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPart {
    /// Control entity that owns this structural part.
    pub owner: Entity,
    /// Semantic role of the internal part.
    pub kind: ControlPartKind,
}

/// Classifies the persistent visual parts currently used by native controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPartKind {
    /// CSS-stylable spinner on a loading button.
    Spinner,
    /// A checked indicator used by checkbox and radio controls.
    Indicator,
    /// A switch track that contains the thumb.
    Track,
    /// A switch thumb nested inside its track.
    Thumb,
    /// A persistent checked mark nested inside an indicator.
    Mark,
    /// A value-dependent fill used by numeric controls.
    Fill,
    /// Text displaying the current editable value.
    Value,
    /// Text displayed while an editable value is empty.
    Placeholder,
    /// Visual insertion caret for editable text controls.
    Cursor,
    /// Visual selected-text region for editable text controls.
    Selection,
    /// One persistent slider interval marker.
    Dot,
    /// A persistent slider endpoint label.
    Label,
    /// A persistent slider value tip.
    Tooltip,
    /// Increase a number input by one configured step.
    Increment,
    /// Decrease a number input by one configured step.
    Decrement,
    /// A persistent popup surface for a choice control.
    Popup,
    /// Container for the calendar's day cells.
    Calendar,
    /// Calendar month label and navigation row.
    CalendarHeader,
    /// Abbreviated weekday in a date-picker calendar.
    CalendarWeekday,
    /// Previous-month action.
    CalendarPrevious,
    /// Next-month action.
    CalendarNext,
    /// One persistent calendar day cell.
    CalendarDay,
    /// Current color preview.
    Preview,
    /// One selectable color swatch.
    Swatch,
    /// Saturation and value color canvas.
    ColorCanvas,
    /// Selection marker on the color canvas.
    ColorCanvasThumb,
    /// Hue gradient track.
    HueTrack,
    /// Selection marker on the hue track.
    HueThumb,
    /// Alpha gradient track.
    AlphaTrack,
    /// Selection marker on the alpha track.
    AlphaThumb,
    /// One color-format selector.
    ColorFormat,
    /// Container holding color output-format selectors.
    ColorFormats,
    /// Container holding preset color swatches.
    ColorSwatches,
    /// Container holding recently used colors.
    RecentColors,
    /// One persistent recent-color swatch.
    RecentColor,
    /// Interactive lower-right text-area resize handle.
    ResizeHandle,
    /// Vertical scrollbar track owned by a scrollable element.
    ScrollbarYTrack,
    /// Vertical scrollbar thumb owned by a scrollable element.
    ScrollbarYThumb,
    /// Horizontal scrollbar track owned by a scrollable element.
    ScrollbarXTrack,
    /// Horizontal scrollbar thumb owned by a scrollable element.
    ScrollbarXThumb,
}

impl ControlPartKind {
    pub(crate) const fn handles_own_pointer(self) -> bool {
        matches!(
            self,
            Self::ResizeHandle
                | Self::ColorCanvas
                | Self::ColorCanvasThumb
                | Self::HueTrack
                | Self::HueThumb
                | Self::AlphaTrack
                | Self::AlphaThumb
                | Self::ScrollbarYTrack
                | Self::ScrollbarYThumb
                | Self::ScrollbarXTrack
                | Self::ScrollbarXThumb
        )
    }
}

/// Changes a selectable control value without emitting a user-facing value-change message.
///
/// Returns `true` when the entity owns a selectable value and the value changed.
pub fn set_control_checked(world: &mut World, entity: Entity, checked: bool) -> bool {
    let changed = match world.get_mut::<ControlChecked>(entity) {
        Some(mut state) if state.0 != checked => {
            state.0 = checked;
            true
        }
        _ => false,
    };
    if !changed {
        return false;
    }
    sync_checked_projection(world, entity, checked);
    #[cfg(feature = "component")]
    crate::widgets::structure::form::mark_form_dirty(world, entity);
    true
}

type SelectableKindQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static TiltCheckbox>,
        Option<&'static TiltToggleButton>,
        Option<&'static TiltSwitchButton>,
        Option<&'static TiltRadioButton>,
    ),
>;
type SelectionValueQuery<'w, 's> =
    Query<'w, 's, (&'static mut ControlChecked, &'static mut ElementState)>;
type RadioQuery<'w, 's> = Query<'w, 's, (Entity, &'static TiltRadioButton)>;
type ToggleQuery<'w, 's> = Query<'w, 's, (Entity, &'static TiltToggleButton)>;

#[derive(SystemParam)]
pub(crate) struct SelectionParams<'w, 's> {
    kinds: SelectableKindQuery<'w, 's>,
    parents: Query<'w, 's, &'static ChildOf>,
    elements: Query<'w, 's, &'static TiltElement>,
    field_sets: Query<'w, 's, &'static FieldSetSelection>,
    values: SelectionValueQuery<'w, 's>,
    radios: RadioQuery<'w, 's>,
    toggles: ToggleQuery<'w, 's>,
    disabled: Query<'w, 's, (), bevy::ecs::query::With<InteractionDisabled>>,
    commands: Commands<'w, 's>,
    changed: MessageWriter<'w, ControlCheckedChanged>,
}

/// Processes shared control activation for selectable TiltUI controls.
pub(crate) fn process_selectable_activation(
    mut activations: MessageReader<ControlActivated>,
    mut params: SelectionParams<'_, '_>,
) {
    for activation in activations.read() {
        if params.disabled.contains(activation.entity) {
            continue;
        }
        let Ok((checkbox, toggle, switch, radio)) = params.kinds.get(activation.entity) else {
            continue;
        };
        if checkbox.is_none() && toggle.is_none() && switch.is_none() && radio.is_none() {
            continue;
        }

        if radio.is_some() {
            let field_set =
                field_set_ancestor(activation.entity, &params.parents, &params.elements);
            let selection = field_set
                .and_then(|entity| params.field_sets.get(entity).ok().copied())
                .unwrap_or_default();
            if params
                .values
                .get(activation.entity)
                .is_ok_and(|(value, _)| value.0)
            {
                if field_set.is_some() && selection.allow_none {
                    set_checked_from_user(
                        activation.entity,
                        false,
                        &mut params.values,
                        &mut params.commands,
                        &mut params.changed,
                    );
                }
                continue;
            }
            let group = radio_group(
                activation.entity,
                &params.parents,
                &params.elements,
                &params.field_sets,
            );
            for (entity, _) in &params.radios {
                if entity == activation.entity
                    || radio_group(
                        entity,
                        &params.parents,
                        &params.elements,
                        &params.field_sets,
                    ) != group
                {
                    continue;
                }
                if let Ok((mut value, mut state)) = params.values.get_mut(entity) {
                    if !value.0 {
                        continue;
                    }
                    value.0 = false;
                    state.checked = false;
                    params.commands.entity(entity).remove::<Checked>();
                    params.changed.write(ControlCheckedChanged {
                        entity,
                        checked: false,
                    });
                }
            }
            set_checked_from_user(
                activation.entity,
                true,
                &mut params.values,
                &mut params.commands,
                &mut params.changed,
            );
        } else if toggle.is_some() {
            let field_set =
                field_set_ancestor(activation.entity, &params.parents, &params.elements);
            let selection = field_set
                .and_then(|entity| params.field_sets.get(entity).ok().copied())
                .unwrap_or_default();
            let Ok((value, _)) = params.values.get(activation.entity) else {
                continue;
            };
            let checked = value.0;
            if let Some(field_set) = field_set {
                match selection.mode {
                    FieldMode::Single if checked && !selection.allow_none => continue,
                    FieldMode::Single if !checked => {
                        for (entity, _) in &params.toggles {
                            if entity == activation.entity
                                || field_set_ancestor(entity, &params.parents, &params.elements)
                                    != Some(field_set)
                            {
                                continue;
                            }
                            set_checked_from_user(
                                entity,
                                false,
                                &mut params.values,
                                &mut params.commands,
                                &mut params.changed,
                            );
                        }
                    }
                    FieldMode::Count(limit) if !checked => {
                        let selected = params
                            .toggles
                            .iter()
                            .filter(|(entity, _)| {
                                field_set_ancestor(*entity, &params.parents, &params.elements)
                                    == Some(field_set)
                                    && params.values.get(*entity).is_ok_and(|(value, _)| value.0)
                            })
                            .count();
                        if selected >= usize::from(limit) {
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            set_checked_from_user(
                activation.entity,
                !checked,
                &mut params.values,
                &mut params.commands,
                &mut params.changed,
            );
        } else if let Ok((value, _)) = params.values.get(activation.entity) {
            set_checked_from_user(
                activation.entity,
                !value.0,
                &mut params.values,
                &mut params.commands,
                &mut params.changed,
            );
        }
    }
}

fn set_checked_from_user(
    entity: Entity,
    checked: bool,
    values: &mut SelectionValueQuery<'_, '_>,
    commands: &mut Commands,
    changed: &mut MessageWriter<ControlCheckedChanged>,
) {
    let Ok((mut value, mut state)) = values.get_mut(entity) else {
        return;
    };
    if value.0 == checked {
        return;
    }
    value.0 = checked;
    state.checked = checked;
    if checked {
        commands.entity(entity).insert(Checked);
    } else {
        commands.entity(entity).remove::<Checked>();
    }
    changed.write(ControlCheckedChanged { entity, checked });
}

fn sync_checked_projection(world: &mut World, entity: Entity, checked: bool) {
    if let Some(mut state) = world.get_mut::<ElementState>(entity) {
        state.checked = checked;
    }
    if checked {
        world.entity_mut(entity).insert(Checked);
    } else {
        world.entity_mut(entity).remove::<Checked>();
    }
}

fn radio_group(
    entity: Entity,
    parents: &Query<&ChildOf>,
    elements: &Query<&TiltElement>,
    field_sets: &Query<&FieldSetSelection>,
) -> Entity {
    let mut current = entity;
    while let Ok(parent) = parents.get(current) {
        let parent = parent.parent();
        if elements
            .get(parent)
            .is_ok_and(|element| element.kind == ElementKind::FieldSet)
        {
            return if field_sets
                .get(parent)
                .map_or(true, |selection| selection.is_radio_single())
            {
                parent
            } else {
                entity
            };
        }
        current = parent;
    }
    parents.get(entity).map_or(entity, ChildOf::parent)
}

fn field_set_ancestor(
    entity: Entity,
    parents: &Query<&ChildOf>,
    elements: &Query<&TiltElement>,
) -> Option<Entity> {
    let mut current = entity;
    while let Ok(parent) = parents.get(current) {
        current = parent.parent();
        if elements
            .get(current)
            .is_ok_and(|element| element.kind == ElementKind::FieldSet)
        {
            return Some(current);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy::{
        app::{App, Update},
        ecs::{
            hierarchy::ChildOf,
            message::{MessageCursor, MessageWriter, Messages},
            system::{In, RunSystemOnce},
        },
        ui::{Checked, InteractionDisabled},
    };

    use super::{
        ControlChecked, ControlCheckedChanged, TiltCheckbox, TiltRadioButton, TiltSwitchButton,
        TiltToggleButton, process_selectable_activation,
    };
    use crate::{ControlActivated, ElementState, TiltElement};
    use tilt_ui_core::{ElementKind, TemplateNodeKind};
    use tilt_ui_html::parse_template;

    fn emit_activation(
        In(entity): In<bevy::ecs::entity::Entity>,
        mut writer: MessageWriter<ControlActivated>,
    ) {
        writer.write(ControlActivated { entity });
    }

    fn materialize_reported_template(
        app: &mut App,
    ) -> HashMap<ElementKind, Vec<bevy::ecs::entity::Entity>> {
        let template = parse_template(
            r#"<div class="root"><p class="title">TiltUI CSS works</p><app-header /><button class="primary">Play</button><button class="primary" disabled="true">Disabled</button><p class="section-title">Selectable controls</p><checkbox class="checkbox" checked="true">Music</checkbox><toggle-button class="toggle">Bold</toggle-button><switch-button class="switch" checked="true">Notifications</switch-button><field-set class="radios"><radio-button class="radio" checked="true">Low</radio-button><radio-button class="radio">High</radio-button></field-set></div>"#,
        )
        .expect("reported template parses");
        let mut entities = vec![None; template.nodes().len()];
        let mut by_kind = HashMap::new();
        for (index, node) in template.nodes().iter().enumerate() {
            if let TemplateNodeKind::Element(kind) = node.kind {
                let entity = app.world_mut().spawn(TiltElement { kind }).id();
                crate::render::materialize_element(app.world_mut(), entity, kind, &node.attributes);
                entities[index] = Some(entity);
                by_kind.entry(kind).or_insert_with(Vec::new).push(entity);
            }
        }
        for (index, node) in template.nodes().iter().enumerate() {
            let Some(entity) = entities[index] else {
                continue;
            };
            if let Some(parent) = node.parent.and_then(|parent| entities[parent.0 as usize]) {
                app.world_mut().entity_mut(parent).add_child(entity);
            }
        }
        by_kind
    }

    fn activate(app: &mut App, entity: bevy::ecs::entity::Entity) {
        app.world_mut()
            .run_system_once_with(emit_activation, entity)
            .expect("activation writer runs");
        app.update();
    }

    #[test]
    fn reported_template_initializes_and_transitions_all_selectable_controls() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let controls = materialize_reported_template(&mut app);
        let checkbox = controls[&ElementKind::Checkbox][0];
        let toggle = controls[&ElementKind::ToggleButton][0];
        let switch = controls[&ElementKind::SwitchButton][0];
        let low = controls[&ElementKind::RadioButton][0];
        let high = controls[&ElementKind::RadioButton][1];

        assert!(app.world().get::<ControlChecked>(checkbox).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(toggle).unwrap().0);
        assert!(app.world().get::<ControlChecked>(switch).unwrap().0);
        assert!(app.world().get::<ControlChecked>(low).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(high).unwrap().0);

        activate(&mut app, toggle);
        activate(&mut app, switch);
        activate(&mut app, high);
        assert!(app.world().get::<ControlChecked>(toggle).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(switch).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(low).unwrap().0);
        assert!(app.world().get::<ControlChecked>(high).unwrap().0);

        app.update();
        assert!(!app.world().get::<ControlChecked>(low).unwrap().0);
        assert!(app.world().get::<ControlChecked>(high).unwrap().0);

        activate(&mut app, low);
        assert!(app.world().get::<ControlChecked>(low).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(high).unwrap().0);

        app.update();
        assert!(!app.world().get::<ControlChecked>(switch).unwrap().0);
        assert!(app.world().get::<ControlChecked>(low).unwrap().0);
    }

    #[test]
    fn checkbox_activation_changes_the_single_authoritative_value() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let checkbox = app
            .world_mut()
            .spawn((TiltCheckbox, ControlChecked(false), ElementState::default()))
            .id();
        let mut changes = MessageCursor::<ControlCheckedChanged>::default();
        app.world_mut()
            .run_system_once_with(emit_activation, checkbox)
            .expect("activation writer runs");
        app.update();

        assert!(app.world().get::<ControlChecked>(checkbox).unwrap().0);
        assert!(app.world().get::<ElementState>(checkbox).unwrap().checked);
        assert!(app.world().get::<Checked>(checkbox).is_some());
        let events = changes
            .read(app.world().resource::<Messages<ControlCheckedChanged>>())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![ControlCheckedChanged {
                entity: checkbox,
                checked: true,
            }]
        );

        app.world_mut()
            .run_system_once_with(emit_activation, checkbox)
            .expect("activation writer runs");
        app.update();
        assert!(!app.world().get::<ControlChecked>(checkbox).unwrap().0);
        assert!(!app.world().get::<ElementState>(checkbox).unwrap().checked);
        assert!(app.world().get::<Checked>(checkbox).is_none());
    }

    #[test]
    fn disabled_selectable_controls_do_not_change_value() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let checkbox = app
            .world_mut()
            .spawn((
                TiltCheckbox,
                ControlChecked(false),
                ElementState {
                    disabled: true,
                    ..Default::default()
                },
                InteractionDisabled,
            ))
            .id();
        app.world_mut()
            .run_system_once_with(emit_activation, checkbox)
            .expect("activation writer runs");
        app.update();

        assert!(!app.world().get::<ControlChecked>(checkbox).unwrap().0);
    }

    #[test]
    fn toggle_and_switch_share_boolean_toggle_behavior() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let toggle = app
            .world_mut()
            .spawn((
                TiltToggleButton,
                ControlChecked(false),
                ElementState::default(),
            ))
            .id();
        let switch = app
            .world_mut()
            .spawn((
                TiltSwitchButton,
                ControlChecked(false),
                ElementState::default(),
            ))
            .id();
        for entity in [toggle, switch] {
            app.world_mut()
                .run_system_once_with(emit_activation, entity)
                .expect("activation writer runs");
        }
        app.update();

        assert!(app.world().get::<ControlChecked>(toggle).unwrap().0);
        assert!(app.world().get::<ControlChecked>(switch).unwrap().0);
    }

    #[test]
    fn radio_selection_is_exclusive_within_a_field_set() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let field_set = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::FieldSet,
            })
            .id();
        let a = app
            .world_mut()
            .spawn((
                TiltRadioButton,
                ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
                ChildOf(field_set),
            ))
            .id();
        let b = app
            .world_mut()
            .spawn((
                TiltRadioButton,
                ControlChecked(false),
                ElementState::default(),
                ChildOf(field_set),
            ))
            .id();
        app.world_mut()
            .run_system_once_with(emit_activation, b)
            .expect("activation writer runs");
        app.update();

        assert!(!app.world().get::<ControlChecked>(a).unwrap().0);
        assert!(app.world().get::<ControlChecked>(b).unwrap().0);
    }

    #[test]
    fn radios_in_separate_field_sets_remain_independent() {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let first = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::FieldSet,
            })
            .id();
        let second = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::FieldSet,
            })
            .id();
        let first_radio = app
            .world_mut()
            .spawn((
                TiltRadioButton,
                ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
                ChildOf(first),
            ))
            .id();
        let second_radio = app
            .world_mut()
            .spawn((
                TiltRadioButton,
                ControlChecked(false),
                ElementState::default(),
                ChildOf(second),
            ))
            .id();
        app.world_mut()
            .run_system_once_with(emit_activation, second_radio)
            .expect("activation writer runs");
        app.update();

        assert!(app.world().get::<ControlChecked>(first_radio).unwrap().0);
        assert!(app.world().get::<ControlChecked>(second_radio).unwrap().0);
    }

    #[test]
    fn field_set_count_limits_toggles_and_allow_none_deselects_radio() {
        use tilt_ui_core::{FieldMode, TemplateAttribute};

        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<ControlCheckedChanged>()
            .add_systems(Update, process_selectable_activation);
        let attributes = [TemplateAttribute::Static {
            name: "mode".into(),
            value: "count(2)".into(),
        }];
        let group = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::FieldSet,
                },
                super::FieldSetSelection::from_attributes(&attributes),
            ))
            .id();
        assert_eq!(
            app.world()
                .get::<super::FieldSetSelection>(group)
                .unwrap()
                .mode,
            FieldMode::Count(2)
        );
        let toggles = (0..3)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        TiltToggleButton,
                        ControlChecked(false),
                        ElementState::default(),
                        ChildOf(group),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        activate(&mut app, toggles[0]);
        activate(&mut app, toggles[1]);
        activate(&mut app, toggles[2]);
        assert!(app.world().get::<ControlChecked>(toggles[0]).unwrap().0);
        assert!(app.world().get::<ControlChecked>(toggles[1]).unwrap().0);
        assert!(!app.world().get::<ControlChecked>(toggles[2]).unwrap().0);

        let radio_group = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::FieldSet,
                },
                super::FieldSetSelection::from_attributes(&[TemplateAttribute::Static {
                    name: "allow-none".into(),
                    value: "true".into(),
                }]),
            ))
            .id();
        let radio = app
            .world_mut()
            .spawn((
                TiltRadioButton,
                ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
                ChildOf(radio_group),
            ))
            .id();
        activate(&mut app, radio);
        assert!(!app.world().get::<ControlChecked>(radio).unwrap().0);
    }
}
