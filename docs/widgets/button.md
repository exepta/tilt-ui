# Button file uploads

`<button type="file">` opens the same picker as `<input type="file">`. It accepts `extensions`, `max-size`, `folder`, `name`, `required`, and `disabled`. Native `folder` selection opens a directory picker; the browser dialog backend does not support folder selection.

```html
<button type="file" name="attachment" extensions="[png, jpg, pdf]" max-size="8MB" required>Upload file</button>
<button type="file" folder="true" name="directory">Choose folder</button>
```

After a successful selection, the button retains its authored label. Its entity receives `FileInputSelection`; `FileInputSelected` carries the filename, optional size, and native path where available. `EditableTextChanged` carries the selected filename and optional field name. A named selection contributes a file value to `FormSubmitted.data`; `required` prevents submission until a selection exists. Resetting the form clears the selection. An oversized selection marks the button `:invalid`. A file button never submits its form when clicked.

## Loading buttons

`<button type="loading">` starts in the loading state. It creates a persistent `::spinner` part and ignores pointer and keyboard activation while loading. The button label remains visible. Loading buttons do not submit forms; use a separate submit action when the operation is ready.

```html
<button type="loading" [loading]="app.saving">Save</button>
```

Use a boolean `[loading]` binding or `set_button_loading(world, button_entity, true/false)` from Rust. The setter returns whether the state changed. Style the button with `button:loading` and its spinner with `button::spinner` or `button:loading::spinner`. The default theme draws a small rotating border ring; the spinner animation runs only while loading. The generated part has no pointer handling of its own, and loading does not change the separate `:disabled` state.

For a loading indicator outside a button, use the standalone [`<spinner />`](spinner.md) widget.
