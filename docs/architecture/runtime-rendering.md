# Runtime Rendering

TiltUI instantiates a component template directly into a semantic Bevy ECS
hierarchy. Built-in element and text entities receive their minimal Bevy UI
primitive during that same instantiation pass:

```text
Template node
    ↓
TiltUI semantic entity
    ↓
Node, Text, or ImageNode on that entity
```

`ComponentId` identifies a compiled component definition. `NodeId` identifies
a node inside one parsed template. A Bevy `Entity` identifies one concrete
runtime instance. These identifiers have separate scopes and are not
interchangeable.

## Component Boundaries

Each component instance has a `ComponentRoot` ownership boundary. The boundary
uses Bevy 0.19's experimental `GhostNode` internally so it remains transparent
to UI layout while retaining the ECS hierarchy needed for component ownership,
teardown, scoped styling, and lifecycle work.

```text
ComponentRoot + GhostNode
└── Template root + Node
    └── Text node + Text
```

Component boundaries do not receive a `Node` solely for layout. Nested
components use the same layout-neutral boundary structure. `GhostNode` is an
experimental Bevy API and is isolated in the component boundary implementation
so a future Bevy API change has one replacement point. It is not part of the
public TiltUI semantic model.

## Camera Ownership

TiltUI does not create cameras automatically. Applications must provide a Bevy
UI camera because camera, window, and render-target ownership is application
specific.

```rust
fn setup(mut commands: Commands) {
    commands.spawn((Camera2d, IsDefaultUiCamera));
}
```

## CSS Runtime

Component `.component.css` files are discovered, loaded, parsed, matched, and
applied by the component-scoped CSS runtime. See
[CSS Runtime](css-runtime.md) for scope isolation, cascade, inheritance,
invalidation, Bevy mappings, and current limitations.
