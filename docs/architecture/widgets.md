# Widget Modules

Built-in element-specific definitions live under `tilt-ui-runtime/src/widgets`.
The directory is organized into `structure`, `content`, `controls`, and
`advanced` groups. Each built-in `ElementKind` has a dedicated file, including
architectural homes for controls that are not implemented yet. `Form`,
`FieldSet`, and `Table` now contain runtime behavior rather than marker files.

```text
widgets/   element-specific markers and persistent anatomy
control/   shared focus, pointer, keyboard, disabled, and selection behavior
render/    creation-time Template/Element to Bevy UI materialization
style/     CSS matching, cascade, motion, and Bevy style application
component/ component instances, assets, and ownership
```

Dedicated widget files do not introduce per-widget plugins or update scans.
The checkbox, radio button, and switch files create their persistent visual
parts during materialization. `Input` and `TextArea` share event-driven
editable state, while `Slider`, `Scrollbar`, and `ProgressBar` share normalized
numeric range state and persistent fill anatomy. Shared checked-state mutation
and radio grouping remain in `control/selection.rs`.

`widgets/state.rs` contains the reusable semantic state for editable and
numeric widgets. It does not own input focus, CSS invalidation, or visual
appearance. Those remain in the shared control and style subsystems.
