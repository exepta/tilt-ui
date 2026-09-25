//! Persistent Gregorian calendar and ISO date selection for TiltUI.

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        message::{Message, MessageReader, Messages},
        system::Commands,
        world::World,
    },
    prelude::Visibility,
    ui::{
        GlobalZIndex, InteractionDisabled,
        widget::{Button, Text},
    },
};
use bevy_input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
use bevy_picking::Pickable;
use tilt_ui_core::{DateFormat, InputType, TemplateAttribute};

use crate::{
    ComponentElementIds, ComponentStyleOwner, ControlActivated, ControlPartKind, ElementState,
    TiltControl,
    widgets::controls::{spawn_part, spawn_text_part},
};

/// An ISO Gregorian calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IsoDate {
    /// Four-digit year.
    pub year: u16,
    /// Month from 1 to 12.
    pub month: u8,
    /// Day within the month.
    pub day: u8,
}

impl IsoDate {
    /// Parses a valid `YYYY-MM-DD` date.
    pub fn parse(value: &str) -> Option<Self> {
        let (year, rest) = value.split_once('-')?;
        let (month, day) = rest.split_once('-')?;
        if year.len() != 4 || month.len() != 2 || day.len() != 2 {
            return None;
        }
        let date = Self {
            year: year.parse().ok()?,
            month: month.parse().ok()?,
            day: day.parse().ok()?,
        };
        (date.year > 0 && date.day > 0 && date.day <= days_in_month(date.year, date.month))
            .then_some(date)
    }

    /// Formats the date as a stable ISO value.
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

fn monday_offset(year: u16, month: u8) -> usize {
    let mut year = i32::from(year);
    if month < 3 {
        year -= 1;
    }
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let sunday = (year + year / 4 - year / 100 + year / 400 + offsets[usize::from(month - 1)] + 1)
        .rem_euclid(7);
    (sunday as usize + 6) % 7
}

fn today_utc() -> IsoDate {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| (elapsed.as_secs() / 86_400) as i64);
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    IsoDate {
        year: year.clamp(1, 9999) as u16,
        month: month as u8,
        day: day as u8,
    }
}

/// Stores the current value and persistent calendar entities.
#[derive(Component, Debug, Clone)]
pub struct DatePickerState {
    /// Selected ISO date, if any.
    pub value: Option<IsoDate>,
    /// Inclusive lower bound.
    pub min: Option<IsoDate>,
    /// Inclusive upper bound.
    pub max: Option<IsoDate>,
    /// Display order independent of the ISO semantic value.
    pub format: DateFormat,
    /// Whether the calendar is currently visible.
    pub open: bool,
    displayed_year: u16,
    displayed_month: u8,
    target_id: Option<String>,
    target: Option<Entity>,
    placeholder: String,
    value_text: Entity,
    popup: Entity,
    month_text: Entity,
    days: Vec<Entity>,
}

/// Notification emitted when a user chooses a different date.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePickerChanged {
    /// DatePicker entity.
    pub entity: Entity,
    /// Newly selected ISO date.
    pub value: IsoDate,
}

#[derive(Component, Debug, Clone, Copy)]
struct CalendarAction {
    owner: Entity,
    kind: CalendarActionKind,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct DatePickerTrigger(pub Entity);

#[derive(Debug, Clone, Copy)]
enum CalendarActionKind {
    Previous,
    Next,
    Day(u8),
}

fn action_part(
    world: &mut World,
    owner: Entity,
    kind: ControlPartKind,
    label: &str,
    action: CalendarActionKind,
) -> Entity {
    let entity = spawn_text_part(world, owner, kind, label);
    world.entity_mut(entity).insert((
        Button,
        TiltControl,
        ElementState::default(),
        TabIndex(-1),
        Pickable::default(),
        CalendarAction {
            owner,
            kind: action,
        },
    ));
    entity
}

fn displayed(date: Option<IsoDate>, format: DateFormat, placeholder: &str) -> String {
    let Some(date) = date else {
        return placeholder.to_owned();
    };
    match format {
        DateFormat::MonthDayYear => format!("{:02}/{:02}/{:04}", date.month, date.day, date.year),
        DateFormat::DayMonthYear => format!("{:02}/{:02}/{:04}", date.day, date.month, date.year),
        DateFormat::YearMonthDay => date.iso(),
    }
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let attr = |name| crate::component::static_attribute_value(attributes, name);
    let value = attr("value").and_then(IsoDate::parse);
    let min = attr("min").and_then(IsoDate::parse);
    let max = attr("max").and_then(IsoDate::parse);
    let format = match attr("format") {
        Some("dmy") => DateFormat::DayMonthYear,
        Some("ymd") => DateFormat::YearMonthDay,
        _ => DateFormat::MonthDayYear,
    };
    let placeholder = attr("placeholder").unwrap_or("Select date");
    let value_text = spawn_text_part(
        world,
        entity,
        ControlPartKind::Value,
        displayed(value, format, placeholder),
    );
    let indicator = spawn_text_part(world, entity, ControlPartKind::Indicator, "▦");
    let popup = spawn_part(world, entity, ControlPartKind::Popup);
    world.entity_mut(popup).insert(GlobalZIndex(100));
    crate::widgets::state::set_widget_display(world, popup, false);
    let header = spawn_part(world, entity, ControlPartKind::CalendarHeader);
    let previous = action_part(
        world,
        entity,
        ControlPartKind::CalendarPrevious,
        "‹",
        CalendarActionKind::Previous,
    );
    let next = action_part(
        world,
        entity,
        ControlPartKind::CalendarNext,
        "›",
        CalendarActionKind::Next,
    );
    let month_text = spawn_text_part(world, entity, ControlPartKind::Label, "");
    for item in [previous, month_text, next] {
        world.entity_mut(header).add_child(item);
    }
    let calendar = spawn_part(world, entity, ControlPartKind::Calendar);
    let mut days = Vec::with_capacity(42);
    for _ in 0..42 {
        let day = action_part(
            world,
            entity,
            ControlPartKind::CalendarDay,
            "",
            CalendarActionKind::Day(0),
        );
        world.entity_mut(calendar).add_child(day);
        days.push(day);
    }
    world
        .entity_mut(popup)
        .add_child(header)
        .add_child(calendar);
    for part in [value_text, indicator, popup] {
        world.entity_mut(entity).add_child(part);
    }
    let today = value.unwrap_or_else(today_utc);
    world.entity_mut(entity).insert(DatePickerState {
        value,
        min,
        max,
        format,
        open: false,
        displayed_year: today.year,
        displayed_month: today.month,
        target_id: attr("for").map(str::to_owned),
        target: None,
        placeholder: placeholder.to_owned(),
        value_text,
        popup,
        month_text,
        days,
    });
    refresh_calendar(world, entity);
}

pub(crate) fn resolve_targets(world: &mut World, scope: Entity) {
    let unresolved = {
        let mut query = world.query::<(Entity, &DatePickerState, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .filter_map(|(entity, state, _)| state.target_id.clone().map(|id| (entity, id)))
            .collect::<Vec<_>>()
    };
    for (entity, id) in unresolved {
        let target = world
            .get::<ComponentElementIds>(scope)
            .and_then(|ids| ids.get(&id))
            .filter(|target| {
                world
                    .get::<crate::EditableText>(*target)
                    .is_some_and(|text| text.input_type == InputType::Date)
            });
        if let Some(mut state) = world.get_mut::<DatePickerState>(entity) {
            state.target = target;
        }
        if let Some(target) = target {
            world.entity_mut(target).insert(DatePickerTrigger(entity));
        }
    }
}

fn refresh_calendar(world: &mut World, entity: Entity) {
    let Some(state) = world.get::<DatePickerState>(entity).cloned() else {
        return;
    };
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    if let Some(mut label) = world.get_mut::<Text>(state.month_text) {
        label.0 = format!(
            "{} {}",
            months[usize::from(state.displayed_month - 1)],
            state.displayed_year
        );
    }
    let offset = monday_offset(state.displayed_year, state.displayed_month);
    let count = days_in_month(state.displayed_year, state.displayed_month);
    for (index, cell) in state.days.iter().copied().enumerate() {
        let day = index
            .checked_sub(offset)
            .and_then(|index| u8::try_from(index + 1).ok())
            .filter(|day| *day <= count);
        let date = day.map(|day| IsoDate {
            year: state.displayed_year,
            month: state.displayed_month,
            day,
        });
        let disabled = date.is_some_and(|date| {
            state.min.is_some_and(|min| date < min) || state.max.is_some_and(|max| date > max)
        });
        if let Some(mut action) = world.get_mut::<CalendarAction>(cell) {
            action.kind = CalendarActionKind::Day(day.unwrap_or(0));
        }
        if let Some(mut text) = world.get_mut::<Text>(cell) {
            text.0 = day.map_or_else(String::new, |day| day.to_string());
        }
        if let Some(mut css) = world.get_mut::<ElementState>(cell) {
            css.checked = date == state.value;
            css.disabled = disabled;
        }
        if disabled {
            world.entity_mut(cell).insert(InteractionDisabled);
        } else {
            world.entity_mut(cell).remove::<InteractionDisabled>();
        }
        world.entity_mut(cell).insert(if day.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
}

/// Opens or closes the existing calendar popup.
pub fn set_date_picker_open(world: &mut World, entity: Entity, open: bool) -> bool {
    let Some(mut state) = world.get_mut::<DatePickerState>(entity) else {
        return false;
    };
    if state.open == open {
        return false;
    }
    state.open = open;
    let popup = state.popup;
    if let Some(mut css) = world.get_mut::<ElementState>(entity) {
        css.open = open;
    }
    crate::widgets::state::set_widget_display(world, popup, open);
    true
}

/// Sets a valid date without emitting a user-originated change event.
pub fn set_date_value(world: &mut World, entity: Entity, value: Option<IsoDate>) -> bool {
    let Some(state) = world.get::<DatePickerState>(entity) else {
        return false;
    };
    if state.value == value
        || value.is_some_and(|date| {
            state.min.is_some_and(|min| date < min) || state.max.is_some_and(|max| date > max)
        })
    {
        return false;
    }
    let (format, text, target, placeholder) = (
        state.format,
        state.value_text,
        state.target,
        state.placeholder.clone(),
    );
    let mut state = world.get_mut::<DatePickerState>(entity).unwrap();
    state.value = value;
    if let Some(value) = value {
        state.displayed_year = value.year;
        state.displayed_month = value.month;
    }
    if let Some(mut label) = world.get_mut::<Text>(text) {
        label.0 = displayed(value, format, &placeholder);
    }
    if let Some(target) = target {
        crate::set_editable_text(world, target, value.map_or_else(String::new, IsoDate::iso));
    }
    refresh_calendar(world, entity);
    true
}

fn change_month(world: &mut World, entity: Entity, direction: i8) {
    let Some(mut state) = world.get_mut::<DatePickerState>(entity) else {
        return;
    };
    let (year, month) = (state.displayed_year, state.displayed_month);
    let next = match direction {
        -1 if month == 1 && year > 1 => Some((year - 1, 12)),
        -1 if month > 1 => Some((year, month - 1)),
        1 if month == 12 && year < 9999 => Some((year + 1, 1)),
        1 if month < 12 => Some((year, month + 1)),
        _ => None,
    };
    if let Some((year, month)) = next {
        state.displayed_year = year;
        state.displayed_month = month;
        refresh_calendar(world, entity);
    }
}

pub(crate) fn process_date_activation(
    mut activated: MessageReader<ControlActivated>,
    mut commands: Commands,
) {
    for activation in activated.read() {
        let entity = activation.entity;
        commands.queue(move |world: &mut World| {
            if let Some(action) = world.get::<CalendarAction>(entity).copied() {
                if world.get::<InteractionDisabled>(action.owner).is_some() {
                    return;
                }
                match action.kind {
                    CalendarActionKind::Previous => change_month(world, action.owner, -1),
                    CalendarActionKind::Next => change_month(world, action.owner, 1),
                    CalendarActionKind::Day(day)
                        if day > 0 && world.get::<InteractionDisabled>(entity).is_none() =>
                    {
                        let Some(state) = world.get::<DatePickerState>(action.owner) else {
                            return;
                        };
                        let date = IsoDate {
                            year: state.displayed_year,
                            month: state.displayed_month,
                            day,
                        };
                        if set_date_value(world, action.owner, Some(date)) {
                            world.resource_mut::<Messages<DatePickerChanged>>().write(
                                DatePickerChanged {
                                    entity: action.owner,
                                    value: date,
                                },
                            );
                        }
                        set_date_picker_open(world, action.owner, false);
                        if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                            focus.set(action.owner, FocusCause::Pressed);
                        }
                    }
                    _ => {}
                }
            } else if world.get::<DatePickerState>(entity).is_some() {
                let open = !world.get::<DatePickerState>(entity).unwrap().open;
                set_date_picker_open(world, entity, open);
            } else if let Some(DatePickerTrigger(picker)) =
                world.get::<DatePickerTrigger>(entity).copied()
                && world.get::<InteractionDisabled>(picker).is_none()
            {
                let open = world
                    .get::<DatePickerState>(picker)
                    .is_some_and(|state| !state.open);
                set_date_picker_open(world, picker, open);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        ecs::message::{MessageCursor, Messages},
        ui::{Display, Node},
    };
    use tilt_ui_core::{ElementKind, TemplateAttribute};

    use super::{DatePickerChanged, DatePickerState, IsoDate, days_in_month, monday_offset};
    use crate::{
        ControlActivated, ElementState, TiltControl, TiltElement, TiltUiControlRuntimePlugin,
    };

    #[test]
    fn iso_dates_validate_leap_days_and_calendar_offsets() {
        assert_eq!(IsoDate::parse("2024-02-29").unwrap().iso(), "2024-02-29");
        assert_eq!(IsoDate::parse("2023-02-29"), None);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(monday_offset(2024, 1), 0);
    }

    #[test]
    fn calendar_selection_changes_the_existing_date_picker_and_emits_once() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let picker = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::DatePicker,
                },
                TiltControl,
                ElementState::default(),
            ))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "2026-09-24".into(),
            }],
        );
        let original = app.world().get::<DatePickerState>(picker).unwrap().clone();
        let target = original.days[monday_offset(2026, 9) + 24];
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: target });
        app.update();
        let changed = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(changed.value, IsoDate::parse("2026-09-25"));
        assert_eq!(changed.popup, original.popup);
        assert_eq!(changed.days, original.days);
        let mut cursor = MessageCursor::<DatePickerChanged>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<DatePickerChanged>>())
                .count(),
            1
        );
    }

    #[test]
    fn closed_calendar_has_no_layout_or_visible_font() {
        let mut world = bevy::ecs::world::World::new();
        let picker = world.spawn((Node::default(), ElementState::default())).id();
        super::materialize(&mut world, picker, &[]);
        let popup = world.get::<DatePickerState>(picker).unwrap().popup;
        assert_eq!(world.get::<Node>(popup).unwrap().display, Display::None);
        world
            .entity_mut(popup)
            .insert(crate::style::RuntimeComputedStyle(
                tilt_ui_css::ComputedStyle {
                    display: Some(tilt_ui_css::Display::Block),
                    ..Default::default()
                },
            ));
        assert!(super::set_date_picker_open(&mut world, picker, true));
        assert_eq!(world.get::<Node>(popup).unwrap().display, Display::Block);
        assert!(super::set_date_picker_open(&mut world, picker, false));
        assert_eq!(world.get::<Node>(popup).unwrap().display, Display::None);
    }
}
