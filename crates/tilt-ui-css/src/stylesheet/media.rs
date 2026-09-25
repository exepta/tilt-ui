/// Describes the viewport available while evaluating responsive CSS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaEnvironment {
    /// Logical viewport width in CSS pixels.
    pub width: f32,
    /// Logical viewport height in CSS pixels.
    pub height: f32,
}

/// Selects the graphical media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// Matches every TiltUI viewport.
    All,
    /// Matches a graphical TiltUI viewport.
    Screen,
    /// A parsed but unsupported media type.
    Unsupported,
}

/// Compares one viewport dimension against a typed CSS length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaComparison {
    /// Requires equal values.
    Equal(crate::Length),
    /// Requires a value greater than or equal to the limit.
    GreaterOrEqual(crate::Length),
    /// Requires a value greater than the limit.
    GreaterThan(crate::Length),
    /// Requires a value less than or equal to the limit.
    LessOrEqual(crate::Length),
    /// Requires a value less than the limit.
    LessThan(crate::Length),
}

/// Defines a viewport orientation test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaOrientation {
    /// Matches when height exceeds width.
    Portrait,
    /// Matches when width is at least height.
    Landscape,
}

/// Describes a parsed responsive condition evaluated against the active UI viewport.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaCondition {
    /// Always matches.
    True,
    /// Never matches.
    False,
    /// Compares viewport width.
    Width(MediaComparison),
    /// Compares viewport height.
    Height(MediaComparison),
    /// Tests viewport orientation.
    Orientation(MediaOrientation),
    /// Requires every child condition to match.
    And(Vec<MediaCondition>),
    /// Requires at least one child condition to match.
    Or(Vec<MediaCondition>),
    /// Negates a child condition.
    Not(Box<MediaCondition>),
}

impl MediaCondition {
    /// Evaluates this typed condition against the supplied viewport.
    pub fn matches(&self, environment: MediaEnvironment) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Width(comparison) => compare(environment.width, *comparison, environment),
            Self::Height(comparison) => compare(environment.height, *comparison, environment),
            Self::Orientation(MediaOrientation::Portrait) => environment.height > environment.width,
            Self::Orientation(MediaOrientation::Landscape) => {
                environment.width >= environment.height
            }
            Self::And(conditions) => conditions
                .iter()
                .all(|condition| condition.matches(environment)),
            Self::Or(conditions) => conditions
                .iter()
                .any(|condition| condition.matches(environment)),
            Self::Not(condition) => !condition.matches(environment),
        }
    }
}

fn compare(value: f32, comparison: MediaComparison, environment: MediaEnvironment) -> bool {
    let limit = match comparison {
        MediaComparison::Equal(length)
        | MediaComparison::GreaterOrEqual(length)
        | MediaComparison::GreaterThan(length)
        | MediaComparison::LessOrEqual(length)
        | MediaComparison::LessThan(length) => media_length(length, environment),
    };
    let Some(limit) = limit else { return false };
    match comparison {
        MediaComparison::Equal(_) => (value - limit).abs() <= f32::EPSILON,
        MediaComparison::GreaterOrEqual(_) => value >= limit,
        MediaComparison::GreaterThan(_) => value > limit,
        MediaComparison::LessOrEqual(_) => value <= limit,
        MediaComparison::LessThan(_) => value < limit,
    }
}

fn media_length(length: crate::Length, environment: MediaEnvironment) -> Option<f32> {
    match length {
        crate::Length::Px(value) => Some(value),
        crate::Length::Vw(value) => Some(environment.width * value / 100.0),
        crate::Length::Vh(value) => Some(environment.height * value / 100.0),
        crate::Length::Auto | crate::Length::Percent(_) => None,
    }
}

/// Stores one alternative in a comma-separated media query list.
#[derive(Debug, Clone)]
pub struct MediaQuery {
    /// Parsed media type for this alternative.
    pub media_type: MediaType,
    /// Parsed boolean condition for this alternative.
    pub condition: MediaCondition,
}

impl MediaQuery {
    /// Evaluates this alternative against a viewport.
    pub fn matches(&self, environment: MediaEnvironment) -> bool {
        matches!(self.media_type, MediaType::All | MediaType::Screen)
            && self.condition.matches(environment)
    }
}

/// Stores a typed media block and its source-ordered nested rules.
#[derive(Debug, Clone)]
pub struct MediaRule {
    /// Comma-separated alternatives evaluated as logical OR.
    pub queries: Vec<MediaQuery>,
    /// Nested rules retaining global stylesheet source order.
    pub rules: Vec<super::StyleRule>,
}

impl MediaRule {
    /// Evaluates the media type and condition against a viewport.
    pub fn matches(&self, environment: MediaEnvironment) -> bool {
        self.queries.iter().any(|query| query.matches(environment))
    }
}
