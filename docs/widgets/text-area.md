# TextArea

`<textarea>` uses the same Unicode-aware editor, focus, selection, clipboard,
IME, validation, and semantic events as `<input>`. Enter inserts a logical
newline. Up/Down and Home/End navigate multiline content, with Shift selection
and Ctrl/Cmd+Home/End. Bevy text layout wraps visual lines independently of
logical newlines; long unbroken words also wrap at the editor width.
`TextScroll` keeps the caret inside the visible editor. The default theme gives
the editor a fixed height and `overflow-y: auto`, so a vertical scrollbar
appears when wrapped content exceeds the visible area. The scrollbar accepts
wheel input, track clicks, and thumb drags. Dragging the scrollbar does not
extend the text selection; dragging inside the editor still selects text.

Supported attributes include `value`, `placeholder`, `name`, `maxlength`,
`minlength`, `max-lines`, `required`, `readonly`, and `disabled`. `max-lines`
limits logical lines on user edits. A rejected paste or edit leaves the previous
semantic value intact. Programmatic changes use `set_editable_text` without
emitting user-originated events.

The persistent lower-right `::resize-handle` can be dragged in both axes.
`set_text_area_size` is the programmatic size API. Resizing respects pixel
`min-width`, `min-height`, `max-width`, and `max-height` where present; other
unit constraints are not resolved into pixels by the resize operation. The
runtime size is an instance-level layout override, not a stylesheet edit.
The editor root, text, and handle entities remain stable while typing and
resizing. CSS parts: `::value`, `::placeholder`, `::cursor`, `::selection`,
and `::resize-handle`.

```html
<textarea name="note" placeholder="A multiline note" max-lines="10" />
```
