//! `tilt-icon:name@size` URL parsing.

use crate::{Icon, IconSize};

/// Parses an icon URL used by TiltUI's image and CSS background loaders.
pub fn parse_source(source: &str) -> Option<(Icon, IconSize)> {
    let value = source
        .trim()
        .strip_prefix("tilt-icon:")?
        .trim_start_matches("//");
    let (name, size) = value.rsplit_once('@')?;
    Some((
        Icon::from_name(name)?,
        IconSize::from_pixels(size.parse().ok()?)?,
    ))
}
