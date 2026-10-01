# tilt-ui

`tilt-ui` is the main crate of [TiltUI](https://github.com/exepta/tilt-ui), a component-based HTML and CSS UI framework for Bevy. Start here when adding TiltUI to an application. It brings together the component model, Bevy runtime, widgets, and public macros.

TiltUI discovers component files under `src-ui/` at build time. Add `tilt-ui-build` as a build dependency and put this in `build.rs`:

```rust
fn main() {
    tilt_ui_build::build().expect("TiltUI component discovery failed");
}
```

Register the generated components and add the plugin before Bevy's `DefaultPlugins`:

```rust
use bevy::prelude::*;
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins)
        .run();
}
```

See the [project guide](https://github.com/exepta/tilt-ui#readme) and [examples](https://github.com/exepta/tilt-ui/tree/main/examples) for `src-ui/index.html`, components, styling, routing, and widgets. TiltUI is in early development.
