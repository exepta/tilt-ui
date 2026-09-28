/// Stores values for the four physical edges of a box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

/// A solid border edge as exposed by Bevy UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderEdge {
    pub width: crate::Length,
    pub color: crate::CssColor,
}

impl<T: Copy> Edges<T> {
    /// Creates edge values with the same value on every side.
    pub const fn all(value: T) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

/// Describes the box display mode supported by the initial stylesheet subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// Participates in normal block layout.
    Block,
    /// Participates in flex layout.
    Flex,
    /// Participates in CSS grid layout.
    Grid,
    /// Does not generate a layout box.
    None,
}

/// Which box boundary `width` and `height` describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxSizing {
    ContentBox,
    BorderBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEvents {
    Auto,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssCursor {
    Auto,
    Default,
    Pointer,
    Text,
    Move,
    Wait,
    Progress,
    Crosshair,
    Help,
    Grab,
    Grabbing,
    NotAllowed,
    ColResize,
    RowResize,
}

/// The sizing function for one CSS grid track.
#[derive(Debug, Clone, PartialEq)]
pub enum GridTrackSize {
    Auto,
    MinContent,
    MaxContent,
    Length(crate::Length),
    Fraction(f32),
    MinMax(Box<GridTrackSize>, Box<GridTrackSize>),
}

/// An explicit grid track, optionally repeated as a group.
#[derive(Debug, Clone, PartialEq)]
pub struct GridTrackGroup {
    pub repetition: GridRepetition,
    pub tracks: Vec<GridTrackSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridRepetition {
    Count(u16),
    AutoFill,
    AutoFit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridAutoFlow {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

/// One axis of an item's placement; CSS grid lines are one-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLine {
    Auto,
    Index(i16),
    Span(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPlacement {
    pub start: GridLine,
    pub end: GridLine,
}

/// Describes clipping and scrolling on one box axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssOverflow {
    /// Allows content to extend beyond the box.
    Visible,
    /// Clips content without scrolling.
    Hidden,
    /// Clips content without affecting layout.
    Clip,
    /// Enables scrolling and always displays a scrollbar.
    Scroll,
    /// Enables scrolling and displays a scrollbar only when needed.
    Auto,
}

/// Describes the positioning scheme for a layout box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// Uses normal flow positioning.
    Static,
    /// Offsets the box relative to its normal-flow position.
    Relative,
    /// Positions the box relative to its containing block.
    Absolute,
}

/// Describes the primary axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    /// Lays out flex items horizontally in source order.
    Row,
    /// Lays out flex items vertically in source order.
    Column,
    /// Lays out flex items horizontally in reverse source order.
    RowReverse,
    /// Lays out flex items vertically in reverse source order.
    ColumnReverse,
}

/// Describes whether flex items may wrap onto multiple lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    /// Keeps all flex items on one line.
    NoWrap,
    /// Allows flex items to wrap in source order.
    Wrap,
    /// Allows flex items to wrap in reverse cross-axis order.
    WrapReverse,
}

/// Describes distribution along a flex container's primary axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    /// Packs items at the start of the primary axis.
    Start,
    /// Packs items at the end of the primary axis.
    End,
    /// Centers items along the primary axis.
    Center,
    /// Distributes free space between items.
    SpaceBetween,
    /// Distributes free space around items.
    SpaceAround,
    /// Distributes equal free space around items.
    SpaceEvenly,
}

/// Describes alignment along a flex container's cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    /// Aligns items to the cross-axis start.
    Start,
    /// Aligns items to the cross-axis end.
    End,
    /// Centers items on the cross axis.
    Center,
    /// Stretches items across the cross axis.
    Stretch,
}

/// Describes an individual flex item's cross-axis alignment override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignSelf {
    /// Inherits the flex container's `align-items` behavior.
    Auto,
    /// Aligns the item to the cross-axis start.
    Start,
    /// Aligns the item to the cross-axis end.
    End,
    /// Centers the item on the cross axis.
    Center,
    /// Stretches the item across the cross axis.
    Stretch,
}
