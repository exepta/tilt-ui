//! Creation-time Bevy UI materialization for built-in TiltUI elements.

mod classification;
mod image;
mod materialize;

pub(crate) use materialize::{materialize_element, materialize_text};
