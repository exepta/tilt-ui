# Macro Boundaries

`tilt-ui-macros` implements the Rust-side registration macros re-exported by
the `tilt-ui` facade:

- `#[component_init]` registers an ordinary Bevy `Startup` system.
- `#[component_update]` registers an ordinary Bevy `Update` system.
- `#[html_fn("name")]` registers an `In<HtmlEvent>` handler. `In<HtmlClick>`,
  `In<HtmlChange>`, and `In<HtmlSubmit>` are also accepted.
- `#[derive(BeuStore)]` registers a `Default + Serialize` typed binding store.
- `#[html_shared]` and `#[html_use]` expose a serializable Bevy resource under
  its type name and lowercase initial alias.
- `#[beu_routes]` registers a function returning `Routes`.
- `#[ui_component]` and `#[beu_registry]` are optional compatibility markers.
  `tilt-ui-build` discovers `.component.rs` sources under the configured source
  root. Without a definition, it uses the matching `.component.html` and
  `.component.css` files. A const or static struct literal can instead provide
  `template_name`, `template_file`, and `styles` (an ordered array of CSS file
  names). The template can use another `.html` name; styles can include
  additional shared CSS files or be empty. No
  component-specific Bevy plugin is needed.

For example:

```rust
use bevy::prelude::*;
use tilt_ui::{HtmlSubmit, component_init, component_update, html_fn};

#[component_init]
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[component_update]
fn refresh_ui(time: Res<Time>) {
    let _elapsed = time.elapsed_secs();
}

#[html_fn("save")]
fn save(In(event): In<HtmlSubmit>) {
    info!(?event.data, "form submitted");
}
```

`<form action="save"><input name="title" /><button type="submit">Save</button></form>`
dispatches to that handler after submit validation succeeds. Static `onclick`
and `onchange`, plus `(click)` and `(change)` bindings, dispatch through the
same registry. Property and text bindings resolve JSON paths, literals,
arithmetic, comparisons, boolean operators, array indexing, and ternaries;
for example `[disabled]="profile.busy || profile.items.length == 0"` and
`{{ profile.count + 1 }}`. A template can import a registered shared resource
with `@use "ProfileState" as profile;` or its fields with `as *`.
Arbitrary Rust/JavaScript function calls and collection rendering are outside
this expression subset.

`tilt-ui` retains `include_components!` as a declarative `macro_rules!`
facade helper. It includes the build-generated component manifest into the
consumer crate, where its `#[path]` declarations continue to reference the
original `.component.rs` source files. Keeping this helper in the facade avoids
turning normal module wiring into procedural source generation and preserves
Cargo and rust-analyzer visibility.

`hot-reload` enables Bevy's file watcher. A changed stylesheet restyles its
instances; a changed template rebuilds its descendants and resets their local
widget state. The runnable example is `cargo run -p tilt-ui-example-hot-reload`.
