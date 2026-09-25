use cssparser::{Parser, Token};

use crate::{
    AlignItems, AlignSelf, AnimationDirection, AnimationName, AnimationSpec, CssGradient,
    CssOverflow, CssTime, CssTransform, Display, FlexDirection, FlexWrap, FontFamily, FontWeight,
    GradientDirection, IterationCount, JustifyContent, Position, StyleDeclaration, TextAlign,
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
        "background-color" => StyleDeclaration::BackgroundColor(parse_color(input)?),
        "background-image" => StyleDeclaration::BackgroundImage(parse_background_image(input)?),
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
        "border-color" => StyleDeclaration::BorderColor(parse_color(input)?),
        "border-radius" => StyleDeclaration::BorderRadius(parse_border_radius(input)?),
        "font-size" => StyleDeclaration::FontSize(parse_length(input, false)?),
        "font-family" => StyleDeclaration::FontFamily(parse_font_family(input)?),
        "font-weight" => StyleDeclaration::FontWeight(parse_font_weight(input)?),
        "text-align" => StyleDeclaration::TextAlign(parse_text_align(input)?),
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

fn parse_background_image(
    input: &mut Parser<'_, '_>,
) -> Result<Option<CssGradient>, StyleParseError> {
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
        })?;
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
            Ok(Some(CssGradient {
                direction: direction.unwrap_or(GradientDirection::Down),
                stops,
            }))
        })
        .map_err(|_| StyleParseError::InvalidPropertyValue {
            property: "background-image".into(),
            value: "invalid linear-gradient()".into(),
        })
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
    match parse_ident(input, "font-family")?.as_str() {
        "sans-serif" => Ok(FontFamily::SansSerif),
        "ui-symbols" => Ok(FontFamily::UiSymbols),
        "monospace" => Ok(FontFamily::Monospace),
        value => invalid_keyword("font-family", value),
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
