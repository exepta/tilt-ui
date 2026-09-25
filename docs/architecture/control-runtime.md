# Control Runtime

TiltUI controls separate native interaction state from CSS selector state.

```text
Interaction / InputFocus / InteractionDisabled / Pressed
    ↓
TiltUiControlRuntimePlugin
    ↓
ElementState
    ↓
:hover / :active / :focus / :disabled
    ↓
existing scoped CSS runtime
```

`ElementState` is exclusively the CSS-facing projection. Native Bevy UI
components remain the low-level source for pointer interaction, focus,
disabled state, and held keyboard state. `ControlActivated` is TiltUI's
semantic action boundary; it intentionally does not evaluate authored event
expressions.

Selectable controls use the same pipeline with one additional semantic value:

```text
ControlActivated
    ↓
ControlChecked
    ↓
Checked / ElementState.checked
    ↓
ControlCheckedChanged
    ↓
:checked
```

`ControlChecked` is the authoritative boolean value for `Checkbox`,
`ToggleButton`, `SwitchButton`, and `RadioButton`. Bevy's marker `Checked` is
derived from that value for native UI and accessibility integration, while
`ElementState.checked` is the CSS selector projection. `ControlCheckedChanged`
is emitted only for a user activation that actually changes the value.
`set_control_checked` provides a programmatic write path and intentionally does
not emit that user-facing notification, preventing a future binding runtime
from creating feedback loops.

Static `checked` is an initialization input only. Materialization creates the
initial `ControlChecked` value from the template attribute; later activation
and programmatic writes never reread static attributes. This allows a control
declared checked to be unchecked at runtime without being reset on a later
schedule pass.

The first native control is `TiltButton`. Materialization adds `TiltControl`,
`TiltButton`, Bevy's UI `Button`, and `TabIndex(0)` to the existing template
element. Template children remain untouched. Native tab navigation is provided
by Bevy's `TabNavigationPlugin`; top-level component boundaries receive a
`TabGroup` and nested components participate in that page-level group.

Pointer interaction projects changed `Interaction` values only. Focus is
projected from Bevy `FocusGained` and `FocusLost` entity events. Enter emits
an activation for the focused enabled button. Space marks it active on press,
then clears it and activates on release. Disabled controls use
`InteractionDisabled`, project `:disabled`, and reject activation. Focus is
not the same as focus visibility; `:focus-visible` is intentionally not yet
implemented.

Checkboxes, toggle buttons, and switches invert `ControlChecked` on a valid
activation. Radio buttons select themselves without clearing on repeat
activation. Their exclusive group is the nearest `field-set` ancestor; when no
field set exists, direct siblings share a fallback group. This is the ECS
hierarchy realization of the core `FieldKind::Radio` / `FieldMode::Single`
semantics. A materialized `field-set` receives `FieldSetSelection`, defaulting
to radio/single; static `kind` or `field-kind`, and `mode` or `field-mode`,
may select the currently supported radio/single or non-exclusive mode. Optional
deselection (`allow_none`) is deferred.

`ControlPart` identifies persistent structural entities owned by a control.
Checkboxes and radio buttons create an `Indicator` containing a `Mark`;
switches create a `Track` containing a `Thumb`. These parts are never created
or removed when checked state changes, and visual-only parts use
`Pickable::IGNORE` so they cannot consume control activation.

Pointer activation resolves the picked entity through `ChildOf` ancestors to
the nearest `TiltControl`. Consequently template text, images, and generated
parts activate their owning control while nested controls cannot activate an
outer control from the same gesture. Shared keyboard activation writes the
same `ControlActivated` message, so selection logic is identical for pointer,
Enter, and Space activation.

The component runtime enables Bevy's `ui_picking` feature. Bevy's UI picking
backend produces the `Pointer<Click>` messages consumed by the shared
activation system; UI `Interaction` alone only provides hover and pressed
state and is not an activation event source.

The initial internal-part CSS API is available through these pseudo-elements:

```css
checkbox::indicator
checkbox::mark
radio-button::indicator
radio-button::mark
switch-button::track
switch-button::thumb
```

Pseudo-class state before a pseudo-element is evaluated on its owning control,
so `switch-button:checked::thumb` styles the checked switch thumb. The part
selector surface is intentionally small and may expand with later control
families.

No `bevy_ui_widgets`, Feathers, generated button labels, widget registries, or
control-specific pointer/keyboard systems are used. The runtime relies on
Bevy's global pointer event stream, resolves it to a `TiltControl`, and then
dispatches selection behavior by semantic markers. Hover, focus, disabled
handling, keyboard activation, and CSS invalidation remain shared.
