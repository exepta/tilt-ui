# TiltUI

A component-based HTML and CSS UI framework for Bevy.

Developed by tilt-us.

Status: Early development.

## Getting started

Only `src-ui/` is required. Place component triplets (`name.component.html`,
`name.component.css`, `name.component.rs`) directly in that directory or in any
subdirectory. A top-level `pages/` directory is recognized as pages for
compatibility, but neither `pages/` nor `components/` is created or required.

In `build.rs`, discover the source root:

```rust
fn main() {
    tilt_ui_build::build().expect("TiltUI component discovery failed");
}
```

Install the asset source, runtime, controls, and UI camera with one plugin.
It must precede `DefaultPlugins` so the `tilt-ui://` source is registered before
Bevy creates its asset server:

```rust
use bevy::prelude::*;
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(TiltUiPlugin::new(tilt_ui_component_catalog())
            .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")))
        .add_plugins(DefaultPlugins)
        .run();
}
```

For another source directory, use the same absolute path in the plugin and
in `build.rs` via `tilt_ui_build::build_from(&UiSourceRoot::new(path))`.
`TiltUiPlugin::with_camera(camera)` customizes the automatically spawned
`Camera2d`; `without_camera()` leaves camera creation to the application.
Mark a manually created camera with Bevy's `IsDefaultUiCamera`.

The [component showcase](examples/component-showcase/src-ui/pages/showcase.component.rs)
demonstrates `#[component_init]`, `#[component_update]`, `#[html_shared]`,
and `#[html_fn]`, including a Rust-driven animated progress bar. These
functions register automatically from the discovered component source; `main.rs`
does not register a component-specific plugin. Run it with
`cargo run -p tilt-ui-example-component-showcase`.

## Text interaction

Input and TextArea characters enter with a short, damped motion by default.
Set `UiMotionSettings.input_text_seconds` to `0.0` to disable it globally, or
use `text-animation="none"` on one field. Paragraphs, H1-H6 headings, labels,
and text directly inside noninteractive containers support drag selection and
Ctrl/Cmd+C. The selection follows wrapped lines and Unicode text.

Right-clicking an Input or TextArea opens the default `ContextMenu` with Copy
(when nonempty), Paste, and Clear as applicable. Right-clicking selected static
text offers Copy. You can also attach your own menu to an element ID:

```html
<button id="item-actions">More actions</button>
<context-menu for="item-actions">
    <button onclick="reset_progress">Reset progress</button>
</context-menu>
```

## Dialogs

Use `<dialog>` for an in-window modal. `trigger` names an element ID in the
same component; `layout="bottom-sheet"` anchors its panel to the bottom.
The dialog closes on Escape, a backdrop click, the built-in close button, or
an authored button with `dialog-close="confirm|cancel|close"`:

```html
<button id="open-settings">Open settings</button>
<dialog trigger="open-settings" title="Settings" layout="bottom-sheet">
    <p>Apply these changes?</p>
    <button dialog-close="cancel">Cancel</button>
    <button dialog-close="confirm">Apply</button>
</dialog>
```

`renderer="system"` opens a native message dialog instead. `type` accepts
`info`, `warning`, `error`, `question`, or `blank`; native dialogs use the
platform's message buttons, while Bevy dialogs retain their authored content.
The native renderer uses the runtime's default `file-dialog` feature; without
it, `renderer="system"` falls back to the in-window modal. Rust code can call
`open_dialog(world, entity)` and `close_dialog(world, entity, result)` and read
`DialogClosed` messages for the result.

To create a dialog entirely from a component's Rust logic, send `ShowDialog`
with a materialized element as `parent`:

```rust
#[html_fn("confirm_action")]
fn confirm_action(
    In(click): In<HtmlClick>,
    mut dialogs: MessageWriter<ShowDialog>,
) {
    dialogs.write(ShowDialog {
        parent: click.target,
        config: DialogConfig::question("Continue?", "Apply the changes now?"),
    });
}
```

`DialogSpawned` reports the new entity. Rust-created dialogs are removed when
closed; template-authored dialogs remain available for the next opening.
