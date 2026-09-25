//! Build-time source-root conventions and component discovery for TiltUI projects.
//!
//! The default source tree is `src-ui/`, beside a consumer project's `src/`
//! directory. Components may pair `.component.rs`, `.component.html`, and
//! `.component.css` files anywhere beneath that tree.

mod component;
mod source;

pub use component::*;
pub use source::*;
