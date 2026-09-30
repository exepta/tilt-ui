# Animated dialogs

Use `animated` on an in-window dialog to play entry and exit motion. Omit it, or set `animated="false"`, for immediate opening and closing. Native system dialogs use their platform presentation and ignore this visual option.

```html
<button id="open-settings">Settings</button>
<dialog trigger="open-settings" animated="true" title="Settings">
  <p>Choose your preferences.</p>
  <button dialog-close="confirm">Done</button>
</dialog>
```

Rust-created dialogs use `DialogConfig::info("Title", "Body").with_animated(true)`. `[animated]` can change the mode at runtime. `open_dialog` and `close_dialog` work in both modes. While the exit animation plays, the panel and backdrop remain visible; `DialogClosed` fires once and focus returns to the previous control after they disappear. Reopening during the exit cancels that close. The default panel entry lasts 220 ms, or 300 ms for a bottom sheet; the backdrop appears immediately. The exit lasts 180–240 ms. An immediate dialog closes and returns focus in the same call.

Style `dialog:animated:open` and `dialog:animated:closing` for the backdrop, and the matching `::popup` selectors for the panel. Override the whole `animation` shorthand with your own keyframes and duration; the runtime waits for the longer of the backdrop and panel exit animations before hiding the dialog. Keep exit animations finite. `dialog:open` and `dialog:closing` also work without `:animated` in custom themes.
