# Input

`<input>` supports `type="text|email|password|number|date|file|range"`.
`value`, `placeholder`, `name`, `maxlength`, `minlength`, `required`,
`readonly`, and `disabled` are read at materialization. `value` is never
reapplied after initialization. `type="range"` currently edits numeric text;
use `<slider>` for pointer-based range interaction.

The focused native Bevy editor supports Unicode insertion, Backspace/Delete,
arrows, Home/End, Shift selection, word navigation, and Ctrl/Cmd+A/C/X/V/Z.
Ctrl+Y or Ctrl/Cmd+Shift+Z redoes an edit. Undo history is local to each field
and is cleared by a programmatic value replacement.
Enter emits `EditableTextCommitted`; edits emit `EditableTextChanged`. Both
events carry the entity, optional `name`, and semantic value. Focus loss also
commits. `set_editable_text` and `set_editable_readonly` are programmatic APIs
and do not emit user-originated events.

Password values remain unmasked in `EditableText.value` and render with mask
characters. The default password face is monospace so the native caret and
visible mask have equal character advances; custom password fonts should also
be monospaced. Number input permits intermediate drafts such as `-` and `10.`;
invalid drafts match `:invalid` rather than being rewritten while typing.
Email and ISO `YYYY-MM-DD` date validity are also projected to `:invalid`.
This is lightweight built-in validation, not a custom validation expression
engine. `:readonly`, `:disabled`, `:focus`, and `:hover` use shared control
state. `::value`, `::placeholder`, `::cursor`, and `::selection` are the
available CSS parts.

File inputs support `folder`, comma-separated `extensions`, `show-size`, and
`max-size` as bytes or `KB`/`MB`/`GB` values (base 1024). Native selection uses the optional `file-dialog` feature;
browser selection uses the browser file API and does not expose a filesystem
path. The current browser dialog backend cannot select folders: `folder` logs
a warning and does not open a file picker. A successful selection emits
`FileInputSelected` with filename, optional
size, and a native path only on desktop. The dialog feature is enabled by
default and can be disabled for an application that does not need file input.
An oversized selection is rejected and marks the Input `:invalid`; cancelling
the dialog leaves its value and validity unchanged.
Custom validation callbacks and authored `(input)` expressions are not yet
implemented.

```html
<input name="username" placeholder="Type here" value="TiltUI" maxlength="64" />
<input type="password" placeholder="Password" required />
```
