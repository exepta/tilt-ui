# Basic control showcase

Run from the repository root:

```sh
cargo run -p tilt-ui-example-basic
```

The page exercises reactive `@if`/`@else`, Rust-style `@match`, tracked and range-based `@for`, and lexical `@let` values. Type in the input to test string truthiness, `equals`, `equalsIgnoreCase`, `startWith`, `endsWith`, and `contains`. The buttons toggle a boolean, change a count and match arm, and add, remove, or reverse tracked list items. The list uses `track item.id`, so reversing it retains each item's entity and widget state.

The name inputs exercise `user.full_name()` and `user.greeting('Hello')`. Both are explicitly registered as pure `#[html_method]` functions in `src-ui/pages/main.component.rs`; changing the shared `User` resource refreshes their text bindings. No controller is required.

The "Inline store actions" panel exercises `$set`, `$add`, `$min`, `$mul`, `$div`, `$clamp`, `$toggle`, and `$append`. Its `ActionState` derives `UiStore`, `Serialize`, and `Deserialize`, and opts into template writes with `#[ui_store(mutable)]`. Input, checkbox, and slider changes demonstrate `$event.value` and `$event.checked`; the reset button chains two calls with a semicolon. The Rust value and bound HTML update together.

`src-ui/index.html` mounts the `main` page in a document shell so the scrollable body starts at the top. The linked document stylesheet provides the page background and scrollbar colors. The app targets 60 FPS with `UiFrameRate::Fps60`.

Control expressions use `==`, `!=`, `>`, `<`, `>=`, `<=`, `&&`, and `||` (or the word `oder`). A `@for` accepts `item in items`, `item of items`, optional `(item, index)`, `; track expression`, and integer ranges such as `0..count` or `1..=count`. A match arm accepts a literal/expression, alternatives separated by `|`, and `_` as the fallback. Local variables use `@let name = expression;` and are visible to following siblings and their descendants in the current HTML element or control block.
