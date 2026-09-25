// ============================================================================
// Form
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormValidationMode {
    Always,

    #[default]
    Send,

    Interact,
}

// ============================================================================
// Button
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonType {
    #[default]
    Auto,

    Button,
    Submit,
    Reset,
}

// ============================================================================
// Divider
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DividerAlignment {
    #[default]
    Vertical,

    Horizontal,
}

// ============================================================================
// Field Set
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    Radio,
    Toggle,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldMode {
    Multi,

    #[default]
    Single,

    Count(u8),
}

// ============================================================================
// Headline
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadlineType {
    #[default]
    H1,

    H2,
    H3,
    H4,
    H5,
    H6,
}

// ============================================================================
// Date
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateFormat {
    #[default]
    MonthDayYear,

    DayMonthYear,
    YearMonthDay,
}

// ============================================================================
// Input
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputType {
    #[default]
    Text,

    Email,
    Date,
    Range,
    Password,
    Number,
    File,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputCap {
    #[default]
    NoCap,

    CapAtNodeSize,
    CapAt(usize),
}

// ============================================================================
// Badge
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BadgeAnchor {
    TopLeft,

    #[default]
    TopRight,

    BottomLeft,
    BottomRight,
}

// ============================================================================
// Tooltip
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTipVariant {
    #[default]
    Follow,

    Point,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTipPriority {
    Top,
    Bottom,
    Left,

    #[default]
    Right,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTipAlignment {
    Vertical,

    #[default]
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTipTrigger {
    Hover,
    Click,
    Drag,
}

// ============================================================================
// Slider
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliderType {
    #[default]
    Default,

    Range,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SliderDotAnchor {
    #[default]
    Top,

    Bottom,
}

// ============================================================================
// HyperLink
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum HyperLinkBrowsers {
    #[default]
    System,

    Custom(Vec<String>),
}

// ============================================================================
// Validation
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ValidationRules {
    pub required: bool,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
}
