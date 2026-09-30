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
    /// Whether the picker selects an inclusive date range.
    pub range: bool,
    /// First date of a range selection.
    pub range_start: Option<IsoDate>,
    /// Last date of a completed range selection.
    pub range_end: Option<IsoDate>,
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
    year_text: Entity,
    weekdays: Vec<Entity>,
    days: Vec<Entity>,
    months: Vec<Entity>,
    years: Vec<Entity>,
    weekday_grid: Entity,
    day_grid: Entity,
    month_grid: Entity,
    year_grid: Entity,
    view: CalendarView,
    year_page_start: u16,
}

/// Notification emitted when a user chooses a different date.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePickerChanged {
    /// DatePicker entity.
    pub entity: Entity,
    /// Newly selected ISO date.
    pub value: IsoDate,
}

/// Emitted after the user picks a range start or completes a range.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePickerRangeChanged {
    /// DatePicker entity.
    pub entity: Entity,
    /// Inclusive range start.
    pub start: IsoDate,
    /// Inclusive range end; `None` while choosing the second date.
    pub end: Option<IsoDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarView {
    Days,
    Months,
    Years,
}

/// Visual role of a day inside an inclusive range.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CalendarRangePosition {
    #[default]
    None,
    Start,
    Middle,
    End,
    Single,
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
    ShowMonths,
    ShowYears,
    Month(u8),
    Year(u16),
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

fn displayed_range(
    start: Option<IsoDate>,
    end: Option<IsoDate>,
    format: DateFormat,
    placeholder: &str,
) -> String {
    let Some(start) = start else {
        return placeholder.to_owned();
    };
    let first = displayed(Some(start), format, placeholder);
    let second = end.map_or_else(
        || "…".to_owned(),
        |end| displayed(Some(end), format, placeholder),
    );
    format!("{first} – {second}")
}

pub(crate) fn parse_range(value: &str) -> Option<(IsoDate, Option<IsoDate>)> {
    let (start, end) = value.split_once("..")?;
    let start = IsoDate::parse(start.trim())?;
    let end = if end.trim().is_empty() {
        None
    } else {
        Some(IsoDate::parse(end.trim())?)
    };
    end.is_none_or(|end| end >= start).then_some((start, end))
}

fn year_page(year: u16) -> u16 {
    (year.saturating_sub(1) / 12) * 12 + 1
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let attr = |name| crate::component::static_attribute_value(attributes, name);
    let range = attr("type") == Some("range")
        || crate::component::has_boolean_static_attribute(attributes, "range");
    let value = (!range)
        .then(|| attr("value").and_then(IsoDate::parse))
        .flatten();
    let parsed_range = attr("value").and_then(parse_range);
    let range_start = range
        .then(|| {
            attr("range-start")
                .and_then(IsoDate::parse)
                .or_else(|| parsed_range.map(|(start, _)| start))
        })
        .flatten();
    let range_end = range
        .then(|| {
            attr("range-end")
                .and_then(IsoDate::parse)
                .or_else(|| parsed_range.and_then(|(_, end)| end))
        })
        .flatten();
    let range_end = range_end.filter(|end| range_start.is_some_and(|start| *end >= start));
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
        if range {
            displayed_range(range_start, range_end, format, placeholder)
        } else {
            displayed(value, format, placeholder)
        },
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
    let month_text = action_part(
        world,
        entity,
        ControlPartKind::Label,
        "",
        CalendarActionKind::ShowMonths,
    );
    let year_text = action_part(
        world,
        entity,
        ControlPartKind::Label,
        "",
        CalendarActionKind::ShowYears,
    );
    for item in [previous, month_text, year_text, next] {
        world.entity_mut(header).add_child(item);
    }
    let weekday_grid = spawn_part(world, entity, ControlPartKind::Calendar);
    let mut weekdays = Vec::with_capacity(7);
    for index in 0..7 {
        let label = weekday_label(world, index);
        let day = spawn_text_part(world, entity, ControlPartKind::CalendarWeekday, &label);
        world.entity_mut(weekday_grid).add_child(day);
        weekdays.push(day);
    }
    let day_grid = spawn_part(world, entity, ControlPartKind::Calendar);
    let mut days = Vec::with_capacity(42);
    for _ in 0..42 {
        let day = action_part(
            world,
            entity,
            ControlPartKind::CalendarDay,
            "",
            CalendarActionKind::Day(0),
        );
        world.entity_mut(day).insert(CalendarRangePosition::None);
        world.entity_mut(day_grid).add_child(day);
        days.push(day);
    }
    let month_grid = spawn_part(world, entity, ControlPartKind::Calendar);
    let mut months = Vec::with_capacity(12);
    for month in 1..=12 {
        let cell = action_part(
            world,
            entity,
            ControlPartKind::CalendarDay,
            "",
            CalendarActionKind::Month(month),
        );
        crate::widgets::state::set_layout(world, cell, |layout| {
            layout.width = Some(bevy::ui::Val::Percent(33.3333))
        });
        world.entity_mut(month_grid).add_child(cell);
        months.push(cell);
    }
    let year_grid = spawn_part(world, entity, ControlPartKind::Calendar);
    let mut years = Vec::with_capacity(12);
    for _ in 0..12 {
        let cell = action_part(
            world,
            entity,
            ControlPartKind::CalendarDay,
            "",
            CalendarActionKind::Year(0),
        );
        crate::widgets::state::set_layout(world, cell, |layout| {
            layout.width = Some(bevy::ui::Val::Percent(25.0))
        });
        world.entity_mut(year_grid).add_child(cell);
        years.push(cell);
    }
    world
        .entity_mut(popup)
        .add_child(header)
        .add_child(weekday_grid)
        .add_child(day_grid)
        .add_child(month_grid)
        .add_child(year_grid);
    for part in [value_text, indicator, popup] {
        world.entity_mut(entity).add_child(part);
    }
    let today = value.or(range_start).unwrap_or_else(today_utc);
    world.entity_mut(entity).insert(DatePickerState {
        value,
        range,
        range_start,
        range_end,
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
        year_text,
        weekdays,
        days,
        months,
        years,
        weekday_grid,
        day_grid,
        month_grid,
        year_grid,
        view: CalendarView::Days,
        year_page_start: year_page(today.year),
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
                    .is_some_and(|text| {
                        text.input_type == InputType::Date
                            || state_is_range_text_target(world, entity, text.input_type)
                    })
            });
        if let Some(mut state) = world.get_mut::<DatePickerState>(entity) {
            state.target = target;
        }
        if let Some(target) = target {
            world.entity_mut(target).insert(DatePickerTrigger(entity));
        }
    }
}

fn state_is_range_text_target(world: &World, picker: Entity, input_type: InputType) -> bool {
    input_type == InputType::Text
        && world
            .get::<DatePickerState>(picker)
            .is_some_and(|state| state.range)
}

fn month_label(world: &World, month: u8) -> String {
    let months = [
        ("month-january", "January"),
        ("month-february", "February"),
        ("month-march", "March"),
        ("month-april", "April"),
        ("month-may", "May"),
        ("month-june", "June"),
        ("month-july", "July"),
        ("month-august", "August"),
        ("month-september", "September"),
        ("month-october", "October"),
        ("month-november", "November"),
        ("month-december", "December"),
    ];
    let (key, fallback) = months[usize::from(month.saturating_sub(1)).min(11)];
    #[cfg(not(feature = "fluent"))]
    let _ = (world, key);
    #[cfg(feature = "fluent")]
    if let Some(value) = world
        .get_resource::<crate::UiLocalization>()
        .and_then(|locale| locale.translate(key, None))
    {
        return value;
    }
    fallback.to_owned()
}

fn weekday_label(world: &World, index: usize) -> String {
    let weekdays = [
        ("weekday-monday", "Mon"),
        ("weekday-tuesday", "Tue"),
        ("weekday-wednesday", "Wed"),
        ("weekday-thursday", "Thu"),
        ("weekday-friday", "Fri"),
        ("weekday-saturday", "Sat"),
        ("weekday-sunday", "Sun"),
    ];
    let (key, fallback) = weekdays[index];
    #[cfg(not(feature = "fluent"))]
    let _ = (world, key);
    #[cfg(feature = "fluent")]
    if let Some(value) = world
        .get_resource::<crate::UiLocalization>()
        .and_then(|locale| locale.translate(key, None))
    {
        return value;
    }
    fallback.to_owned()
}

#[cfg(feature = "fluent")]
pub(crate) fn refresh_localized_calendar_labels(world: &mut World) {
    let pickers = {
        let mut query = world.query_filtered::<Entity, bevy::ecs::query::With<DatePickerState>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for picker in pickers {
        refresh_calendar(world, picker);
    }
}

fn refresh_calendar(world: &mut World, entity: Entity) {
    let Some(state) = world.get::<DatePickerState>(entity).cloned() else {
        return;
    };
    let month = month_label(world, state.displayed_month);
    if let Some(mut label) = world.get_mut::<Text>(state.month_text) {
        if label.0 != month {
            label.0 = month;
        }
    }
    let year_label = if state.view == CalendarView::Years {
        format!(
            "{}–{}",
            state.year_page_start,
            (u32::from(state.year_page_start) + 11).min(9999)
        )
    } else {
        state.displayed_year.to_string()
    };
    if let Some(mut label) = world.get_mut::<Text>(state.year_text) {
        if label.0 != year_label {
            label.0 = year_label;
        }
    }
    for (index, cell) in state.weekdays.iter().copied().enumerate() {
        let next = weekday_label(world, index);
        if let Some(mut label) = world.get_mut::<Text>(cell)
            && label.0 != next
        {
            label.0 = next;
        }
    }
    for (grid, visible) in [
        (state.weekday_grid, state.view == CalendarView::Days),
        (state.day_grid, state.view == CalendarView::Days),
        (state.month_grid, state.view == CalendarView::Months),
        (state.year_grid, state.view == CalendarView::Years),
    ] {
        crate::widgets::state::set_widget_display(world, grid, visible);
    }
    let offset = monday_offset(state.displayed_year, state.displayed_month);
    let count = days_in_month(state.displayed_year, state.displayed_month);
    let visible_cells = (offset + usize::from(count)).div_ceil(7) * 7;
    for (index, cell) in state.days.iter().copied().enumerate() {
        crate::widgets::state::set_widget_display(world, cell, index < visible_cells);
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
        let range_position = if state.range {
            match (date, state.range_start, state.range_end) {
                (Some(date), Some(start), Some(end)) if date == start && date == end => {
                    CalendarRangePosition::Single
                }
                (Some(date), Some(start), None) if date == start => CalendarRangePosition::Single,
                (Some(date), Some(start), _) if date == start => CalendarRangePosition::Start,
                (Some(date), _, Some(end)) if date == end => CalendarRangePosition::End,
                (Some(date), Some(start), Some(end)) if date > start && date < end => {
                    CalendarRangePosition::Middle
                }
                _ => CalendarRangePosition::None,
            }
        } else {
            CalendarRangePosition::None
        };
        if let Some(mut position) = world.get_mut::<CalendarRangePosition>(cell)
            && *position != range_position
        {
            *position = range_position;
        }
        if let Some(mut action) = world.get_mut::<CalendarAction>(cell) {
            action.kind = CalendarActionKind::Day(day.unwrap_or(0));
        }
        if let Some(mut text) = world.get_mut::<Text>(cell) {
            let next = day.map_or_else(String::new, |day| day.to_string());
            if text.0 != next {
                text.0 = next;
            }
        }
        if let Some(mut css) = world.get_mut::<ElementState>(cell) {
            let checked = if state.range {
                range_position != CalendarRangePosition::None
            } else {
                date == state.value
            };
            if css.checked != checked {
                css.checked = checked;
            }
            if css.disabled != disabled {
                css.disabled = disabled;
            }
        }
        if disabled && world.get::<InteractionDisabled>(cell).is_none() {
            world.entity_mut(cell).insert(InteractionDisabled);
        } else if !disabled && world.get::<InteractionDisabled>(cell).is_some() {
            world.entity_mut(cell).remove::<InteractionDisabled>();
        }
        let visibility = if day.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if world.get::<Visibility>(cell) != Some(&visibility) {
            world.entity_mut(cell).insert(visibility);
        }
    }
    for (index, cell) in state.months.iter().copied().enumerate() {
        let month = index as u8 + 1;
        let label = month_label(world, month)
            .chars()
            .take(3)
            .collect::<String>();
        if let Some(mut text) = world.get_mut::<Text>(cell) {
            if text.0 != label {
                text.0 = label;
            }
        }
        let first = IsoDate {
            year: state.displayed_year,
            month,
            day: 1,
        };
        let last = IsoDate {
            day: days_in_month(first.year, first.month),
            ..first
        };
        let disabled =
            state.min.is_some_and(|min| last < min) || state.max.is_some_and(|max| first > max);
        if let Some(mut css) = world.get_mut::<ElementState>(cell) {
            if css.checked != (state.displayed_month == month) {
                css.checked = state.displayed_month == month;
            }
            if css.disabled != disabled {
                css.disabled = disabled;
            }
        }
        if disabled && world.get::<InteractionDisabled>(cell).is_none() {
            world.entity_mut(cell).insert(InteractionDisabled);
        } else if !disabled && world.get::<InteractionDisabled>(cell).is_some() {
            world.entity_mut(cell).remove::<InteractionDisabled>();
        }
    }
    for (index, cell) in state.years.iter().copied().enumerate() {
        let year = u32::from(state.year_page_start) + index as u32;
        let valid = (1..=9999).contains(&year);
        let disabled = !valid
            || state.min.is_some_and(|min| year < u32::from(min.year))
            || state.max.is_some_and(|max| year > u32::from(max.year));
        if let Some(mut action) = world.get_mut::<CalendarAction>(cell) {
            action.kind = CalendarActionKind::Year(if valid { year as u16 } else { 0 });
        }
        if let Some(mut text) = world.get_mut::<Text>(cell) {
            let next = if valid {
                year.to_string()
            } else {
                String::new()
            };
            if text.0 != next {
                text.0 = next;
            }
        }
        if let Some(mut css) = world.get_mut::<ElementState>(cell) {
            if css.checked != (year == u32::from(state.displayed_year)) {
                css.checked = year == u32::from(state.displayed_year);
            }
            if css.disabled != disabled {
                css.disabled = disabled;
            }
        }
        if disabled && world.get::<InteractionDisabled>(cell).is_none() {
            world.entity_mut(cell).insert(InteractionDisabled);
        } else if !disabled && world.get::<InteractionDisabled>(cell).is_some() {
            world.entity_mut(cell).remove::<InteractionDisabled>();
        }
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
    if open {
        state.view = CalendarView::Days;
    }
    let popup = state.popup;
    if let Some(mut css) = world.get_mut::<ElementState>(entity) {
        css.open = open;
    }
    crate::widgets::state::set_widget_display(world, popup, open);
    if open {
        refresh_calendar(world, entity);
    }
    true
}

/// Sets a valid date without emitting a user-originated change event.
pub fn set_date_value(world: &mut World, entity: Entity, value: Option<IsoDate>) -> bool {
    let Some(state) = world.get::<DatePickerState>(entity) else {
        return false;
    };
    if state.range
        || state.value == value
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
        state.year_page_start = year_page(value.year);
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

/// Sets an inclusive date range without emitting a user-originated message.
/// A missing end keeps the picker ready for the second click.
pub fn set_date_range(
    world: &mut World,
    entity: Entity,
    start: Option<IsoDate>,
    end: Option<IsoDate>,
) -> bool {
    let Some(state) = world.get::<DatePickerState>(entity) else {
        return false;
    };
    if !state.range
        || state.range_start == start && state.range_end == end
        || end.is_some() && start.is_none()
        || start.zip(end).is_some_and(|(start, end)| end < start)
        || start.is_some_and(|date| {
            state.min.is_some_and(|min| date < min) || state.max.is_some_and(|max| date > max)
        })
        || end.is_some_and(|date| {
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
    state.range_start = start;
    state.range_end = end;
    if let Some(date) = end.or(start) {
        state.displayed_year = date.year;
        state.displayed_month = date.month;
        state.year_page_start = year_page(date.year);
    }
    let display = displayed_range(start, end, format, &placeholder);
    if let Some(mut label) = world.get_mut::<Text>(text) {
        label.0 = display;
    }
    if let Some(target) = target {
        let value = start.map_or_else(String::new, |start| {
            end.map_or_else(
                || start.iso(),
                |end| format!("{}..{}", start.iso(), end.iso()),
            )
        });
        crate::set_editable_text(world, target, value);
    }
    refresh_calendar(world, entity);
    true
}

fn change_month(world: &mut World, entity: Entity, direction: i8) {
    let Some(mut state) = world.get_mut::<DatePickerState>(entity) else {
        return;
    };
    let (year, month) = (state.displayed_year, state.displayed_month);
    if state.view == CalendarView::Years {
        state.year_page_start = (i32::from(state.year_page_start) + i32::from(direction) * 12)
            .clamp(1, i32::from(year_page(9999))) as u16;
        refresh_calendar(world, entity);
        return;
    }
    if state.view == CalendarView::Months {
        state.displayed_year = (i32::from(year) + i32::from(direction)).clamp(1, 9999) as u16;
        state.year_page_start = year_page(state.displayed_year);
        refresh_calendar(world, entity);
        return;
    }
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
        state.year_page_start = year_page(year);
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
                    CalendarActionKind::ShowMonths => {
                        if let Some(mut state) = world.get_mut::<DatePickerState>(action.owner) {
                            state.view = CalendarView::Months;
                        }
                        refresh_calendar(world, action.owner);
                    }
                    CalendarActionKind::ShowYears => {
                        if let Some(mut state) = world.get_mut::<DatePickerState>(action.owner) {
                            state.view = CalendarView::Years;
                            state.year_page_start = year_page(state.displayed_year);
                        }
                        refresh_calendar(world, action.owner);
                    }
                    CalendarActionKind::Month(month)
                        if world.get::<InteractionDisabled>(entity).is_none() =>
                    {
                        if let Some(mut state) = world.get_mut::<DatePickerState>(action.owner) {
                            state.displayed_month = month;
                            state.view = CalendarView::Days;
                        }
                        refresh_calendar(world, action.owner);
                    }
                    CalendarActionKind::Year(year)
                        if year > 0 && world.get::<InteractionDisabled>(entity).is_none() =>
                    {
                        if let Some(mut state) = world.get_mut::<DatePickerState>(action.owner) {
                            state.displayed_year = year;
                            state.year_page_start = year_page(year);
                            state.view = CalendarView::Months;
                        }
                        refresh_calendar(world, action.owner);
                    }
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
                        let range = state.range;
                        let previous_start = state.range_start;
                        let previous_end = state.range_end;
                        if range {
                            let (start, end) = match (previous_start, previous_end) {
                                (Some(start), None) if date >= start => (start, Some(date)),
                                _ => (date, None),
                            };
                            if set_date_range(world, action.owner, Some(start), end) {
                                world
                                    .resource_mut::<Messages<DatePickerRangeChanged>>()
                                    .write(DatePickerRangeChanged {
                                        entity: action.owner,
                                        start,
                                        end,
                                    });
                            }
                            if end.is_none() {
                                return;
                            }
                        } else if set_date_value(world, action.owner, Some(date)) {
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

    #[cfg(feature = "fluent")]
    #[test]
    fn calendar_month_follows_the_selected_locale() {
        let mut world = bevy::ecs::world::World::new();
        let mut localization = crate::UiLocalization::new("en-US").unwrap();
        localization
            .insert_ftl("en-US", "month-march = March")
            .unwrap();
        localization
            .insert_ftl("de-DE", "month-march = März")
            .unwrap();
        world.insert_resource(localization);
        assert_eq!(super::month_label(&world, 3), "March");
        world
            .resource_mut::<crate::UiLocalization>()
            .set_locale("de-DE")
            .unwrap();
        assert_eq!(super::month_label(&world, 3), "März");
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
    fn year_then_month_selection_reaches_the_requested_calendar_without_rebuilding() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let picker = app
            .world_mut()
            .spawn((Node::default(), TiltControl, ElementState::default()))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "2026-09-24".into(),
                },
                TemplateAttribute::Static {
                    name: "min".into(),
                    value: "2025-01-01".into(),
                },
                TemplateAttribute::Static {
                    name: "max".into(),
                    value: "2027-12-31".into(),
                },
            ],
        );
        let original = app.world().get::<DatePickerState>(picker).unwrap().clone();
        let activate = |app: &mut App, entity| {
            app.world_mut()
                .resource_mut::<Messages<ControlActivated>>()
                .write(ControlActivated { entity });
            app.update();
        };
        activate(&mut app, original.year_text);
        let state = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(state.view, super::CalendarView::Years);
        assert_eq!(
            app.world().get::<Node>(state.day_grid).unwrap().display,
            Display::None
        );
        let year = state
            .years
            .iter()
            .copied()
            .find(|entity| {
                matches!(
                    app.world()
                        .get::<super::CalendarAction>(*entity)
                        .map(|action| action.kind),
                    Some(super::CalendarActionKind::Year(2027))
                )
            })
            .unwrap();
        activate(&mut app, year);
        let state = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(state.view, super::CalendarView::Months);
        assert_eq!(state.displayed_year, 2027);
        let february = state.months[1];
        activate(&mut app, february);
        let state = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(state.view, super::CalendarView::Days);
        assert_eq!((state.displayed_year, state.displayed_month), (2027, 2));
        assert_eq!(state.days, original.days);
        assert_eq!(
            app.world().get::<Node>(state.month_grid).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn range_selection_emits_start_then_complete_end_and_keeps_popup_open_between_clicks() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let picker = app
            .world_mut()
            .spawn((Node::default(), TiltControl, ElementState::default()))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "range".into(),
                },
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "2026-09-05..2026-09-09".into(),
                },
            ],
        );
        assert!(super::set_date_picker_open(app.world_mut(), picker, true));
        let state = app.world().get::<DatePickerState>(picker).unwrap().clone();
        let day = |number: usize| state.days[monday_offset(2026, 9) + number - 1];
        let mut cursor = MessageCursor::<super::DatePickerRangeChanged>::default();
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: day(10) });
        app.update();
        let state = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(state.range_start, IsoDate::parse("2026-09-10"));
        assert_eq!(state.range_end, None);
        assert!(state.open);
        assert_eq!(
            app.world().get::<super::CalendarRangePosition>(day(10)),
            Some(&super::CalendarRangePosition::Single)
        );
        let first = cursor
            .read(
                app.world()
                    .resource::<Messages<super::DatePickerRangeChanged>>(),
            )
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].end, None);
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: day(22) });
        app.update();
        let state = app.world().get::<DatePickerState>(picker).unwrap();
        assert_eq!(state.range_end, IsoDate::parse("2026-09-22"));
        assert!(!state.open);
        assert!(app.world().get::<ElementState>(day(16)).unwrap().checked);
        assert_eq!(
            app.world().get::<super::CalendarRangePosition>(day(10)),
            Some(&super::CalendarRangePosition::Start)
        );
        assert_eq!(
            app.world().get::<super::CalendarRangePosition>(day(16)),
            Some(&super::CalendarRangePosition::Middle)
        );
        assert_eq!(
            app.world().get::<super::CalendarRangePosition>(day(22)),
            Some(&super::CalendarRangePosition::End)
        );
        let second = cursor
            .read(
                app.world()
                    .resource::<Messages<super::DatePickerRangeChanged>>(),
            )
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].end, IsoDate::parse("2026-09-22"));
        assert!(!super::set_date_range(
            app.world_mut(),
            picker,
            IsoDate::parse("2026-09-24"),
            IsoDate::parse("2026-09-01")
        ));
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
