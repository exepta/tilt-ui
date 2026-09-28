use crate::{
    GridAutoFlow, GridLine, GridPlacement, GridRepetition, GridTrackGroup, GridTrackSize, Length,
    StyleDeclaration,
};

use super::StyleParseError;

pub(super) fn parse_grid_declaration(
    property: &str,
    source: &str,
) -> Result<Option<StyleDeclaration>, StyleParseError> {
    let invalid = || StyleParseError::InvalidPropertyValue {
        property: property.to_owned(),
        value: source.to_owned(),
    };
    let lowercase = source.trim().to_ascii_lowercase();
    let source = lowercase.as_str();
    let declaration = match property {
        "grid-template-rows" => {
            StyleDeclaration::GridTemplateRows(parse_track_groups(source).ok_or_else(invalid)?)
        }
        "grid-template-columns" => {
            StyleDeclaration::GridTemplateColumns(parse_track_groups(source).ok_or_else(invalid)?)
        }
        "grid-auto-rows" => {
            StyleDeclaration::GridAutoRows(parse_auto_tracks(source).ok_or_else(invalid)?)
        }
        "grid-auto-columns" => {
            StyleDeclaration::GridAutoColumns(parse_auto_tracks(source).ok_or_else(invalid)?)
        }
        "grid-auto-flow" => {
            let words = source.split_ascii_whitespace().collect::<Vec<_>>();
            let flow = match words.as_slice() {
                ["row"] => GridAutoFlow::Row,
                ["column"] => GridAutoFlow::Column,
                ["dense"] | ["row", "dense"] | ["dense", "row"] => GridAutoFlow::RowDense,
                ["column", "dense"] | ["dense", "column"] => GridAutoFlow::ColumnDense,
                _ => return Err(invalid()),
            };
            StyleDeclaration::GridAutoFlow(flow)
        }
        "grid-row" => StyleDeclaration::GridRow(parse_placement(source).ok_or_else(invalid)?),
        "grid-column" => StyleDeclaration::GridColumn(parse_placement(source).ok_or_else(invalid)?),
        _ => return Ok(None),
    };
    Ok(Some(declaration))
}

fn parse_track_groups(source: &str) -> Option<Vec<GridTrackGroup>> {
    if source.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut groups = Vec::new();
    for item in split_top_level(source, ' ')? {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some(inner) = function_args(item, "repeat") {
            let args = split_top_level(inner, ',')?;
            if args.len() != 2 {
                return None;
            }
            let repetition = match args[0].trim() {
                "auto-fill" => GridRepetition::AutoFill,
                "auto-fit" => GridRepetition::AutoFit,
                count => {
                    GridRepetition::Count(count.parse::<u16>().ok().filter(|count| *count > 0)?)
                }
            };
            let tracks = split_top_level(args[1].trim(), ' ')?
                .into_iter()
                .filter(|part| !part.trim().is_empty())
                .map(|part| parse_track(part.trim()))
                .collect::<Option<Vec<_>>>()?;
            if tracks.is_empty() {
                return None;
            }
            groups.push(GridTrackGroup { repetition, tracks });
        } else {
            groups.push(GridTrackGroup {
                repetition: GridRepetition::Count(1),
                tracks: vec![parse_track(item)?],
            });
        }
    }
    (!groups.is_empty()).then_some(groups)
}

fn parse_auto_tracks(source: &str) -> Option<Vec<GridTrackSize>> {
    let tracks = split_top_level(source, ' ')?
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(|part| parse_track(part.trim()))
        .collect::<Option<Vec<_>>>()?;
    (!tracks.is_empty()).then_some(tracks)
}

fn parse_track(source: &str) -> Option<GridTrackSize> {
    if let Some(inner) = function_args(source, "minmax") {
        let args = split_top_level(inner, ',')?;
        if args.len() != 2 {
            return None;
        }
        let min = parse_track(args[0].trim())?;
        let max = parse_track(args[1].trim())?;
        if matches!(
            min,
            GridTrackSize::Fraction(_) | GridTrackSize::MinMax(_, _)
        ) || matches!(max, GridTrackSize::MinMax(_, _))
        {
            return None;
        }
        return Some(GridTrackSize::MinMax(Box::new(min), Box::new(max)));
    }
    match source {
        "auto" => Some(GridTrackSize::Auto),
        "min-content" => Some(GridTrackSize::MinContent),
        "max-content" => Some(GridTrackSize::MaxContent),
        "0" => Some(GridTrackSize::Length(Length::Px(0.0))),
        _ => {
            for (suffix, wrap) in [("fr", 0u8), ("px", 1), ("%", 2), ("vw", 3), ("vh", 4)] {
                if let Some(number) = source.strip_suffix(suffix) {
                    let value = number
                        .parse::<f32>()
                        .ok()
                        .filter(|value| value.is_finite() && *value >= 0.0)?;
                    return Some(match wrap {
                        0 => GridTrackSize::Fraction(value),
                        1 => GridTrackSize::Length(Length::Px(value)),
                        2 => GridTrackSize::Length(Length::Percent(value)),
                        3 => GridTrackSize::Length(Length::Vw(value)),
                        _ => GridTrackSize::Length(Length::Vh(value)),
                    });
                }
            }
            None
        }
    }
}

fn parse_placement(source: &str) -> Option<GridPlacement> {
    let parts = split_top_level(source, '/')?;
    match parts.as_slice() {
        [start] => Some(GridPlacement {
            start: parse_grid_line(start.trim())?,
            end: GridLine::Auto,
        }),
        [start, end] => {
            let start = parse_grid_line(start.trim())?;
            let end = parse_grid_line(end.trim())?;
            if matches!((start, end), (GridLine::Span(_), GridLine::Span(_))) {
                return None;
            }
            Some(GridPlacement { start, end })
        }
        _ => None,
    }
}

fn parse_grid_line(source: &str) -> Option<GridLine> {
    if source == "auto" {
        return Some(GridLine::Auto);
    }
    if let Some(span) = source.strip_prefix("span ") {
        return Some(GridLine::Span(
            span.trim().parse::<u16>().ok().filter(|span| *span > 0)?,
        ));
    }
    Some(GridLine::Index(
        source.parse::<i16>().ok().filter(|index| *index != 0)?,
    ))
}

fn function_args<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn split_top_level(source: &str, separator: char) -> Option<Vec<&str>> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            _ if ch == separator && depth == 0 => {
                result.push(&source[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    result.push(&source[start..]);
    Some(result)
}
