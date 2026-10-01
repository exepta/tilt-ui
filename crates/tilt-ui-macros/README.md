# tilt-ui-macros

`tilt-ui-macros` implements the procedural macros used by [TiltUI](https://github.com/exepta/tilt-ui), including component, routing, and HTML handler annotations.

Application code should normally depend on the main [`tilt-ui`](https://crates.io/crates/tilt-ui) crate, which re-exports these macros alongside the types they need. A direct dependency on `tilt-ui-macros` is mainly useful for tooling that integrates with TiltUI's macro layer.

See the [project guide](https://github.com/exepta/tilt-ui#readme) and [routing showcase](https://github.com/exepta/tilt-ui/tree/main/examples/routing) for usage in an application.
