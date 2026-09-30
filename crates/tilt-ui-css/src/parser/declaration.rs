use cssparser::{Parser, Token};

use crate::{
    AlignItems, AlignSelf, AnimatedEffect, AnimatedEffectKind, AnimationDirection, AnimationName,
    AnimationSpec, BackgroundAttachment, BackgroundEffect, BackgroundPosition, BackgroundSize,
    BorderEdge, BorderStyle, BoxSizing, CssBackgroundImage, CssBoxShadow, CssCursor, CssGradient,
    CssLineHeight, CssOverflow, CssTextShadow, CssTime, CssTransform, Display, EffectQuality,
    FlexDirection, FlexWrap, FontFamily, FontWeight, GradientDirection, IterationCount,
    JustifyContent, PointerEvents, Position, StyleDeclaration, TextAlign, TextTransform, TextWrap,
    TimingFunction, TransitionProperty, TransitionSpec,
};

use super::{
    StyleParseError,
    value::{parse_border_radius, parse_color, parse_edges, parse_length},
};

pub(crate) fn parse_declaration(
    property: &str,
    input: &mut Parser<'_, '_>,
) -> Result<StyleDeclaration, StyleParseError> {
    let declaration = match property {
        "display" => StyleDeclaration::Display(parse_display(input)?),
        "box-sizing" => StyleDeclaration::BoxSizing(
            match input
                .expect_ident()
                .map_err(|_| StyleParseError::InvalidPropertyValue {
                    property: property.into(),
                    value: "expected box sizing".into(),
                })?
                .as_ref()
            {
                "content-box" => BoxSizing::ContentBox,
                "border-box" => BoxSizing::BorderBox,
                _ => {
                    return Err(StyleParseError::InvalidPropertyValue {
                        property: property.into(),
                        value: "expected content-box or border-box".into(),
                    });
                }
            },
        ),
        "pointer-events" => {
            StyleDeclaration::PointerEvents(match parse_ident(input, property)?.as_str() {
                "auto" => PointerEvents::Auto,
                "none" => PointerEvents::None,
                value => return invalid_keyword(property, value),
            })
        }
        "cursor" => StyleDeclaration::Cursor(match parse_ident(input, property)?.as_str() {
            "auto" => CssCursor::Auto,
            "default" => CssCursor::Default,
            "pointer" => CssCursor::Pointer,
            "text" => CssCursor::Text,
            "move" => CssCursor::Move,
            "wait" => CssCursor::Wait,
            "progress" => CssCursor::Progress,
            "crosshair" => CssCursor::Crosshair,
            "help" => CssCursor::Help,
            "grab" => CssCursor::Grab,
            "grabbing" => CssCursor::Grabbing,
            "not-allowed" => CssCursor::NotAllowed,
            "col-resize" => CssCursor::ColResize,
            "row-resize" => CssCursor::RowResize,
            "nwse-resize" => CssCursor::NwseResize,
            value => return invalid_keyword(property, value),
        }),
        "z-index" => StyleDeclaration::ZIndex(parse_z_index(input)?),
        "scroll-width" => StyleDeclaration::ScrollWidth(match parse_length(input, false)? {
            crate::Length::Px(value) if value >= 0.0 => value,
            _ => {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: property.into(),
                    value: "expected a non-negative px length".into(),
                });
            }
        }),
        "overflow" => {
            let x = parse_overflow_value(input)?;
            let y = input.try_parse(parse_overflow_value).unwrap_or(x);
            StyleDeclaration::Overflow(x, y)
        }
        "overflow-x" => StyleDeclaration::OverflowX(parse_overflow_value(input)?),
        "overflow-y" => StyleDeclaration::OverflowY(parse_overflow_value(input)?),
        "position" => StyleDeclaration::Position(parse_position(input)?),
        "width" => StyleDeclaration::Width(parse_length(input, true)?),
        "height" => StyleDeclaration::Height(parse_length(input, true)?),
        "min-width" => StyleDeclaration::MinWidth(parse_length(input, true)?),
        "min-height" => StyleDeclaration::MinHeight(parse_length(input, true)?),
        "max-width" => StyleDeclaration::MaxWidth(parse_length(input, true)?),
        "max-height" => StyleDeclaration::MaxHeight(parse_length(input, true)?),
        "top" => StyleDeclaration::Top(parse_length(input, true)?),
        "right" => StyleDeclaration::Right(parse_length(input, true)?),
        "bottom" => StyleDeclaration::Bottom(parse_length(input, true)?),
        "left" => StyleDeclaration::Left(parse_length(input, true)?),
        "margin" => StyleDeclaration::Margin(parse_edges(input, true)?),
        "margin-top" => StyleDeclaration::MarginTop(parse_length(input, true)?),
        "margin-right" => StyleDeclaration::MarginRight(parse_length(input, true)?),
        "margin-bottom" => StyleDeclaration::MarginBottom(parse_length(input, true)?),
        "margin-left" => StyleDeclaration::MarginLeft(parse_length(input, true)?),
        "padding" => StyleDeclaration::Padding(parse_edges(input, false)?),
        "padding-top" => StyleDeclaration::PaddingTop(parse_length(input, false)?),
        "padding-right" => StyleDeclaration::PaddingRight(parse_length(input, false)?),
        "padding-bottom" => StyleDeclaration::PaddingBottom(parse_length(input, false)?),
        "padding-left" => StyleDeclaration::PaddingLeft(parse_length(input, false)?),
        "gap" => StyleDeclaration::Gap(parse_length(input, false)?),
        "row-gap" => StyleDeclaration::RowGap(parse_length(input, false)?),
        "column-gap" => StyleDeclaration::ColumnGap(parse_length(input, false)?),
        "flex-direction" => StyleDeclaration::FlexDirection(parse_flex_direction(input)?),
        "flex-wrap" => StyleDeclaration::FlexWrap(parse_flex_wrap(input)?),
        "justify-content" => StyleDeclaration::JustifyContent(parse_justify_content(input)?),
        "align-items" => StyleDeclaration::AlignItems(parse_align_items(input)?),
        "align-self" => StyleDeclaration::AlignSelf(parse_align_self(input)?),
        "flex-grow" => StyleDeclaration::FlexGrow(parse_non_negative_number(input, property)?),
        "flex-shrink" => StyleDeclaration::FlexShrink(parse_non_negative_number(input, property)?),
        "flex-basis" => StyleDeclaration::FlexBasis(parse_length(input, true)?),
        "flex" => {
            let (grow, shrink, basis) = parse_flex(input)?;
            StyleDeclaration::Flex(grow, shrink, basis)
        }
        "flex-flow" => {
            let (direction, wrap) = parse_flex_flow(input)?;
            StyleDeclaration::FlexFlow(direction, wrap)
        }
        "background-color" => StyleDeclaration::BackgroundColor(parse_color(input)?),
        "background-image" => StyleDeclaration::BackgroundImage(parse_background_image(input)?),
        "background-size" => StyleDeclaration::BackgroundSize(parse_background_size(input)?),
        "background-position" => {
            StyleDeclaration::BackgroundPosition(parse_background_position(input)?)
        }
        "background-attachment" => {
            StyleDeclaration::BackgroundAttachment(parse_background_attachment(input)?)
        }
        "background-filter" => {
            StyleDeclaration::BackgroundFilter(parse_effect_filter(input, "background-filter")?)
        }
        "backdrop-filter" => {
            let effects = parse_effect_filter(input, "backdrop-filter")?;
            if effects
                .iter()
                .any(|effect| matches!(effect, BackgroundEffect::OilPaint(_)))
            {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "backdrop-filter".into(),
                    value: "oil-paint is only supported for cached background images".into(),
                });
            }
            StyleDeclaration::BackdropFilter(effects)
        }
        "animated-filter" => StyleDeclaration::AnimatedEffects(parse_animated_effects(input)?),
        "effect-quality" => {
            StyleDeclaration::EffectQuality(match parse_ident(input, property)?.as_str() {
                "auto" => EffectQuality::Auto,
                "low" => EffectQuality::Low,
                "medium" => EffectQuality::Medium,
                "high" => EffectQuality::High,
                value => return invalid_keyword(property, value),
            })
        }
        "color" => StyleDeclaration::Color(parse_color(input)?),
        "opacity" => StyleDeclaration::Opacity(parse_opacity(input)?),
        "transform" => StyleDeclaration::Transform(parse_transform(input)?),
        "animation" => StyleDeclaration::Animation(parse_animation_list(input)?),
        "animation-name" => StyleDeclaration::AnimationName(parse_animation_names(input)?),
        "animation-duration" => {
            StyleDeclaration::AnimationDuration(parse_time_list(input, property)?)
        }
        "animation-delay" => StyleDeclaration::AnimationDelay(parse_time_list(input, property)?),
        "animation-timing-function" => {
            StyleDeclaration::AnimationTimingFunction(parse_timing_list(input)?)
        }
        "animation-iteration-count" => {
            StyleDeclaration::AnimationIterationCount(parse_iterations(input)?)
        }
        "animation-direction" => StyleDeclaration::AnimationDirection(parse_directions(input)?),
        "transition" => StyleDeclaration::Transition(parse_transition_list(input)?),
        "transition-property" => {
            StyleDeclaration::TransitionProperty(parse_transition_properties(input)?)
        }
        "transition-duration" => {
            StyleDeclaration::TransitionDuration(parse_time_list(input, property)?)
        }
        "transition-delay" => StyleDeclaration::TransitionDelay(parse_time_list(input, property)?),
        "transition-timing-function" => {
            StyleDeclaration::TransitionTimingFunction(parse_timing_list(input)?)
        }
        "border-width" => StyleDeclaration::BorderWidth(parse_edges(input, false)?),
        "border-top-width" => StyleDeclaration::BorderTopWidth(parse_length(input, false)?),
        "border-right-width" => StyleDeclaration::BorderRightWidth(parse_length(input, false)?),
        "border-bottom-width" => StyleDeclaration::BorderBottomWidth(parse_length(input, false)?),
        "border-left-width" => StyleDeclaration::BorderLeftWidth(parse_length(input, false)?),
        "border-style" => StyleDeclaration::BorderStyle(parse_border_styles(input, property)?),
        "border-top-style" => {
            StyleDeclaration::BorderTopStyle(parse_border_style(input, property)?)
        }
        "border-right-style" => {
            StyleDeclaration::BorderRightStyle(parse_border_style(input, property)?)
        }
        "border-bottom-style" => {
            StyleDeclaration::BorderBottomStyle(parse_border_style(input, property)?)
        }
        "border-left-style" => {
            StyleDeclaration::BorderLeftStyle(parse_border_style(input, property)?)
        }
        "border-brush-strength" => {
            StyleDeclaration::BorderBrushStrength(parse_brush_strength(input, property)?)
        }
        "border" => StyleDeclaration::Border(parse_border(input, property)?),
        "border-top" => StyleDeclaration::BorderTop(parse_border(input, property)?),
        "border-right" => StyleDeclaration::BorderRight(parse_border(input, property)?),
        "border-bottom" => StyleDeclaration::BorderBottom(parse_border(input, property)?),
        "border-left" => StyleDeclaration::BorderLeft(parse_border(input, property)?),
        "border-color" => {
            let first = parse_color(input)?;
            let second = input.try_parse(parse_color).ok();
            let third = input.try_parse(parse_color).ok();
            let fourth = input.try_parse(parse_color).ok();
            match (second, third, fourth) {
                (None, _, _) => StyleDeclaration::BorderColor(first),
                (Some(right), None, _) => StyleDeclaration::BorderColors(crate::Edges {
                    top: first,
                    right,
                    bottom: first,
                    left: right,
                }),
                (Some(right), Some(bottom), None) => StyleDeclaration::BorderColors(crate::Edges {
                    top: first,
                    right,
                    bottom,
                    left: right,
                }),
                (Some(right), Some(bottom), Some(left)) => {
                    StyleDeclaration::BorderColors(crate::Edges {
                        top: first,
                        right,
                        bottom,
                        left,
                    })
                }
            }
        }
        "border-top-color" => StyleDeclaration::BorderTopColor(parse_color(input)?),
        "border-right-color" => StyleDeclaration::BorderRightColor(parse_color(input)?),
        "border-bottom-color" => StyleDeclaration::BorderBottomColor(parse_color(input)?),
        "border-left-color" => StyleDeclaration::BorderLeftColor(parse_color(input)?),
        "border-radius" => StyleDeclaration::BorderRadius(parse_border_radius(input)?),
        "box-shadow" => StyleDeclaration::BoxShadow(parse_box_shadows(input)?),
        "text-shadow" => StyleDeclaration::TextShadow(parse_text_shadow(input)?),
        "outline" => StyleDeclaration::Outline(parse_border(input, property)?),
        "outline-width" => StyleDeclaration::OutlineWidth(parse_length(input, false)?),
        "outline-color" => StyleDeclaration::OutlineColor(parse_color(input)?),
        "outline-offset" => StyleDeclaration::OutlineOffset(parse_length(input, false)?),
        "font-size" => StyleDeclaration::FontSize(parse_length(input, false)?),
        "line-height" => StyleDeclaration::LineHeight(parse_line_height(input)?),
        "font-family" => StyleDeclaration::FontFamily(parse_font_family(input)?),
        "font-weight" => StyleDeclaration::FontWeight(parse_font_weight(input)?),
        "text-align" => StyleDeclaration::TextAlign(parse_text_align(input)?),
        "text-wrap" => StyleDeclaration::TextWrap(match parse_ident(input, property)?.as_str() {
            "wrap" => TextWrap::Wrap,
            "nowrap" => TextWrap::NoWrap,
            value => return invalid_keyword(property, value),
        }),
        "text-transform" => {
            StyleDeclaration::TextTransform(match parse_ident(input, property)?.as_str() {
                "none" => TextTransform::None,
                "uppercase" => TextTransform::Uppercase,
                "lowercase" => TextTransform::Lowercase,
                "capitalize" => TextTransform::Capitalize,
                value => return invalid_keyword(property, value),
            })
        }
        _ => return Err(StyleParseError::UnsupportedProperty(property.to_owned())),
    };

    input
        .expect_exhausted()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "unexpected trailing tokens".into(),
        })?;
    Ok(declaration)
}

/// Parses an authored declaration after removing its optional priority suffix.
pub(crate) fn parse_authored_declaration(
    property: &str,
    source: &str,
) -> Result<StyleDeclaration, StyleParseError> {
    let source = source.trim();
    let (value, important) = strip_important(source);
    let declaration = if property.starts_with("--") {
        if property.len() <= 2 || value.trim().is_empty() {
            return Err(StyleParseError::InvalidDeclaration(property.to_owned()));
        }
        StyleDeclaration::CustomProperty(property.to_owned(), value.trim().to_owned())
    } else if has_deferred_function(value) {
        if matches!(
            parse_declaration_value(property, "__invalid_probe__"),
            Err(StyleParseError::UnsupportedProperty(_))
        ) {
            return Err(StyleParseError::UnsupportedProperty(property.to_owned()));
        }
        StyleDeclaration::Deferred(property.to_owned(), value.trim().to_owned())
    } else {
        parse_declaration_value(property, value)?
    };
    Ok(if important {
        StyleDeclaration::Important(Box::new(declaration))
    } else {
        declaration
    })
}

/// Parses a resolved property value into the existing typed declaration model.
pub fn parse_declaration_value(
    property: &str,
    source: &str,
) -> Result<StyleDeclaration, StyleParseError> {
    if let Some(declaration) = super::grid::parse_grid_declaration(property, source)? {
        return Ok(declaration);
    }
    let mut input = cssparser::ParserInput::new(source);
    parse_declaration(property, &mut cssparser::Parser::new(&mut input))
}

fn strip_important(source: &str) -> (&str, bool) {
    let trimmed = trim_css_end(source);
    let word_start = trimmed
        .trim_end_matches(|ch: char| ch.is_ascii_alphabetic())
        .len();
    let word = &trimmed[word_start..];
    if !word.eq_ignore_ascii_case("important") {
        return (source, false);
    }
    let before_word = trim_css_end(&trimmed[..word_start]);
    if let Some(value) = before_word.strip_suffix('!') {
        (value.trim_end(), true)
    } else {
        (source, false)
    }
}

fn trim_css_end(mut source: &str) -> &str {
    loop {
        source = source.trim_end();
        if source.ends_with("*/")
            && let Some(start) = source.rfind("/*")
        {
            source = &source[..start];
        } else {
            return source;
        }
    }
}

fn has_deferred_function(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    ["var(", "calc(", "min(", "max(", "sin("]
        .iter()
        .any(|name| {
            lower.match_indices(name).any(|(index, _)| {
                !lower[..index]
                    .chars()
                    .last()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
            })
        })
}

fn parse_border(input: &mut Parser<'_, '_>, property: &str) -> Result<BorderEdge, StyleParseError> {
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: property.to_owned(),
        value: "expected a border width, style and color, or none".into(),
    };
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(BorderEdge {
            width: crate::Length::Px(0.0),
            color: crate::CssColor::transparent(),
            style: BorderStyle::None,
        });
    }
    if input
        .try_parse(|input| {
            let value = input.expect_number()?;
            if value == 0.0 {
                input.expect_exhausted()?;
                Ok(())
            } else {
                Err(input.new_custom_error::<_, ()>(()))
            }
        })
        .is_ok()
    {
        return Ok(BorderEdge {
            width: crate::Length::Px(0.0),
            color: crate::CssColor::transparent(),
            style: BorderStyle::None,
        });
    }
    let mut width = None;
    let mut color = None;
    let mut style = None;
    while !input.is_exhausted() {
        if style.is_none()
            && let Ok(value) = input.try_parse(|input| parse_border_style(input, property))
        {
            style = Some(value);
        } else if width.is_none() {
            if let Ok(value) = input.try_parse(|input| parse_length(input, false)) {
                width = Some(value);
                continue;
            }
            if color.is_none() {
                color = Some(parse_color(input)?);
            } else {
                return Err(invalid());
            }
        } else if color.is_none() {
            color = Some(parse_color(input)?);
        } else {
            return Err(invalid());
        }
    }
    if style.is_none() {
        return Err(invalid());
    }
    Ok(BorderEdge {
        width: width.ok_or_else(invalid)?,
        color: color.ok_or_else(invalid)?,
        style: style.unwrap(),
    })
}

fn parse_border_style(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<BorderStyle, StyleParseError> {
    let value = parse_ident(input, property)?;
    match value.as_str() {
        "none" => Ok(BorderStyle::None),
        "solid" | "line" => Ok(BorderStyle::Solid),
        "dotted" => Ok(BorderStyle::Dotted),
        "dashed" => Ok(BorderStyle::Dashed),
        "dash-dot" | "dot-and-lines" | "dot-and-line" => Ok(BorderStyle::DashDot),
        "skeleton" | "skelleton" => Ok(BorderStyle::Skeleton),
        "brushed" => Ok(BorderStyle::Brushed),
        _ => Err(StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: format!("unknown border style `{value}`"),
        }),
    }
}

fn parse_border_styles(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<crate::Edges<BorderStyle>, StyleParseError> {
    let first = parse_border_style(input, property)?;
    let second = input
        .try_parse(|input| parse_border_style(input, property))
        .ok();
    let third = input
        .try_parse(|input| parse_border_style(input, property))
        .ok();
    let fourth = input
        .try_parse(|input| parse_border_style(input, property))
        .ok();
    Ok(match (second, third, fourth) {
        (None, _, _) => crate::Edges::all(first),
        (Some(horizontal), None, _) => crate::Edges {
            top: first,
            right: horizontal,
            bottom: first,
            left: horizontal,
        },
        (Some(right), Some(bottom), None) => crate::Edges {
            top: first,
            right,
            bottom,
            left: right,
        },
        (Some(right), Some(bottom), Some(left)) => crate::Edges {
            top: first,
            right,
            bottom,
            left,
        },
    })
}

fn parse_brush_strength(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<f32, StyleParseError> {
    let value = input
        .next()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "expected a number from 0 to 1 or a percentage".into(),
        })?
        .clone();
    let strength = match value {
        Token::Number { value, .. } => value,
        Token::Percentage { unit_value, .. } => unit_value,
        _ => {
            return Err(StyleParseError::InvalidPropertyValue {
                property: property.to_owned(),
                value: "expected a number from 0 to 1 or a percentage".into(),
            });
        }
    };
    if !(0.0..=1.0).contains(&strength) {
        return Err(StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "brush strength must be between 0 and 1".into(),
        });
    }
    Ok(strength)
}

fn parse_box_shadows(input: &mut Parser<'_, '_>) -> Result<Vec<CssBoxShadow>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(Vec::new());
    }
    let mut shadows = Vec::new();
    loop {
        let mut lengths = Vec::new();
        let mut color = None;
        loop {
            if input.is_exhausted() || input.try_parse(|input| input.expect_comma()).is_ok() {
                break;
            }
            if let Ok(length) = input.try_parse(|input| parse_length(input, false)) {
                lengths.push(length);
                if lengths.len() > 4 {
                    return Err(StyleParseError::InvalidPropertyValue {
                        property: "box-shadow".into(),
                        value: "expected 2-4 lengths".into(),
                    });
                }
            } else if color.is_none() {
                color = Some(parse_color(input)?);
            } else {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "box-shadow".into(),
                    value: "duplicate color or unsupported inset shadow".into(),
                });
            }
        }
        if !(2..=4).contains(&lengths.len()) {
            return Err(StyleParseError::InvalidPropertyValue {
                property: "box-shadow".into(),
                value: "expected x and y offsets".into(),
            });
        }
        let blur = lengths.get(2).copied().unwrap_or(crate::Length::Px(0.0));
        if !non_negative_length(blur) {
            return Err(StyleParseError::InvalidPropertyValue {
                property: "box-shadow".into(),
                value: "blur radius must be non-negative".into(),
            });
        }
        shadows.push(CssBoxShadow {
            x: lengths[0],
            y: lengths[1],
            blur,
            spread: lengths.get(3).copied().unwrap_or(crate::Length::Px(0.0)),
            color: color.unwrap_or(crate::CssColor::rgba(0.0, 0.0, 0.0, 1.0)),
        });
        if input.is_exhausted() {
            break;
        }
    }
    Ok(shadows)
}

fn parse_text_shadow(input: &mut Parser<'_, '_>) -> Result<Option<CssTextShadow>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(None);
    }
    let mut lengths = Vec::new();
    let mut color = None;
    while !input.is_exhausted() {
        if let Ok(length) = input.try_parse(|input| parse_length(input, false)) {
            lengths.push(length);
        } else if color.is_none() {
            color = Some(parse_color(input)?);
        } else {
            return Err(StyleParseError::InvalidPropertyValue {
                property: "text-shadow".into(),
                value: "expected two offsets and an optional color".into(),
            });
        }
    }
    let [x, y] = lengths.as_slice() else {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "text-shadow".into(),
            value: "expected two offsets".into(),
        });
    };
    if !matches!(x, crate::Length::Px(_)) || !matches!(y, crate::Length::Px(_)) {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "text-shadow".into(),
            value: "text offsets require px".into(),
        });
    }
    Ok(Some(CssTextShadow {
        x: *x,
        y: *y,
        color: color.unwrap_or(crate::CssColor::rgba(0.0, 0.0, 0.0, 1.0)),
    }))
}

fn non_negative_length(length: crate::Length) -> bool {
    match length {
        crate::Length::Px(value)
        | crate::Length::Percent(value)
        | crate::Length::Vw(value)
        | crate::Length::Vh(value) => value >= 0.0,
        crate::Length::Auto => false,
    }
}

fn parse_background_image(
    input: &mut Parser<'_, '_>,
) -> Result<Option<CssBackgroundImage>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(None);
    }
    let token = input
        .next()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "background-image".into(),
            value: "expected linear-gradient()".into(),
        })?
        .clone();
    match &token {
        Token::UnquotedUrl(value) if !value.trim().is_empty() => {
            return Ok(Some(CssBackgroundImage::Url(value.to_string())));
        }
        Token::Function(name) if name.eq_ignore_ascii_case("url") => {
            let source = input
                .parse_nested_block(|input| {
                    let source = input.expect_string()?.to_string();
                    input.expect_exhausted()?;
                    Ok::<_, cssparser::ParseError<'_, StyleParseError>>(source)
                })
                .map_err(|_| StyleParseError::InvalidPropertyValue {
                    property: "background-image".into(),
                    value: "invalid url()".into(),
                })?;
            if source.trim().is_empty() {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "background-image".into(),
                    value: "empty url()".into(),
                });
            }
            return Ok(Some(CssBackgroundImage::Url(source)));
        }
        _ => {}
    }
    if !matches!(token, Token::Function(name) if name.eq_ignore_ascii_case("linear-gradient")) {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "background-image".into(),
            value: "expected linear-gradient()".into(),
        });
    }
    input
        .parse_nested_block(|input| {
            let direction = input
                .try_parse(|input| {
                    let token = input.next()?.clone();
                    match token {
                        Token::Ident(value) if value.eq_ignore_ascii_case("to") => {
                            let side = input.expect_ident()?.to_ascii_lowercase();
                            match side.as_str() {
                                "top" => Ok(GradientDirection::Up),
                                "right" => Ok(GradientDirection::Right),
                                "bottom" => Ok(GradientDirection::Down),
                                "left" => Ok(GradientDirection::Left),
                                _ => Err(input.new_custom_error::<_, StyleParseError>(
                                    StyleParseError::InvalidPropertyValue {
                                        property: "background-image".into(),
                                        value: "unsupported gradient direction".into(),
                                    },
                                )),
                            }
                        }
                        Token::Dimension { value, unit, .. }
                            if unit.eq_ignore_ascii_case("deg") =>
                        {
                            let angle = value.rem_euclid(360.0);
                            if (angle - 0.0).abs() < f32::EPSILON {
                                Ok(GradientDirection::Up)
                            } else if (angle - 90.0).abs() < f32::EPSILON {
                                Ok(GradientDirection::Right)
                            } else if (angle - 180.0).abs() < f32::EPSILON {
                                Ok(GradientDirection::Down)
                            } else if (angle - 270.0).abs() < f32::EPSILON {
                                Ok(GradientDirection::Left)
                            } else {
                                Err(input.new_custom_error::<_, StyleParseError>(
                                    StyleParseError::InvalidPropertyValue {
                                        property: "background-image".into(),
                                        value: "only cardinal angles are supported".into(),
                                    },
                                ))
                            }
                        }
                        _ => Err(input.new_custom_error::<_, StyleParseError>(
                            StyleParseError::InvalidPropertyValue {
                                property: "background-image".into(),
                                value: "invalid gradient direction".into(),
                            },
                        )),
                    }
                })
                .ok();
            if direction.is_some() {
                input.expect_comma()?;
            }
            let stops = input.parse_comma_separated(|input| {
                parse_color(input)
                    .map_err(|error| input.new_custom_error::<_, StyleParseError>(error))
            })?;
            if stops.len() < 2 {
                return Err(input.new_custom_error::<_, StyleParseError>(
                    StyleParseError::InvalidPropertyValue {
                        property: "background-image".into(),
                        value: "at least two colors are required".into(),
                    },
                ));
            }
            Ok(Some(CssBackgroundImage::LinearGradient(CssGradient {
                direction: direction.unwrap_or(GradientDirection::Down),
                stops,
            })))
        })
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "background-image".into(),
            value: "invalid linear-gradient()".into(),
        })
}

fn parse_background_size(input: &mut Parser<'_, '_>) -> Result<BackgroundSize, StyleParseError> {
    match parse_ident(input, "background-size")?.as_str() {
        "cover" => Ok(BackgroundSize::Cover),
        "contain" => Ok(BackgroundSize::Contain),
        "stretch" => Ok(BackgroundSize::Stretch),
        value => invalid_keyword("background-size", value),
    }
}

fn parse_background_position(
    input: &mut Parser<'_, '_>,
) -> Result<BackgroundPosition, StyleParseError> {
    let mut position = BackgroundPosition::default();
    let mut axes = [false, false];
    while !input.is_exhausted() {
        let token = input
            .next()
            .map_err(|_| StyleParseError::InvalidPropertyValue {
                property: "background-position".into(),
                value: "expected keywords or percentages".into(),
            })?
            .clone();
        let (axis, value) = match token {
            Token::Ident(value) if value.eq_ignore_ascii_case("left") => (0, 0.0),
            Token::Ident(value) if value.eq_ignore_ascii_case("right") => (0, 1.0),
            Token::Ident(value) if value.eq_ignore_ascii_case("top") => (1, 0.0),
            Token::Ident(value) if value.eq_ignore_ascii_case("bottom") => (1, 1.0),
            Token::Ident(value) if value.eq_ignore_ascii_case("center") => {
                (if axes[0] { 1 } else { 0 }, 0.5)
            }
            Token::Percentage { unit_value, .. } if (0.0..=1.0).contains(&unit_value) => {
                (if axes[0] { 1 } else { 0 }, unit_value)
            }
            _ => {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "background-position".into(),
                    value: "expected left, center, right, top, bottom or a percentage".into(),
                });
            }
        };
        if axes[axis] {
            return Err(StyleParseError::InvalidPropertyValue {
                property: "background-position".into(),
                value: "duplicate position axis".into(),
            });
        }
        axes[axis] = true;
        if axis == 0 {
            position.x = value;
        } else {
            position.y = value;
        }
    }
    if !axes[0] && !axes[1] {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "background-position".into(),
            value: "expected a position".into(),
        });
    }
    Ok(position)
}

fn parse_background_attachment(
    input: &mut Parser<'_, '_>,
) -> Result<BackgroundAttachment, StyleParseError> {
    match parse_ident(input, "background-attachment")?.as_str() {
        "scroll" => Ok(BackgroundAttachment::Scroll),
        "fixed" => Ok(BackgroundAttachment::Fixed),
        value => invalid_keyword("background-attachment", value),
    }
}

fn parse_effect_filter(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<Vec<BackgroundEffect>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(Vec::new());
    }
    let invalid = || {
        StyleParseError::InvalidPropertyValue {
        property: property.into(),
        value: "expected blur(px), grayscale(amount), oil-paint(radius), contrast(amount) or invert(amount)".into(),
    }
    };
    let mut effects = Vec::new();
    while !input.is_exhausted() {
        let Token::Function(name) = input.next().map_err(|_| invalid())?.clone() else {
            return Err(invalid());
        };
        let effect = input
            .parse_nested_block(|input| {
                let effect = match name.to_ascii_lowercase().as_str() {
                    "blur" => {
                        let Token::Dimension { value, unit, .. } = input.next()?.clone() else {
                            return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                        };
                        if !unit.eq_ignore_ascii_case("px") || !(0.0..=24.0).contains(&value) {
                            return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                        }
                        BackgroundEffect::Blur(value.round() as u8)
                    }
                    "oil-paint" | "oil" => {
                        let radius = input.expect_number()?;
                        if !(1.0..=3.0).contains(&radius) {
                            return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                        }
                        BackgroundEffect::OilPaint(radius.round() as u8)
                    }
                    "grayscale" | "black-white" | "invert" | "contrast" => {
                        let value = match input.next()?.clone() {
                            Token::Number { value, .. } => value,
                            Token::Percentage { unit_value, .. } => unit_value,
                            _ => {
                                return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                            }
                        };
                        if name.eq_ignore_ascii_case("contrast") {
                            if !(0.0..=4.0).contains(&value) {
                                return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                            }
                            BackgroundEffect::Contrast((value * 100.0).round() as u16)
                        } else {
                            if !(0.0..=1.0).contains(&value) {
                                return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                            }
                            let amount = (value * 100.0).round() as u8;
                            if name.eq_ignore_ascii_case("invert") {
                                BackgroundEffect::Invert(amount)
                            } else {
                                BackgroundEffect::Grayscale(amount)
                            }
                        }
                    }
                    _ => return Err(input.new_custom_error::<_, StyleParseError>(invalid())),
                };
                input.expect_exhausted()?;
                Ok(effect)
            })
            .map_err(|_| invalid())?;
        effects.push(effect);
        if effects.len() > 8 {
            return Err(invalid());
        }
    }
    if effects.is_empty() {
        return Err(invalid());
    }
    Ok(effects)
}

fn parse_animated_effects(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<AnimatedEffect>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return Ok(Vec::new());
    }
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: "animated-filter".into(),
        value: concat!(
            "expected noise, signal-lost, old-movie, side-glow, bloom, ",
            "water-pearls or water-wave (strength, speed)"
        )
        .into(),
    };
    let mut effects = Vec::new();
    while !input.is_exhausted() {
        let Token::Function(name) = input.next().map_err(|_| invalid())?.clone() else {
            return Err(invalid());
        };
        let kind = match name.to_ascii_lowercase().as_str() {
            "noise" => AnimatedEffectKind::Noise,
            "signal-lost" | "retro-tv" => AnimatedEffectKind::SignalLost,
            "old-movie" | "old-film" => AnimatedEffectKind::OldMovie,
            "side-glow" => AnimatedEffectKind::SideGlow,
            "bloom" => AnimatedEffectKind::Bloom,
            "water-pearls" => AnimatedEffectKind::WaterPearls,
            "water-wave" => AnimatedEffectKind::WaterWave,
            _ => return Err(invalid()),
        };
        let (strength, speed) = input
            .parse_nested_block(|input| {
                let strength = input.expect_number()?;
                let speed = input
                    .try_parse(|input| {
                        input.expect_comma()?;
                        input.expect_number()
                    })
                    .unwrap_or(1.0);
                if !(0.0..=1.0).contains(&strength) || !(0.0..=4.0).contains(&speed) {
                    return Err(input.new_custom_error::<_, StyleParseError>(invalid()));
                }
                input.expect_exhausted()?;
                Ok((strength, speed))
            })
            .map_err(|_| invalid())?;
        effects.push(AnimatedEffect {
            kind,
            strength: (strength * 100.0).round() as u8,
            speed: (speed * 100.0).round() as u16,
        });
        if effects.len() > 8 {
            return Err(invalid());
        }
    }
    if effects.is_empty() {
        return Err(invalid());
    }
    Ok(effects)
}

fn parse_animation_list(input: &mut Parser<'_, '_>) -> Result<Vec<AnimationSpec>, StyleParseError> {
    input.parse_comma_separated(parse_animation).map_err(|_| {
        StyleParseError::InvalidPropertyValue {
            property: "animation".into(),
            value: "invalid animation shorthand".into(),
        }
    })
}

fn parse_animation<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<AnimationSpec, cssparser::ParseError<'i, StyleParseError>> {
    let mut spec = AnimationSpec::default();
    let mut has_duration = false;
    let mut has_delay = false;
    while !input.is_exhausted() {
        let token = input.next()?.clone();
        match token {
            Token::Dimension { value, unit, .. }
                if unit.eq_ignore_ascii_case("ms") || unit.eq_ignore_ascii_case("s") =>
            {
                let seconds = if unit.eq_ignore_ascii_case("ms") {
                    value / 1000.0
                } else {
                    value
                };
                if seconds < 0.0 {
                    return Err(
                        input.new_custom_error(StyleParseError::InvalidPropertyValue {
                            property: "animation".into(),
                            value: "negative times are unsupported".into(),
                        }),
                    );
                }
                if !has_duration {
                    spec.duration = CssTime(seconds);
                    has_duration = true;
                } else if !has_delay {
                    spec.delay = CssTime(seconds);
                    has_delay = true;
                } else {
                    return Err(
                        input.new_custom_error(StyleParseError::InvalidPropertyValue {
                            property: "animation".into(),
                            value: "too many time values".into(),
                        }),
                    );
                }
            }
            Token::Number { value, .. } if value >= 0.0 => {
                spec.iteration_count = IterationCount::Finite(value)
            }
            Token::Ident(value) => match value.to_ascii_lowercase().as_str() {
                "none" => spec.name = None,
                "infinite" => spec.iteration_count = IterationCount::Infinite,
                "linear" => spec.timing_function = TimingFunction::Linear,
                "ease" => spec.timing_function = TimingFunction::Ease,
                "ease-in" => spec.timing_function = TimingFunction::EaseIn,
                "ease-out" => spec.timing_function = TimingFunction::EaseOut,
                "ease-in-out" => spec.timing_function = TimingFunction::EaseInOut,
                "normal" => spec.direction = AnimationDirection::Normal,
                "reverse" => spec.direction = AnimationDirection::Reverse,
                "alternate" => spec.direction = AnimationDirection::Alternate,
                "alternate-reverse" => spec.direction = AnimationDirection::AlternateReverse,
                name if spec.name.is_none() => spec.name = Some(AnimationName(name.to_owned())),
                _ => {
                    return Err(
                        input.new_custom_error(StyleParseError::InvalidPropertyValue {
                            property: "animation".into(),
                            value: "duplicate animation name".into(),
                        }),
                    );
                }
            },
            _ => {
                return Err(
                    input.new_custom_error(StyleParseError::InvalidPropertyValue {
                        property: "animation".into(),
                        value: "unsupported shorthand token".into(),
                    }),
                );
            }
        }
    }
    Ok(spec)
}

fn parse_animation_names(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<Option<AnimationName>>, StyleParseError> {
    input
        .parse_comma_separated(
            |input| -> Result<Option<AnimationName>, cssparser::ParseError<'_, StyleParseError>> {
                let name = input.expect_ident()?;
                Ok(if name.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(AnimationName(name.to_string()))
                })
            },
        )
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "animation-name".into(),
            value: "expected animation names".into(),
        })
}

fn parse_time_list(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<Vec<CssTime>, StyleParseError> {
    input
        .parse_comma_separated(parse_time)
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: property.into(),
            value: "expected non-negative ms or s time".into(),
        })
}

fn parse_time<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<CssTime, cssparser::ParseError<'i, StyleParseError>> {
    match input.next()?.clone() {
        Token::Dimension { value, unit, .. } if value >= 0.0 && unit.eq_ignore_ascii_case("ms") => {
            Ok(CssTime(value / 1000.0))
        }
        Token::Dimension { value, unit, .. } if value >= 0.0 && unit.eq_ignore_ascii_case("s") => {
            Ok(CssTime(value))
        }
        _ => Err(
            input.new_custom_error(StyleParseError::InvalidPropertyValue {
                property: "time".into(),
                value: "expected non-negative ms or s time".into(),
            }),
        ),
    }
}

fn parse_timing_list(input: &mut Parser<'_, '_>) -> Result<Vec<TimingFunction>, StyleParseError> {
    input
        .parse_comma_separated(parse_timing)
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "animation-timing-function".into(),
            value: "unsupported timing function".into(),
        })
}

fn parse_timing<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<TimingFunction, cssparser::ParseError<'i, StyleParseError>> {
    let ident = input.expect_ident()?;
    match ident.to_ascii_lowercase().as_str() {
        "linear" => Ok(TimingFunction::Linear),
        "ease" => Ok(TimingFunction::Ease),
        "ease-in" => Ok(TimingFunction::EaseIn),
        "ease-out" => Ok(TimingFunction::EaseOut),
        "ease-in-out" => Ok(TimingFunction::EaseInOut),
        _ => Err(
            input.new_custom_error(StyleParseError::InvalidPropertyValue {
                property: "timing-function".into(),
                value: "unsupported timing function".into(),
            }),
        ),
    }
}

fn parse_iterations(input: &mut Parser<'_, '_>) -> Result<Vec<IterationCount>, StyleParseError> {
    input
        .parse_comma_separated(
            |input| -> Result<IterationCount, cssparser::ParseError<'_, StyleParseError>> {
                match input.next()?.clone() {
                    Token::Ident(value) if value.eq_ignore_ascii_case("infinite") => {
                        Ok(IterationCount::Infinite)
                    }
                    Token::Number { value, .. } if value >= 0.0 => {
                        Ok(IterationCount::Finite(value))
                    }
                    _ => Err(
                        input.new_custom_error(StyleParseError::InvalidPropertyValue {
                            property: "animation-iteration-count".into(),
                            value: "expected non-negative number or infinite".into(),
                        }),
                    ),
                }
            },
        )
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "animation-iteration-count".into(),
            value: "invalid iteration count".into(),
        })
}

fn parse_directions(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<AnimationDirection>, StyleParseError> {
    input
        .parse_comma_separated(
            |input| -> Result<AnimationDirection, cssparser::ParseError<'_, StyleParseError>> {
                let ident = input.expect_ident()?;
                match ident.to_ascii_lowercase().as_str() {
                    "normal" => Ok(AnimationDirection::Normal),
                    "reverse" => Ok(AnimationDirection::Reverse),
                    "alternate" => Ok(AnimationDirection::Alternate),
                    "alternate-reverse" => Ok(AnimationDirection::AlternateReverse),
                    _ => Err(
                        input.new_custom_error(StyleParseError::InvalidPropertyValue {
                            property: "animation-direction".into(),
                            value: "invalid direction".into(),
                        }),
                    ),
                }
            },
        )
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "animation-direction".into(),
            value: "invalid direction".into(),
        })
}

fn parse_transition_list(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<TransitionSpec>, StyleParseError> {
    if input
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        input
            .expect_exhausted()
            .map_err(|_| StyleParseError::InvalidPropertyValue {
                property: "transition".into(),
                value: "`none` cannot be combined with other transitions".into(),
            })?;
        return Ok(Vec::new());
    }
    input
        .parse_comma_separated(|input| {
            let property = parse_transition_property(input)?;
            let duration = parse_time(input)?;
            let timing_function = input.try_parse(parse_timing).unwrap_or_default();
            let delay = input.try_parse(parse_time).unwrap_or(CssTime(0.0));
            Ok(TransitionSpec {
                property,
                duration,
                delay,
                timing_function,
            })
        })
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "transition".into(),
            value: "expected property duration [timing] [delay]".into(),
        })
}

fn parse_transition_properties(
    input: &mut Parser<'_, '_>,
) -> Result<Vec<TransitionProperty>, StyleParseError> {
    input
        .parse_comma_separated(parse_transition_property)
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "transition-property".into(),
            value: "unsupported transition property".into(),
        })
}

fn parse_transition_property<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<TransitionProperty, cssparser::ParseError<'i, StyleParseError>> {
    match input.expect_ident()?.to_ascii_lowercase().as_str() {
        "none" => Ok(TransitionProperty::None),
        "all" => Ok(TransitionProperty::All),
        "color" => Ok(TransitionProperty::Color),
        "background" | "background-color" => Ok(TransitionProperty::BackgroundColor),
        "border-color" => Ok(TransitionProperty::BorderColor),
        "border-radius" => Ok(TransitionProperty::BorderRadius),
        "font-size" => Ok(TransitionProperty::FontSize),
        "opacity" => Ok(TransitionProperty::Opacity),
        "transform" => Ok(TransitionProperty::Transform),
        "width" => Ok(TransitionProperty::Width),
        "height" => Ok(TransitionProperty::Height),
        _ => Err(
            input.new_custom_error(StyleParseError::InvalidPropertyValue {
                property: "transition".into(),
                value: "unsupported transition property".into(),
            }),
        ),
    }
}

fn parse_transform(input: &mut Parser<'_, '_>) -> Result<CssTransform, StyleParseError> {
    let mut transform = CssTransform::default();
    while !input.is_exhausted() {
        let name = match input
            .next()
            .map_err(|_| StyleParseError::InvalidPropertyValue {
                property: "transform".into(),
                value: "expected transform function".into(),
            })?
            .clone()
        {
            Token::Function(name) => name.to_ascii_lowercase(),
            _ => {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "transform".into(),
                    value: "expected transform function".into(),
                });
            }
        };
        input
            .parse_nested_block(|input| match name.as_str() {
                "translate" => {
                    transform.translate_x = super::value::parse_length(input, false)
                        .map_err(|e| input.new_custom_error(e))?;
                    input.expect_comma()?;
                    transform.translate_y = super::value::parse_length(input, false)
                        .map_err(|e| input.new_custom_error(e))?;
                    Ok(())
                }
                "translatex" => {
                    transform.translate_x = super::value::parse_length(input, false)
                        .map_err(|e| input.new_custom_error(e))?;
                    Ok(())
                }
                "translatey" => {
                    transform.translate_y = super::value::parse_length(input, false)
                        .map_err(|e| input.new_custom_error(e))?;
                    Ok(())
                }
                "scale" => {
                    let x = input.expect_number()?;
                    transform.scale_x = x;
                    transform.scale_y = input
                        .try_parse(|p| {
                            p.expect_comma()?;
                            p.expect_number()
                        })
                        .unwrap_or(x);
                    Ok(())
                }
                "scalex" => {
                    transform.scale_x = input.expect_number()?;
                    Ok(())
                }
                "scaley" => {
                    transform.scale_y = input.expect_number()?;
                    Ok(())
                }
                "rotate" => {
                    transform.rotation = parse_angle(input)?;
                    Ok(())
                }
                _ => Err(
                    input.new_custom_error(StyleParseError::InvalidPropertyValue {
                        property: "transform".into(),
                        value: "unsupported transform function".into(),
                    }),
                ),
            })
            .map_err(|_| StyleParseError::InvalidPropertyValue {
                property: "transform".into(),
                value: "invalid transform function".into(),
            })?;
    }
    Ok(transform)
}

fn parse_angle<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<f32, cssparser::ParseError<'i, StyleParseError>> {
    match input.next()?.clone() {
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("deg") => {
            Ok(value.to_radians())
        }
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("rad") => Ok(value),
        _ => Err(
            input.new_custom_error(StyleParseError::InvalidPropertyValue {
                property: "transform".into(),
                value: "expected deg or rad angle".into(),
            }),
        ),
    }
}

fn parse_ident(input: &mut Parser<'_, '_>, property: &str) -> Result<String, StyleParseError> {
    input
        .expect_ident()
        .map(|value| value.to_string())
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "expected an identifier".into(),
        })
}

fn parse_display(input: &mut Parser<'_, '_>) -> Result<Display, StyleParseError> {
    match parse_ident(input, "display")?.as_str() {
        "block" => Ok(Display::Block),
        "flex" => Ok(Display::Flex),
        "grid" => Ok(Display::Grid),
        "none" => Ok(Display::None),
        value => invalid_keyword("display", value),
    }
}

fn parse_overflow_value(input: &mut Parser<'_, '_>) -> Result<CssOverflow, StyleParseError> {
    match parse_ident(input, "overflow")?.as_str() {
        "visible" => Ok(CssOverflow::Visible),
        "hidden" => Ok(CssOverflow::Hidden),
        "clip" => Ok(CssOverflow::Clip),
        "scroll" => Ok(CssOverflow::Scroll),
        "auto" => Ok(CssOverflow::Auto),
        value => invalid_keyword("overflow", value),
    }
}

fn parse_position(input: &mut Parser<'_, '_>) -> Result<Position, StyleParseError> {
    match parse_ident(input, "position")?.as_str() {
        "static" => Ok(Position::Static),
        "relative" => Ok(Position::Relative),
        "absolute" => Ok(Position::Absolute),
        value => invalid_keyword("position", value),
    }
}

fn parse_flex(input: &mut Parser<'_, '_>) -> Result<(f32, f32, crate::Length), StyleParseError> {
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: "flex".into(),
        value: "expected grow, optional shrink and basis, or none/auto/initial".into(),
    };
    if let Ok(keyword) =
        input.try_parse(|input| input.expect_ident().map(|word| word.to_ascii_lowercase()))
    {
        return match keyword.as_str() {
            "none" => Ok((0.0, 0.0, crate::Length::Auto)),
            "auto" => Ok((1.0, 1.0, crate::Length::Auto)),
            "initial" => Ok((0.0, 1.0, crate::Length::Auto)),
            _ => Err(invalid()),
        };
    }
    let Ok(grow) = input.try_parse(|input| parse_non_negative_number(input, "flex")) else {
        let basis = parse_length(input, true)?;
        if input.is_exhausted() {
            return Ok((1.0, 1.0, basis));
        }
        return Err(invalid());
    };
    let shrink = input
        .try_parse(|input| parse_non_negative_number(input, "flex"))
        .unwrap_or(1.0);
    let basis = input
        .try_parse(|input| parse_length(input, true))
        .unwrap_or(crate::Length::Percent(0.0));
    Ok((grow, shrink, basis))
}

fn parse_flex_flow(
    input: &mut Parser<'_, '_>,
) -> Result<(FlexDirection, FlexWrap), StyleParseError> {
    let mut direction = None;
    let mut wrap = None;
    while !input.is_exhausted() {
        if direction.is_none() {
            if let Ok(value) = input.try_parse(|input| parse_flex_direction(input)) {
                direction = Some(value);
                continue;
            }
        }
        if wrap.is_none() {
            if let Ok(value) = input.try_parse(|input| parse_flex_wrap(input)) {
                wrap = Some(value);
                continue;
            }
        }
        return Err(StyleParseError::InvalidPropertyValue {
            property: "flex-flow".into(),
            value: "expected a direction and/or wrap mode".into(),
        });
    }
    if direction.is_none() && wrap.is_none() {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "flex-flow".into(),
            value: "empty value".into(),
        });
    }
    Ok((
        direction.unwrap_or(FlexDirection::Row),
        wrap.unwrap_or(FlexWrap::NoWrap),
    ))
}

fn parse_flex_direction(input: &mut Parser<'_, '_>) -> Result<FlexDirection, StyleParseError> {
    match parse_ident(input, "flex-direction")?.as_str() {
        "row" => Ok(FlexDirection::Row),
        "column" => Ok(FlexDirection::Column),
        "row-reverse" => Ok(FlexDirection::RowReverse),
        "column-reverse" => Ok(FlexDirection::ColumnReverse),
        value => invalid_keyword("flex-direction", value),
    }
}

fn parse_flex_wrap(input: &mut Parser<'_, '_>) -> Result<FlexWrap, StyleParseError> {
    match parse_ident(input, "flex-wrap")?.as_str() {
        "nowrap" => Ok(FlexWrap::NoWrap),
        "wrap" => Ok(FlexWrap::Wrap),
        "wrap-reverse" => Ok(FlexWrap::WrapReverse),
        value => invalid_keyword("flex-wrap", value),
    }
}

fn parse_justify_content(input: &mut Parser<'_, '_>) -> Result<JustifyContent, StyleParseError> {
    match parse_ident(input, "justify-content")?.as_str() {
        "start" | "flex-start" => Ok(JustifyContent::Start),
        "end" | "flex-end" => Ok(JustifyContent::End),
        "center" => Ok(JustifyContent::Center),
        "space-between" => Ok(JustifyContent::SpaceBetween),
        "space-around" => Ok(JustifyContent::SpaceAround),
        "space-evenly" => Ok(JustifyContent::SpaceEvenly),
        value => invalid_keyword("justify-content", value),
    }
}

fn parse_align_items(input: &mut Parser<'_, '_>) -> Result<AlignItems, StyleParseError> {
    match parse_ident(input, "align-items")?.as_str() {
        "start" | "flex-start" => Ok(AlignItems::Start),
        "end" | "flex-end" => Ok(AlignItems::End),
        "center" => Ok(AlignItems::Center),
        "stretch" => Ok(AlignItems::Stretch),
        value => invalid_keyword("align-items", value),
    }
}

fn parse_align_self(input: &mut Parser<'_, '_>) -> Result<AlignSelf, StyleParseError> {
    match parse_ident(input, "align-self")?.as_str() {
        "auto" => Ok(AlignSelf::Auto),
        "start" | "flex-start" => Ok(AlignSelf::Start),
        "end" | "flex-end" => Ok(AlignSelf::End),
        "center" => Ok(AlignSelf::Center),
        "stretch" => Ok(AlignSelf::Stretch),
        value => invalid_keyword("align-self", value),
    }
}

fn parse_font_family(input: &mut Parser<'_, '_>) -> Result<FontFamily, StyleParseError> {
    let mut family = Vec::new();
    while !input.is_exhausted() {
        let token = input
            .next()
            .map_err(|_| StyleParseError::InvalidPropertyValue {
                property: "font-family".into(),
                value: "expected a family name".into(),
            })?
            .clone();
        match token {
            Token::Ident(value) => family.push(value.to_string()),
            Token::QuotedString(value) if family.is_empty() => family.push(value.to_string()),
            Token::Comma if !family.is_empty() => {
                while !input.is_exhausted() {
                    input
                        .next()
                        .map_err(|_| StyleParseError::InvalidPropertyValue {
                            property: "font-family".into(),
                            value: "invalid fallback list".into(),
                        })?;
                }
                break;
            }
            _ => {
                return Err(StyleParseError::InvalidPropertyValue {
                    property: "font-family".into(),
                    value: "invalid family name".into(),
                });
            }
        }
    }
    let family = family.join(" ");
    if family.is_empty() {
        return Err(StyleParseError::InvalidPropertyValue {
            property: "font-family".into(),
            value: "empty family name".into(),
        });
    }
    Ok(match family.to_ascii_lowercase().as_str() {
        "sans-serif" => Ok(FontFamily::SansSerif),
        "ui-symbols" => Ok(FontFamily::UiSymbols),
        "monospace" => Ok(FontFamily::Monospace),
        "serif" => Ok(FontFamily::Serif),
        "cursive" => Ok(FontFamily::Cursive),
        "fantasy" => Ok(FontFamily::Fantasy),
        "system-ui" => Ok(FontFamily::SystemUi),
        "emoji" => Ok(FontFamily::Emoji),
        _ => Ok(FontFamily::Named(family)),
    }?)
}

fn parse_line_height(input: &mut Parser<'_, '_>) -> Result<CssLineHeight, StyleParseError> {
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: "line-height".into(),
        value: "expected normal, a positive number, percentage, or px length".into(),
    };
    match input.next().map_err(|_| invalid())?.clone() {
        Token::Ident(value) if value.eq_ignore_ascii_case("normal") => Ok(CssLineHeight::Normal),
        Token::Number { value, .. } if value >= 0.0 => Ok(CssLineHeight::Relative(value)),
        Token::Percentage { unit_value, .. } if unit_value >= 0.0 => {
            Ok(CssLineHeight::Relative(unit_value))
        }
        Token::Dimension { value, unit, .. } if value >= 0.0 && unit.eq_ignore_ascii_case("px") => {
            Ok(CssLineHeight::Pixels(value))
        }
        _ => Err(invalid()),
    }
}

fn parse_z_index(input: &mut Parser<'_, '_>) -> Result<i32, StyleParseError> {
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: "z-index".into(),
        value: "expected an integer or auto".into(),
    };
    match input.next().map_err(|_| invalid())?.clone() {
        Token::Ident(value) if value.eq_ignore_ascii_case("auto") => Ok(0),
        Token::Number {
            int_value: Some(value),
            ..
        } => Ok(value),
        _ => Err(invalid()),
    }
}

fn parse_font_weight(input: &mut Parser<'_, '_>) -> Result<FontWeight, StyleParseError> {
    let token = input
        .next()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "font-weight".into(),
            value: "expected a font weight".into(),
        })?
        .clone();
    match token {
        Token::Ident(value) if value.eq_ignore_ascii_case("normal") => Ok(FontWeight::Normal),
        Token::Ident(value) if value.eq_ignore_ascii_case("bold") => Ok(FontWeight::Bold),
        Token::Number {
            int_value: Some(value),
            ..
        } if (1..=1000).contains(&value) => Ok(FontWeight::Number(value as u16)),
        _ => Err(StyleParseError::InvalidPropertyValue {
            property: "font-weight".into(),
            value: "expected normal, bold, or a number from 1 to 1000".into(),
        }),
    }
}

fn parse_text_align(input: &mut Parser<'_, '_>) -> Result<TextAlign, StyleParseError> {
    match parse_ident(input, "text-align")?.as_str() {
        "start" | "left" => Ok(TextAlign::Start),
        "end" | "right" => Ok(TextAlign::End),
        "center" => Ok(TextAlign::Center),
        "justify" => Ok(TextAlign::Justify),
        value => invalid_keyword("text-align", value),
    }
}

fn parse_non_negative_number(
    input: &mut Parser<'_, '_>,
    property: &str,
) -> Result<f32, StyleParseError> {
    let value = input
        .expect_number()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "expected a number".into(),
        })?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(StyleParseError::InvalidPropertyValue {
            property: property.to_owned(),
            value: "must not be negative".into(),
        })
    }
}

fn parse_opacity(input: &mut Parser<'_, '_>) -> Result<f32, StyleParseError> {
    let token = input
        .next()
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "opacity".into(),
            value: "expected a number or percentage".into(),
        })?
        .clone();
    let value = match token {
        Token::Number { value, .. } => value,
        Token::Percentage { unit_value, .. } => unit_value,
        _ => {
            return Err(StyleParseError::InvalidPropertyValue {
                property: "opacity".into(),
                value: "expected a number or percentage".into(),
            });
        }
    };
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(StyleParseError::InvalidPropertyValue {
            property: "opacity".into(),
            value: "must be between 0 and 1".into(),
        })
    }
}

fn invalid_keyword<T>(property: &str, value: &str) -> Result<T, StyleParseError> {
    Err(StyleParseError::InvalidPropertyValue {
        property: property.to_owned(),
        value: value.to_owned(),
    })
}
