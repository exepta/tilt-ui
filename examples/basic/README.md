# Basic control showcase

Run from the repository root:

```sh
cargo run -p tilt-ui-example-basic
```

Use `TILT_UI_BASIC_SIZE=390x844` to inspect the narrow layout. Set `TILT_UI_BASIC_SCREENSHOT=/tmp/tilt-basic.png` to save a window screenshot after startup.

The page exercises reactive `@if`/`@else`, Rust-style `@match`, tracked and range-based `@for`, and lexical `@let` values. Type in the input to test string truthiness, `equals`, `equalsIgnoreCase`, `startWith`, `endsWith`, and `contains`. The buttons toggle a boolean, change a count and match arm, and add, remove, or reverse tracked list items. The list uses `track item.id`, so reversing it retains each item's entity and widget state.

The "UI lifecycle" panel at the top reads the current `UiDocumentState` and the `UiState` components for `main` and `state-probe`. It also shows the latest `UiStateEvent` messages and a total transition count. Click **Hide probe** and **Show probe**: the green component disappears and returns, while its card and event list report `Hidden` and `Visible`. Repeatedly clicking the same button should not increase the count. `track_ui_lifecycle` in `main.component.rs` is registered after `UiStateRuntimeSet::Observe` in `src/main.rs` so it receives transitions in the same update. Loading, Loaded, and Ready appear during startup; asset errors appear in the log if a required source fails to load.

The name inputs exercise `user.full_name()` and `user.greeting('Hello')`. Both are explicitly registered as pure `#[html_method]` functions in `src-ui/pages/main.component.rs`; changing the shared `User` resource refreshes their text bindings. No controller is required.

The "Inline store actions" panel exercises `$set`, `$add`, `$min`, `$mul`, `$div`, `$clamp`, `$toggle`, and `$append`. Its `ActionState` derives `UiStore`, `Serialize`, and `Deserialize`, and opts into template writes with `#[ui_store(mutable)]`. Input, checkbox, and slider changes demonstrate `$event.value` and `$event.checked`; the reset button chains two calls with a semicolon. The Rust value and bound HTML update together.

`src-ui/index.html` mounts the `main` page in a document shell so the scrollable body starts at the top. The linked document stylesheet provides the page background and scrollbar colors. The app targets 60 FPS with `UiFrameRate::Fps60`.

Control expressions use `==`, `!=`, `>`, `<`, `>=`, `<=`, `&&`, and `||` (or the word `oder`). A `@for` accepts `item in items`, `item of items`, optional `(item, index)`, `; track expression`, and integer ranges such as `0..count` or `1..=count`. A match arm accepts a literal/expression, alternatives separated by `|`, and `_` as the fallback. Local variables use `@let name = expression;` and are visible to following siblings and their descendants in the current HTML element or control block.
