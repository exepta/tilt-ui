use cssparser::{Parser, Token};

use crate::{BorderRadius, CssColor, Edges, Length};

use super::StyleParseError;

pub(crate) fn parse_length(
    input: &mut Parser<'_, '_>,
    allow_auto: bool,
) -> Result<Length, StyleParseError> {
    let token = input
        .next()
        .map_err(|_| StyleParseError::InvalidLength("expected a length".into()))?
        .clone();
    match token {
        Token::Ident(value) if allow_auto && value.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
        Token::Number { value, .. } if value == 0.0 => Ok(Length::Px(0.0)),
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Ok(Length::Px(value))
        }
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("vw") => {
            Ok(Length::Vw(value))
        }
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("vh") => {
            Ok(Length::Vh(value))
        }
        Token::Percentage { unit_value, .. } => Ok(Length::Percent(unit_value * 100.0)),
        Token::Dimension { unit, .. } => Err(StyleParseError::InvalidLength(format!(
            "unsupported unit `{unit}`"
        ))),
        _ => Err(StyleParseError::InvalidLength(
            "expected auto, px, %, vw, or vh".into(),
        )),
    }
}

pub(crate) fn parse_edges(
    input: &mut Parser<'_, '_>,
    allow_auto: bool,
) -> Result<Edges<Length>, StyleParseError> {
    let first = parse_length(input, allow_auto)?;
    let second = input
        .try_parse(|input| parse_length(input, allow_auto))
        .ok();
    let third = input
        .try_parse(|input| parse_length(input, allow_auto))
        .ok();
    let fourth = input
        .try_parse(|input| parse_length(input, allow_auto))
        .ok();

    Ok(match (second, third, fourth) {
        (None, _, _) => Edges::all(first),
        (Some(horizontal), None, _) => Edges {
            top: first,
            right: horizontal,
            bottom: first,
            left: horizontal,
        },
        (Some(right), Some(bottom), None) => Edges {
            top: first,
            right,
            bottom,
            left: right,
        },
        (Some(right), Some(bottom), Some(left)) => Edges {
            top: first,
            right,
            bottom,
            left,
        },
    })
}

pub(crate) fn parse_border_radius(
    input: &mut Parser<'_, '_>,
) -> Result<BorderRadius, StyleParseError> {
    let edges = parse_edges(input, false)?;
    Ok(BorderRadius {
        top_left: edges.top,
        top_right: edges.right,
        bottom_right: edges.bottom,
        bottom_left: edges.left,
    })
}

pub(crate) fn parse_color(input: &mut Parser<'_, '_>) -> Result<CssColor, StyleParseError> {
    let token = input
        .next()
        .map_err(|_| StyleParseError::InvalidColor("expected a color".into()))?
        .clone();
    match token {
        Token::Hash(value) | Token::IDHash(value) => parse_hex_color(&value),
        Token::Ident(value) if value.eq_ignore_ascii_case("transparent") => {
            Ok(CssColor::transparent())
        }
        Token::Ident(value) => {
            let (red, green, blue) =
                cssparser::color::parse_named_color(&value.to_ascii_lowercase()).map_err(|_| {
                    StyleParseError::InvalidColor(format!("unknown color `{value}`"))
                })?;
            Ok(CssColor::rgba(
                red as f32 / 255.0,
                green as f32 / 255.0,
                blue as f32 / 255.0,
                1.0,
            ))
        }
        Token::Function(name) if name.eq_ignore_ascii_case("rgb") => input
            .parse_nested_block(parse_rgb)
            .map_err(|_| StyleParseError::InvalidColor("invalid rgb() color".into())),
        Token::Function(name) if name.eq_ignore_ascii_case("rgba") => input
            .parse_nested_block(parse_rgba)
            .map_err(|_| StyleParseError::InvalidColor("invalid rgba() color".into())),
        _ => Err(StyleParseError::InvalidColor(
            "expected named color, hex, rgb(), rgba(), or transparent".into(),
        )),
    }
}

fn parse_hex_color(value: &str) -> Result<CssColor, StyleParseError> {
    let (red, green, blue, alpha) = cssparser::color::parse_hash_color(value.as_bytes())
        .map_err(|_| StyleParseError::InvalidColor(format!("invalid hex color `#{value}`")))?;
    Ok(CssColor::rgba(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    ))
}

fn parse_rgb<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<CssColor, cssparser::ParseError<'i, StyleParseError>> {
    let red = parse_color_channel(input)?;
    input.expect_comma()?;
    let green = parse_color_channel(input)?;
    input.expect_comma()?;
    let blue = parse_color_channel(input)?;
    input.expect_exhausted()?;
    Ok(CssColor::rgba(red, green, blue, 1.0))
}

fn parse_rgba<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<CssColor, cssparser::ParseError<'i, StyleParseError>> {
    let red = parse_color_channel(input)?;
    input.expect_comma()?;
    let green = parse_color_channel(input)?;
    input.expect_comma()?;
    let blue = parse_color_channel(input)?;
    input.expect_comma()?;
    let alpha = parse_alpha_channel(input)?;
    input.expect_exhausted()?;
    Ok(CssColor::rgba(red, green, blue, alpha))
}

fn parse_color_channel<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<f32, cssparser::ParseError<'i, StyleParseError>> {
    let token = input.next()?.clone();
    let value = match token {
        Token::Number { value, .. } if (0.0..=255.0).contains(&value) => value / 255.0,
        Token::Percentage { unit_value, .. } if (0.0..=1.0).contains(&unit_value) => unit_value,
        _ => {
            return Err(
                input.new_custom_error(StyleParseError::InvalidColor("invalid RGB channel".into()))
            );
        }
    };
    Ok(value)
}

fn parse_alpha_channel<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<f32, cssparser::ParseError<'i, StyleParseError>> {
    let token = input.next()?.clone();
    match token {
        Token::Number { value, .. }
        | Token::Percentage {
            unit_value: value, ..
        } if (0.0..=1.0).contains(&value) => Ok(value),
        _ => Err(input.new_custom_error(StyleParseError::InvalidColor(
            "invalid alpha channel".into(),
        ))),
    }
}
