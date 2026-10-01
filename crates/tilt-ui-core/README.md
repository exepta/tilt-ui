# tilt-ui-core

`tilt-ui-core` provides shared component metadata, template types, and element definitions for [TiltUI](https://github.com/exepta/tilt-ui). The HTML parser, CSS engine, build tooling, and runtime use these types to describe the same UI model.

For a Bevy application, use the main [`tilt-ui`](https://crates.io/crates/tilt-ui) crate, which re-exports the public core model. Depend on `tilt-ui-core` directly when building tooling or integrations that only need TiltUI's shared data types.

See the [project guide](https://github.com/exepta/tilt-ui#readme) for the application-level setup.
