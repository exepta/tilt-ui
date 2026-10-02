# TiltUI

**TiltUI is a component-based HTML and CSS UI framework for Bevy.** The `tilt-ui` crate is the main dependency for applications. It combines the component model, Bevy runtime, widgets, routing, and public macros. `tilt-ui-build` is its companion build dependency for discovering UI files.

TiltUI is in early development. Its HTML and CSS are rendered by Bevy; they are not a browser DOM, and support follows the features implemented by TiltUI.

## Version compatibility

| TiltUI line | Bevy line | Notes |
| --- | --- | --- |
| `0.1.0-rc.*` and `0.1.0` | [`0.19.x`](https://bevy.org/news/bevy-0-19/) | Current compatibility target; this repository tests with Bevy `0.19.1`. |

Use the **same TiltUI version** for `tilt-ui` and `tilt-ui-build`. Release candidates need their full version, including `-rc.N`. Other Bevy lines are not declared compatible with this TiltUI line. Check the [workspace manifest](https://github.com/exepta/tilt-ui/blob/main/Cargo.toml) for the current dependency target.

## Start a project

You need Rust 1.85 or newer, Bevy 0.19, and a project with `src/` and `src-ui/`. While working from the current Git repository, the dependencies can be:

```toml
[dependencies]
bevy = "0.19"
tilt-ui = { git = "https://github.com/exepta/tilt-ui" }

[build-dependencies]
tilt-ui-build = { git = "https://github.com/exepta/tilt-ui" }
```

For a completed crates.io release, replace both Git dependencies with the **same published TiltUI version**. The two dependencies should also use the same Git revision if you pin one. Bevy's default features are the easiest way to start; if you disable them, include windowing, rendering, UI rendering, and font support.

Create these files:

```text
my-app/
├── Cargo.toml
├── build.rs
├── src/
│   └── main.rs
└── src-ui/
    ├── index.html
    ├── app-main.component.css
    ├── app-main.component.html
    └── app-main.component.rs
```

`build.rs` discovers the document and components:

```rust
fn main() {
    tilt_ui_build::build().expect("TiltUI component discovery failed");
}
```

`src-ui/index.html` is the entry document. Mount the component by its file name:

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>My TiltUI app</title>
</head>
<body>
  <app-main></app-main>
</body>
</html>
```

Put the component's markup in `src-ui/app-main.component.html`:

```html
<div class="card">
  <h1>Hello from TiltUI</h1>
  <p>This component is rendered inside Bevy.</p>
  <button onclick="greet">Click me</button>
</div>
```

Style it in `src-ui/app-main.component.css`:

```css
.card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 24px;
    background-color: #183453;
    border-radius: 16px;
}
.card h1, .card p { color: #ffffff; }
```

Handle the button in `src-ui/app-main.component.rs`:

```rust
use bevy::prelude::*;
use tilt_ui::{HtmlEvent, html_fn};

#[html_fn("greet")]
fn greet(In(_event): In<HtmlEvent>) {
    info!("Hello from TiltUI");
}
```

Finally, include the generated catalog in `src/main.rs`. Add `TiltUiPlugin` **before** Bevy's `DefaultPlugins` so its asset source is registered in time:

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

Run the app with `cargo run`. For a complete project already in this repository, run `cargo run -p tilt-ui-example-basic` from the workspace root.

## Work with components and styles

A component can live anywhere under `src-ui/`. Its `name.component.html`, `name.component.css`, and `name.component.rs` files form one unit; use `<name></name>` in the entry document or another component. `pages/` and `components/` are conventions, not required directory names. The build step finds Rust component files and generates the catalog consumed by `include_components!()`.

Put document-wide CSS in a file linked from the `<head>` of `src-ui/index.html`:

```html
<link rel="stylesheet" href="styles/site.css">
```

Keep component-specific CSS beside its component. TiltUI supports layout tools such as flex, grid, and block, plus colors, borders, backgrounds, filters, and other implemented CSS properties. See the [CSS showcase](https://github.com/exepta/tilt-ui/tree/main/examples/css-showcase) for actual supported syntax.

Use template expressions such as `{{ state.name }}`, `@if`, and tracked `@for` for dynamic content. Rust handlers annotated with `#[html_fn("name")]` can respond to template events such as `onclick="name"`. Shared resources and writable stores can bind Rust state to the template; the [basic example](https://github.com/exepta/tilt-ui/tree/main/examples/basic) demonstrates both.

Image and other UI assets are resolved from the configured `src-ui` root. Use a `tilt-ui://` source when you need an explicit TiltUI asset path. For SVG images, see the [WASM showcase](https://github.com/exepta/tilt-ui/tree/main/examples/wasm-showcase).

## Add routing

Routing is optional. Put a `<router-outlet></router-outlet>` in a component and register a `Routes` table. An optional `src-ui/routers.rs` can serve as the central registry. Route modules such as `features/bob.route.rs` can live **anywhere** below `src-ui/`; import them from `routers.rs` and merge their tables.

A plain route creates a new component on each visit. `load!(component)` creates and retains one when the outlet is available; `lazy!(component)` creates it on first visit and then retains it. The [routing showcase](https://github.com/exepta/tilt-ui/tree/main/examples/routing) contains a working registry, navigation, redirects, and fallbacks.

## Widgets and optional features

TiltUI includes controls such as buttons, inputs, text areas, checkboxes, sliders, menus, dialogs, and toasts. Use their HTML tags in component templates, respond to their events in Rust, and style them with CSS. The [component showcase](https://github.com/exepta/tilt-ui/tree/main/examples/component-showcase) and [widget documentation](https://github.com/exepta/tilt-ui/tree/main/docs/widgets) show the available attributes and behavior.

| Feature on `tilt-ui` | Use |
| --- | --- |
| `component` | Component runtime, enabled by default. |
| `tilt-icons` | Built-in SVG icon catalog, for example `<icon name="home" size="32" />`. |
| `fluent` | Fluent `.ftl` localization catalogs. |
| `hot-reload` | Asset file watching during development. |

Enable optional features in `Cargo.toml`, for example `tilt-ui = { git = "https://github.com/exepta/tilt-ui", features = ["tilt-icons", "fluent"] }`. The [icon catalog](https://github.com/exepta/tilt-ui/tree/main/examples/icons-catalog) and [WASM showcase](https://github.com/exepta/tilt-ui/tree/main/examples/wasm-showcase) demonstrate those features.

## Common setup checks

- `src-ui/index.html` must exist, and local stylesheet links must point to real files.
- Use the same source root in `build.rs` and `TiltUiPlugin::with_source_root` if you move `src-ui/`.
- Keep `TiltUiPlugin` before `DefaultPlugins`; `include_components!()` needs the generated build output, so run Cargo's build script.
- Keep `tilt-ui` and `tilt-ui-build` on the same version or Git revision.

For deeper topics, start with the [full project guide](https://github.com/exepta/tilt-ui#readme), the [examples](https://github.com/exepta/tilt-ui/tree/main/examples), and the [runtime architecture notes](https://github.com/exepta/tilt-ui/tree/main/docs/architecture).
