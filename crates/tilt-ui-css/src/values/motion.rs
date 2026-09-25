use crate::Length;

/// Identifies a named keyframe animation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationName(pub String);

/// Stores a CSS time value normalized to seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssTime(pub f32);

/// Defines the timing curve applied to motion progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimingFunction {
    /// Uses unmodified linear progress.
    Linear,
    /// Uses the CSS `ease` cubic Bezier curve.
    #[default]
    Ease,
    /// Uses the CSS `ease-in` cubic Bezier curve.
    EaseIn,
    /// Uses the CSS `ease-out` cubic Bezier curve.
    EaseOut,
    /// Uses the CSS `ease-in-out` cubic Bezier curve.
    EaseInOut,
}

/// Defines how an animation traverses successive iterations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationDirection {
    /// Plays each iteration from start to end.
    #[default]
    Normal,
    /// Plays each iteration from end to start.
    Reverse,
    /// Alternates direction on successive iterations.
    Alternate,
    /// Alternates direction, beginning in reverse.
    AlternateReverse,
}

/// Defines how many times an animation repeats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IterationCount {
    /// Plays a finite number of iterations.
    Finite(f32),
    /// Repeats without completion.
    Infinite,
}

impl Default for IterationCount {
    fn default() -> Self {
        Self::Finite(1.0)
    }
}

/// Describes one typed CSS animation declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationSpec {
    /// Name resolved within the owning stylesheet scope.
    pub name: Option<AnimationName>,
    /// Duration of one iteration.
    pub duration: CssTime,
    /// Positive start delay.
    pub delay: CssTime,
    /// Timing curve for keyframe segments.
    pub timing_function: TimingFunction,
    /// Iteration count.
    pub iteration_count: IterationCount,
    /// Iteration direction.
    pub direction: AnimationDirection,
}

impl Default for AnimationSpec {
    fn default() -> Self {
        Self {
            name: None,
            duration: CssTime(0.0),
            delay: CssTime(0.0),
            timing_function: TimingFunction::default(),
            iteration_count: IterationCount::default(),
            direction: AnimationDirection::default(),
        }
    }
}

/// Selects a property eligible for CSS transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionProperty {
    /// Disables transition of the selected property slot.
    None,
    /// Matches every currently interpolatable property.
    All,
    /// Matches foreground text color.
    Color,
    /// Matches background color.
    BackgroundColor,
    /// Matches border color.
    BorderColor,
    /// Matches border radius.
    BorderRadius,
    /// Matches text font size.
    FontSize,
    /// Matches element opacity in the typed style layer.
    Opacity,
    /// Matches UI transforms.
    Transform,
    /// Matches width.
    Width,
    /// Matches height.
    Height,
}

/// Describes one typed CSS transition declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionSpec {
    /// Property selected for interpolation.
    pub property: TransitionProperty,
    /// Transition duration.
    pub duration: CssTime,
    /// Positive start delay.
    pub delay: CssTime,
    /// Timing curve.
    pub timing_function: TimingFunction,
}

/// Stores a typed two-dimensional CSS transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssTransform {
    /// Horizontal translation.
    pub translate_x: Length,
    /// Vertical translation.
    pub translate_y: Length,
    /// Horizontal scale.
    pub scale_x: f32,
    /// Vertical scale.
    pub scale_y: f32,
    /// Clockwise rotation in radians.
    pub rotation: f32,
}

impl Default for CssTransform {
    fn default() -> Self {
        Self {
            translate_x: Length::Px(0.0),
            translate_y: Length::Px(0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        }
    }
}
