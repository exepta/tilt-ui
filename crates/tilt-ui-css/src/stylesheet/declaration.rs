use crate::{
    AlignItems, AlignSelf, AnimatedEffect, AnimationSpec, BackgroundAttachment, BackgroundEffect,
    BackgroundPosition, BackgroundSize, BorderEdge, BorderRadius, BoxSizing, CssBackgroundImage,
    CssBoxShadow, CssColor, CssCursor, CssLineHeight, CssOverflow, CssTextShadow, CssTransform,
    Display, Edges, EffectQuality, FlexDirection, FlexWrap, FontFamily, FontWeight, GridAutoFlow,
    GridPlacement, GridTrackGroup, GridTrackSize, JustifyContent, Length, PointerEvents, Position,
    TextAlign, TextTransform, TextWrap, TransitionSpec,
};

/// Describes a stylesheet declaration that has already been parsed into a typed value.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleDeclaration {
    /// A declaration with CSS `!important` priority.
    Important(Box<StyleDeclaration>),
    /// A cascading custom property, inherited by descendants.
    CustomProperty(String, String),
    /// A value containing `var()` or a runtime math function.
    Deferred(String, String),
    /// Sets the box display mode.
    Display(Display),
    BoxSizing(BoxSizing),
    PointerEvents(PointerEvents),
    Cursor(CssCursor),
    ZIndex(i32),
    ScrollWidth(f32),
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
    FlexBasis(Length),
    Flex(f32, f32, Length),
    FlexFlow(FlexDirection, FlexWrap),
    GridTemplateRows(Vec<GridTrackGroup>),
    GridTemplateColumns(Vec<GridTrackGroup>),
    GridAutoRows(Vec<GridTrackSize>),
    GridAutoColumns(Vec<GridTrackSize>),
    GridAutoFlow(GridAutoFlow),
    GridRow(GridPlacement),
    GridColumn(GridPlacement),
    /// Sets the background color.
    BackgroundColor(CssColor),
    /// Sets a linear gradient background or clears the background image.
    BackgroundImage(Option<CssBackgroundImage>),
    BackgroundSize(BackgroundSize),
    BackgroundPosition(BackgroundPosition),
    BackgroundAttachment(BackgroundAttachment),
    BackgroundFilter(Vec<BackgroundEffect>),
    BackdropFilter(Vec<BackgroundEffect>),
    AnimatedEffects(Vec<AnimatedEffect>),
    EffectQuality(EffectQuality),
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
    BorderTopWidth(Length),
    BorderRightWidth(Length),
    BorderBottomWidth(Length),
    BorderLeftWidth(Length),
    Border(BorderEdge),
    BorderTop(BorderEdge),
    BorderRight(BorderEdge),
    BorderBottom(BorderEdge),
    BorderLeft(BorderEdge),
    /// Sets the border color.
    BorderColor(CssColor),
    BorderTopColor(CssColor),
    BorderRightColor(CssColor),
    BorderBottomColor(CssColor),
    BorderLeftColor(CssColor),
    /// Sets border corner radii.
    BorderRadius(BorderRadius),
    BoxShadow(Vec<CssBoxShadow>),
    TextShadow(Option<CssTextShadow>),
    Outline(BorderEdge),
    OutlineWidth(Length),
    OutlineColor(CssColor),
    OutlineOffset(Length),
    /// Sets the text font size.
    FontSize(Length),
    LineHeight(CssLineHeight),
    /// Selects a supported font family.
    FontFamily(FontFamily),
    /// Sets the text font weight.
    FontWeight(FontWeight),
    /// Sets horizontal text alignment.
    TextAlign(TextAlign),
    TextWrap(TextWrap),
    TextTransform(TextTransform),
}
