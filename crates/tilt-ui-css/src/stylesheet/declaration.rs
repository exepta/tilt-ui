use crate::{
    AlignItems, AlignSelf, AnimationSpec, BorderRadius, CssColor, CssGradient, CssOverflow,
    CssTransform, Display, Edges, FlexDirection, FlexWrap, FontFamily, FontWeight, JustifyContent,
    Length, Position, TextAlign, TransitionSpec,
};

/// Describes a stylesheet declaration that has already been parsed into a typed value.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleDeclaration {
    /// Sets the box display mode.
    Display(Display),
    /// Sets overflow behavior on both axes.
    Overflow(CssOverflow, CssOverflow),
    /// Sets horizontal overflow behavior.
    OverflowX(CssOverflow),
    /// Sets vertical overflow behavior.
    OverflowY(CssOverflow),
    /// Sets the box positioning scheme.
    Position(Position),
    /// Sets the box width.
    Width(Length),
    /// Sets the box height.
    Height(Length),
    /// Sets the minimum box width.
    MinWidth(Length),
    /// Sets the minimum box height.
    MinHeight(Length),
    /// Sets the maximum box width.
    MaxWidth(Length),
    /// Sets the maximum box height.
    MaxHeight(Length),
    /// Sets the top position offset.
    Top(Length),
    /// Sets the right position offset.
    Right(Length),
    /// Sets the bottom position offset.
    Bottom(Length),
    /// Sets the left position offset.
    Left(Length),
    /// Sets margins for all box edges.
    Margin(Edges<Length>),
    /// Sets the top margin.
    MarginTop(Length),
    /// Sets the right margin.
    MarginRight(Length),
    /// Sets the bottom margin.
    MarginBottom(Length),
    /// Sets the left margin.
    MarginLeft(Length),
    /// Sets padding for all box edges.
    Padding(Edges<Length>),
    /// Sets the top padding.
    PaddingTop(Length),
    /// Sets the right padding.
    PaddingRight(Length),
    /// Sets the bottom padding.
    PaddingBottom(Length),
    /// Sets the left padding.
    PaddingLeft(Length),
    /// Sets a shared gap between flex rows and columns.
    Gap(Length),
    /// Sets the gap between flex rows.
    RowGap(Length),
    /// Sets the gap between flex columns.
    ColumnGap(Length),
    /// Sets the flex primary-axis direction.
    FlexDirection(FlexDirection),
    /// Sets flex line wrapping behavior.
    FlexWrap(FlexWrap),
    /// Sets primary-axis item distribution.
    JustifyContent(JustifyContent),
    /// Sets cross-axis alignment for flex items.
    AlignItems(AlignItems),
    /// Sets an individual flex item's cross-axis alignment.
    AlignSelf(AlignSelf),
    /// Sets the flex growth factor.
    FlexGrow(f32),
    /// Sets the flex shrink factor.
    FlexShrink(f32),
    /// Sets the background color.
    BackgroundColor(CssColor),
    /// Sets a linear gradient background or clears the background image.
    BackgroundImage(Option<CssGradient>),
    /// Sets the foreground text color.
    Color(CssColor),
    /// Sets opacity as a normalized factor.
    Opacity(f32),
    /// Sets the UI transform.
    Transform(CssTransform),
    /// Sets complete animation specifications.
    Animation(Vec<AnimationSpec>),
    /// Sets animation names.
    AnimationName(Vec<Option<crate::AnimationName>>),
    /// Sets animation durations.
    AnimationDuration(Vec<crate::CssTime>),
    /// Sets animation delays.
    AnimationDelay(Vec<crate::CssTime>),
    /// Sets animation timing functions.
    AnimationTimingFunction(Vec<crate::TimingFunction>),
    /// Sets animation iteration counts.
    AnimationIterationCount(Vec<crate::IterationCount>),
    /// Sets animation directions.
    AnimationDirection(Vec<crate::AnimationDirection>),
    /// Sets complete transition specifications.
    Transition(Vec<TransitionSpec>),
    /// Sets transition property names.
    TransitionProperty(Vec<crate::TransitionProperty>),
    /// Sets transition durations.
    TransitionDuration(Vec<crate::CssTime>),
    /// Sets transition delays.
    TransitionDelay(Vec<crate::CssTime>),
    /// Sets transition timing functions.
    TransitionTimingFunction(Vec<crate::TimingFunction>),
    /// Sets border widths for all box edges.
    BorderWidth(Edges<Length>),
    /// Sets the border color.
    BorderColor(CssColor),
    /// Sets border corner radii.
    BorderRadius(BorderRadius),
    /// Sets the text font size.
    FontSize(Length),
    /// Selects a supported font family.
    FontFamily(FontFamily),
    /// Sets the text font weight.
    FontWeight(FontWeight),
    /// Sets horizontal text alignment.
    TextAlign(TextAlign),
}
