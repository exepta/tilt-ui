mod attribute;
mod id;
mod name;
mod node;
// The module name mirrors the public `Template` type and the prescribed file layout.
#[allow(clippy::module_inception)]
mod template;

pub use attribute::*;
pub use id::*;
pub use name::*;
pub use node::*;
pub use template::*;
