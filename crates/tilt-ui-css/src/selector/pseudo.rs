use std::fmt;

use cssparser::ToCss;
use selectors::parser::NonTSPseudoClass;

use super::TiltUiSelectorImpl;

/// Represents a TiltUI node state pseudo-class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiltUiPseudoClass {
    /// Matches a node in the hover state.
    Hover,
    /// Matches a node in the active state.
    Active,
    /// Matches a node in the focus state.
    Focus,
    /// Matches a disabled node.
    Disabled,
    /// Matches an active loading button.
    Loading,
    /// Matches a checked node.
    Checked,
    /// Matches a readonly editable control.
    Readonly,
    /// Matches a semantically invalid control.
    Invalid,
    /// Matches an open popup control.
    Open,
    /// Matches a dialog with animated presentation enabled.
    Animated,
    /// Matches a dialog during its exit animation.
    Closing,
}

impl NonTSPseudoClass for TiltUiPseudoClass {
    type Impl = TiltUiSelectorImpl;

    fn is_active_or_hover(&self) -> bool {
        matches!(self, Self::Hover | Self::Active)
    }

    fn is_user_action_state(&self) -> bool {
        matches!(self, Self::Hover | Self::Active | Self::Focus)
    }
}

impl ToCss for TiltUiPseudoClass {
    fn to_css<W>(&self, destination: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        destination.write_str(match self {
            Self::Hover => ":hover",
            Self::Active => ":active",
            Self::Focus => ":focus",
            Self::Disabled => ":disabled",
            Self::Loading => ":loading",
            Self::Checked => ":checked",
            Self::Readonly => ":readonly",
            Self::Invalid => ":invalid",
            Self::Open => ":open",
            Self::Animated => ":animated",
            Self::Closing => ":closing",
        })
    }
}

/// Identifies a generated visual part of a TiltUI control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiltUiPseudoElement {
    /// Matches a loading button spinner.
    Spinner,
    /// Matches a control indicator.
    Indicator,
    /// Matches a control checked mark.
    Mark,
    /// Matches a switch track.
    Track,
    /// Matches a switch thumb.
    Thumb,
    /// Matches a numeric control fill.
    Fill,
    /// Matches editable control value text.
    Value,
    /// Matches editable control placeholder text.
    Placeholder,
    /// Matches an editable control caret.
    Cursor,
    /// Matches an editable control selection.
    Selection,
    /// Matches a slider interval marker.
    Dot,
    /// Matches a slider endpoint label.
    Label,
    /// Matches a slider value tip.
    Tooltip,
    /// Number input increment action.
    Increment,
    /// Number input decrement action.
    Decrement,
    /// Matches a text-area resize handle.
    ResizeHandle,
    /// Matches the vertical scrollbar track.
    ScrollbarYTrack,
    /// Matches the vertical scrollbar thumb.
    ScrollbarYThumb,
    /// Matches the horizontal scrollbar track.
    ScrollbarXTrack,
    /// Matches the horizontal scrollbar thumb.
    ScrollbarXThumb,
    /// Matches a control-owned popup surface.
    Popup,
    /// Matches a date-picker calendar grid.
    Calendar,
    /// Matches a date-picker month header.
    CalendarHeader,
    /// Matches a date-picker weekday heading.
    CalendarWeekday,
    /// Matches the previous-month action.
    CalendarPrevious,
    /// Matches the next-month action.
    CalendarNext,
    /// Matches a persistent calendar day cell.
    CalendarDay,
    /// Matches a hovered calendar day cell.
    HoveredCalendarDay,
    /// Matches a selected calendar day cell.
    SelectedCalendarDay,
    /// Matches the first day of a selected date range.
    RangeStartDay,
    /// Matches a day between the start and end of a selected date range.
    RangeMiddleDay,
    /// Matches the last day of a selected date range.
    RangeEndDay,
    /// Matches a selected range containing only one day.
    RangeSingleDay,
    /// Matches a disabled calendar day cell.
    DisabledCalendarDay,
    /// Matches the current color preview.
    Preview,
    /// Matches a selectable color swatch.
    Swatch,
    /// Matches a color saturation and value canvas.
    ColorCanvas,
    /// Matches a color canvas selection marker.
    ColorCanvasThumb,
    /// Matches a hue gradient track.
    HueTrack,
    /// Matches a hue selection marker.
    HueThumb,
    /// Matches an alpha gradient track.
    AlphaTrack,
    /// Matches an alpha selection marker.
    AlphaThumb,
    /// Matches a color output-format selector.
    ColorFormat,
    /// Matches the color output-format selector row.
    ColorFormats,
    /// Matches the preset color swatch row.
    ColorSwatches,
    /// Matches the selected color output-format selector.
    SelectedColorFormat,
    /// Matches a recent-color container.
    RecentColors,
    /// Matches a recent-color swatch.
    RecentColor,
}

impl selectors::parser::PseudoElement for TiltUiPseudoElement {
    type Impl = TiltUiSelectorImpl;
}

impl ToCss for TiltUiPseudoElement {
    fn to_css<W>(&self, destination: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        destination.write_str(match self {
            Self::Spinner => "::spinner",
            Self::Indicator => "::indicator",
            Self::Mark => "::mark",
            Self::Track => "::track",
            Self::Thumb => "::thumb",
            Self::Fill => "::fill",
            Self::Value => "::value",
            Self::Placeholder => "::placeholder",
            Self::Cursor => "::cursor",
            Self::Selection => "::selection",
            Self::Dot => "::dot",
            Self::Label => "::label",
            Self::Tooltip => "::tooltip",
            Self::Increment => "::increment",
            Self::Decrement => "::decrement",
            Self::ResizeHandle => "::resize-handle",
            Self::ScrollbarYTrack => "::scrollbar-y-track",
            Self::ScrollbarYThumb => "::scrollbar-y-thumb",
            Self::ScrollbarXTrack => "::scrollbar-x-track",
            Self::ScrollbarXThumb => "::scrollbar-x-thumb",
            Self::Popup => "::popup",
            Self::Calendar => "::calendar",
            Self::CalendarHeader => "::calendar-header",
            Self::CalendarWeekday => "::weekday",
            Self::CalendarPrevious => "::calendar-previous",
            Self::CalendarNext => "::calendar-next",
            Self::CalendarDay => "::day",
            Self::HoveredCalendarDay => "::hovered-day",
            Self::SelectedCalendarDay => "::selected-day",
            Self::RangeStartDay => "::range-start-day",
            Self::RangeMiddleDay => "::range-middle-day",
            Self::RangeEndDay => "::range-end-day",
            Self::RangeSingleDay => "::range-single-day",
            Self::DisabledCalendarDay => "::disabled-day",
            Self::Preview => "::preview",
            Self::Swatch => "::swatch",
            Self::ColorCanvas => "::canvas",
            Self::ColorCanvasThumb => "::canvas-thumb",
            Self::HueTrack => "::hue-track",
            Self::HueThumb => "::hue-thumb",
            Self::AlphaTrack => "::alpha-track",
            Self::AlphaThumb => "::alpha-thumb",
            Self::ColorFormat => "::format",
            Self::ColorFormats => "::formats",
            Self::ColorSwatches => "::swatches",
            Self::SelectedColorFormat => "::selected-format",
            Self::RecentColors => "::recent-colors",
            Self::RecentColor => "::recent-color",
        })
    }
}
