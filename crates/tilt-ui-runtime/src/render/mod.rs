//! Creation-time Bevy UI materialization for built-in TiltUI elements.

mod classification;
pub(crate) mod image;
mod materialize;

#[cfg(feature = "tilt-icons")]
pub use image::set_icon_size;
pub(crate) use materialize::{materialize_element, materialize_text};
