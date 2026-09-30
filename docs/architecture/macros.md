# Macro Boundaries

`tilt-ui-macros` implements the Rust-side registration macros re-exported by
the `tilt-ui` facade:

- `#[component_init]` registers an ordinary Bevy `Startup` system.
- `#[component_update]` registers an ordinary Bevy `Update` system.
- `#[html_fn("name")]` registers an `In<HtmlEvent>` handler. `In<HtmlClick>`,
  `In<HtmlChange>`, and `In<HtmlSubmit>` are also accepted.
- `#[html_method("component", "root.path.method")]` registers a pure method
  that can be called from that component's template expression.
- `#[derive(UiStore)]` registers a `Default + Serialize` typed binding store.
  Add `#[ui_store(mutable)]` and `Deserialize` to allow inline event actions
  to update it while keeping the Rust value and binding snapshot synchronized.
- `#[html_shared]` and `#[html_use]` expose a serializable Bevy resource under
  its type name and lowercase initial alias.
- `#[ui_routes]` registers a function returning `Routes`.
- `#[ui_component]` and `#[ui_registry]` are optional compatibility markers.
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
    info!(?event.form_data, "form submitted");
}
```

`<form action="save"><input name="title" /><button type="submit">Save</button></form>`
dispatches to that handler after submit validation succeeds.
`HtmlSubmit.form_data` is a `FormData` map from field names to ordered
`Vec<FormValue>` entries. `FormValue::Text` carries text and selected options;
`FormValue::File` carries filename, optional size and optional native path.
The older `event.data` map still exposes the first value per name as a string.
Use `validate="always|interact|send"` to show `:invalid` immediately, after
field interaction or after a failed submit. `required`, `minlength`,
`maxlength`, `pattern`, `min`, `max` and `step` participate in validation.
For example, `<input type="number" show-fields="true" min="0" step="1" />`
shows increment/decrement buttons; typing also accepts arithmetic with
`+`, `-`, `*`, `/` and `%`, evaluated on commit.

Static `onclick`
and `onchange`, plus `(click)` and `(change)` bindings, dispatch through the
same registry. Property and text bindings resolve JSON paths, literals,
arithmetic, comparisons, boolean operators, array indexing, and ternaries;
for example `[disabled]="profile.busy || profile.items.length == 0"` and
`{{ profile.count + 1 }}`. A template can import a registered shared resource
with `@use "ProfileState" as profile;` or its fields with `as *`.
String methods such as `equals`, `equalsIgnoreCase`, `startWith`, `endsWith`,
and `contains` are built in. For a component method, place an explicit bridge
in its `.component.rs` file:

```rust
use tilt_ui::{html_method, serde_json::Value};

#[html_method("profile", "user.full_name")]
fn full_name(user: &Value, arguments: &[Value]) -> Option<Value> {
    if !arguments.is_empty() { return None; }
    Some(Value::String(format!(
        "{} {}",
        user.get("first_name")?.as_str()?,
        user.get("last_name")?.as_str()?,
    )))
}
```

The template can then use `{{ user.full_name() }}`. The receiver comes from a
registered store, shared resource, or local value. Registration is scoped to
the component name and exact receiver path, so an unknown or mismatched call
does not run. Methods receive only serialized values and arguments, not the
Bevy `World`; keep them pure so binding updates follow their inputs. Calls
accept at most eight arguments. Expressions are limited to 4096 source bytes,
256 tokens, and 16 nested expressions. An invalid call leaves a text binding
empty or its existing property unchanged. Arbitrary Rust/JavaScript calls and
legacy controller lookup are not supported.

Writable stores accept small action scripts in `onclick`, `onchange`, and the
other event attributes. For example:

```rust
#[derive(tilt_ui::UiStore, serde::Serialize, serde::Deserialize)]
#[ui_store(mutable)]
struct Counter { value: i32, enabled: bool }

impl Default for Counter {
    fn default() -> Self { Self { value: 0, enabled: true } }
}
```

```html
<button onclick="$add(counter.value, 1); $toggle(counter.enabled)">Update</button>
<checkbox onchange="$set(counter.enabled, $event.checked)">Enabled</checkbox>
```

`$set(target, value)` assigns an expression; `$add` and `$min` add and
subtract numbers (`$sub` aliases `$min`). `$mul`, `$div`, and `$clamp` handle
numeric values, `$toggle` flips a boolean, and `$append` concatenates a string
or pushes one value onto an array. `$event` is shorthand for `$event.value`;
other event fields include `checked`, `selected`, `text`, pointer coordinates,
and color channels where the corresponding control provides them. Targets may
use fields and numeric array indices. Calls run from left to right, and a
registered Rust handler may appear in the same semicolon-separated script.
Invalid or read-only writes are skipped with a warning. Scripts are limited to
8192 bytes and 32 calls; only registered actions and handlers can execute.

`tilt-ui` retains `include_components!` as a declarative `macro_rules!`
facade helper. It includes the build-generated component manifest into the
consumer crate, where its `#[path]` declarations continue to reference the
original `.component.rs` source files. Keeping this helper in the facade avoids
turning normal module wiring into procedural source generation and preserves
Cargo and rust-analyzer visibility.

`hot-reload` enables Bevy's file watcher. A changed stylesheet restyles its
instances; a changed template rebuilds its descendants and resets their local
widget state. The runnable example is `cargo run -p tilt-ui-example-hot-reload`.
