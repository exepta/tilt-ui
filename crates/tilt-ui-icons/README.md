# tilt-ui-icons

`tilt-ui-icons` is the SVG icon catalog for [TiltUI](https://github.com/exepta/tilt-ui). It can be used directly to obtain SVG markup, or through the main `tilt-ui` crate by enabling its optional `tilt-icons` feature. That feature is disabled by default.

```rust
use tilt_ui_icons::{Icon, IconSize};

let svg = Icon::Home.svg(IconSize::Px32);
```

In TiltUI templates, use `<icon name="home" size="32" />`. The catalog supports 16, 32, and 64 pixel output. See the [icon catalog documentation](https://github.com/exepta/tilt-ui/blob/main/docs/widgets/icons.md) and [interactive showcase](https://github.com/exepta/tilt-ui/tree/main/examples/icons-catalog).
