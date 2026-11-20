# tilt-ui-runtime

`tilt-ui-runtime` connects [TiltUI](https://github.com/exepta/tilt-ui) components, assets, styles, and widgets to Bevy's UI and ECS. It contains the rendering and interaction systems used by the main `tilt-ui` crate.

For an application, start with [`tilt-ui`](https://crates.io/crates/tilt-ui) and `TiltUiPlugin`. Use `tilt-ui-runtime` directly when building a custom integration or working on the runtime itself.

See the [project guide](https://github.com/exepta/tilt-ui#readme) and [examples](https://github.com/exepta/tilt-ui/tree/main/examples) for complete applications.

Retained hidden trees can opt into `ParkedUiBindings` to pause reactivity without
destroying their entities. Restore their bindings before showing them again.
See the [performance guide](../../docs/performance.md#bevy-020-runtime-workloads)
for lifecycle requirements and runtime regression tests.
