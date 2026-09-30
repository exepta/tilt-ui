# Toast

Use `<toast>` for a short notification whose content is written in HTML. It stays outside the scrollable page and joins a viewport-level stack. The trigger is the ID of a button in the same component:

```html
<button id="save-notice">Show notice</button>
<toast type="success" trigger="save-notice" duration="5s" class="save-toast">
    <p>Saved</p>
    <p>Your changes are ready.</p>
</toast>
```

`type` accepts `success`, `error`, `warning`, and `info`. The default duration is five seconds. `duration="750ms"`, `duration="5s"`, and `duration="5000"` are equivalent formats; `duration="0"` or `duration="persistent"` keeps the toast open. `open` displays it on mount. Author any child HTML and style the toast, its descendants, `toast::label` for a `title` attribute, and the built-in `.toast-close` button with CSS. An authored button with `toast-close` also closes its containing toast.

From Rust, send a `ShowToast` message from a system or `#[html_fn]` handler:

```rust
toasts.write(ShowToast {
    parent: click.target,
    config: ToastConfig::success("Saved", "Your changes are ready."),
});
```

`ToastConfig` also offers `error`, `warning`, `info`, `with_duration`, `persistent`, and `with_class`. For direct `World` access, use `spawn_toast`, `show_toast`, and `close_toast`. `ToastSpawned` reports the entity of a Rust-created toast; `ToastClosed` reports manual closure, timeout, or stack overflow. Template toasts are retained for later reopening; Rust-created toasts are removed after closing.

`ToastStackSettings` is a public Bevy resource. Change `placement` to `TopRight`, `TopLeft`, `BottomRight`, or `BottomLeft`; `max_visible`, `width`, `margin`, and `gap` control the stack. The default keeps up to four visible toasts at the upper right. When the stack is full, the oldest toast closes to make room for the new one.
