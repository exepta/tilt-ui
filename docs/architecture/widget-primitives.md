# Widget Primitives

TiltUI materializes widget roots and persistent `ControlPart` children in the
existing template pass. Widget-specific files configure semantics; shared
control systems handle focus, disabled state, pointer targeting, and CSS state.
No widget has a later initialization scan or a numeric identity registry.

## Component-local IDs

Each materialized component boundary owns `ComponentElementIds`. Static `id`
attributes are registered while its template is materialized. Lookup is local
to that boundary; nested component internals have their own index. Duplicate
IDs are retained as diagnostics and never replace the first entity.

## Editable text

`Input` and `TextArea` share TiltUI `EditableText` semantic state and Bevy
0.19's low-level `bevy::text::EditableText` editor. The native editor owns
Unicode-aware caret movement, selection, clipboard operations, and text layout;
TiltUI projects completed edits into its semantic value and emits
`EditableTextChanged` or `EditableTextCommitted`. The keyboard adapter reads
events only for `InputFocus`. Pointer presses and drags resolve through the
nearest control ancestor. IME preedit/commit events are forwarded to the
focused editor, with the candidate popup positioned at the caret.

Static `value` initializes the semantic value once. `set_editable_text` changes
it programmatically without emitting a user-originated event. `readonly`
preserves focus, navigation, selection, and copy while blocking mutations;
`disabled` blocks interaction. Validation projects to `ElementState.invalid`,
then follows targeted CSS invalidation. Password text remains unmasked in
semantic state; its visible value part uses a mask. File inputs use an optional
native/browser dialog backend and emit `FileInputSelected`.

Both editors have persistent `::value`, `::placeholder`, `::selection`, and
`::cursor` parts. The native editor renders ordinary text and caret/selection;
part CSS is projected onto the native cursor and selection colors. `TextArea`
adds a persistent `::resize-handle`. Its user size is stored as
`WidgetLayoutOverride` and applied after authored CSS without modifying the
stylesheet. Bevy's `TextScroll` handles caret-driven internal scrolling.
Visual wrapping uses Bevy's word-or-character line breaking, so unbroken text
does not extend beyond the editor border. A monospace font is the default for
password inputs: the hidden native text and visible mask then share caret
advance widths.

## Overflow scrolling

`overflow`, `overflow-x`, and `overflow-y` are typed CSS declarations. `auto`
and `scroll` use Bevy UI overflow clipping and `ScrollPosition` for ordinary
elements such as `body` and `div`; native editors retain Bevy `TextScroll`.
One shared wheel path uses Bevy's picked scroll event and UI stack to choose
the innermost visible scroll container at the pointer. It walks ancestors, passing
unused movement from an exhausted inner container to its parent. Persistent
scrollbar tracks and thumbs are created when CSS first makes an element
scrollable. `auto` hides the visual bar until content exceeds the viewport;
`scroll` keeps it visible. A shared pointer handler supports track seeking
and thumb dragging. Scrollbars are styled by `::scrollbar-y-track`,
`::scrollbar-y-thumb`, `::scrollbar-x-track`, and `::scrollbar-x-thumb`.
An active scrollbar pointer stays captured until release, so its drag cannot
become an editable-text selection when the pointer crosses the text area.
The current bars overlay content rather than reserving layout space.
For ordinary UI nodes the scroll range matches Bevy's layout clamp:
`content_size - node_size + scrollbar_size`. Editable text instead uses the
native text layout size and content-box viewport. Scrollbar geometry is checked
after layout because Bevy updates `ComputedNode` geometry without ordinary
change detection. A user-driven text scroll is reapplied after Bevy's
caret-visibility pass; the temporary request is cleared by the next text or
cursor edit, returning scroll ownership to the native editor.

## Numeric range and drag

`NumericRange` centralizes finite bounds, clamping, step snapping, and
zero-safe normalization. `Slider` and `ProgressBar` both use it. Persistent
`::track`, `::fill`, and slider `::thumb` parts carry layout-derived percentage
overrides. These survive CSS restyling; appearance remains CSS-owned.
`set_slider_value`, `set_slider_values`, and `set_progress_value` update the
existing entities without user events.

The shared `ActiveControlDrag` component exists only during a slider or
TextArea-resize gesture. Slider track press and drag map pointer coordinates
through the computed track geometry, then emit `SliderChanged`; gesture end
emits `SliderCommitted`. A disabled control never starts a drag. The same
numeric helper drives keyboard changes and programmatic setters.

## Scope and performance

Internal parts retain the owning component style scope and are not template
elements. Normal text parts ignore picking; the TextArea resize handle is an
intentional pointer target. Typing, value changes, and dragging do not reparse
CSS, rematerialize widgets, or scan all controls. Style invalidation occurs
only when a projected CSS pseudo-state changes.
