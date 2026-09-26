# CSS Runtime

TiltUI component styles are parsed once into a `UiStyleSheetAsset`. The runtime
does not reparse CSS while applying it.

```text
UiStyleSheetAsset
    ↓
dirty component scope
    ↓
Servo selector matching
    ↓
cascade origin, specificity, and source order
    ↓
RuntimeComputedStyle
    ↓
Bevy UI components
```

Each materialized `TiltElement` and `TiltText` carries a
`ComponentStyleOwner(Entity)`. The entity is the concrete component boundary,
not a compile-time `ComponentId`, so separate instances remain isolated. A
nested component receives a new owner for its internal template entities.
The selector adapter builds a short-lived immutable view once for a dirty
scope. Its ancestor and sibling traversal contains only elements belonging to
that scope, which prevents selectors from entering or leaving nested component
internals. Component hosts are not selector subjects in this milestone;
`:host`, `:host-context`, and `::ng-deep` are unsupported.

Selector scope and inheritance intentionally differ. `color`, `font-size`,
`font-weight`, and `text-align` inherit through the ECS visual hierarchy,
including layout-transparent `GhostNode` component boundaries. Layout and
paint properties do not inherit.

The runtime recognizes `:hover`, `:active`, `:focus`, `:disabled`, and
`:checked` via the small `ElementState` component. The shared control runtime
projects native interaction and focus state into this selector-only component.
Static `disabled` and `checked` attributes initialize their corresponding
values during materialization. Missing state is treated as all false.

The built-in default theme is one CSS source parsed once at style-plugin setup.
The cascade order is `DefaultTheme`, selected named theme, provider CSS from
outer to inner scopes, then component-authored `Author` declarations. Origin
precedence is resolved before specificity, source order, and declaration order,
so authored CSS overrides a theme even when its selector is less specific.
Applications can opt out of the bundled defaults with
`TiltUiStyleRuntimePlugin::default().with_default_theme(false)`.

Generated control parts are selector pseudo-elements rather than ordinary
template elements. Existing parts include `::indicator`, `::mark`, `::track`,
and `::thumb`; pseudo-classes such as `:checked` are read from the owning
control. Scrollable elements additionally expose `::scrollbar-y-track`,
`::scrollbar-y-thumb`, `::scrollbar-x-track`, and `::scrollbar-x-thumb`.
Part entities share their owner's component style scope.

New component instances are marked `StyleDirty`. When an author stylesheet is
still loading, available default-theme rules are applied once and the scope is
recorded as pending author CSS. A small readiness query marks only pending
scopes dirty when their stylesheet becomes available; selector matching does
not repeat every frame while loading. Changed selector metadata
(`ElementState`, classes, IDs, or static attributes) marks just its owning
scope dirty. CSS owns supported Bevy style fields on TiltUI materialized
entities; external writes to those fields can be replaced on the next dirty
style pass.

Supported layout mappings update Bevy 0.19 `Node`: display, position, typed
`overflow`/`overflow-x`/`overflow-y` (`visible`, `hidden`, `clip`, `auto`,
`scroll`), box
sizes and offsets, margin, padding, gaps, flex settings, border widths, and
border radii. Colors map to `BackgroundColor`, `BorderColor`, and `TextColor`.
Text formatting maps to `TextFont` and `TextLayout`. Bevy 0.19 requires
`BackgroundColor` and `BorderColor` on every `Node`; when CSS stops owning a
color the runtime restores Bevy's transparent default rather than removing a
required component.

`display: grid` is accepted and tables derive their own Bevy grid tracks and
cell positions. Other authored grid properties are not yet parsed.

`opacity` is retained in `RuntimeComputedStyle`, but is not visually applied:
Bevy UI has no native equivalent for CSS subtree opacity. `!important` is not
modeled by the parser and is unsupported. Attribute selectors work against
static attributes plus `id` and `class`. Simple store/resource property paths
and text interpolation run through the binding runtime. With the `hot-reload`
feature, modified stylesheet assets mark only their owning component scopes
dirty. Host styling and CSS subtree opacity remain outside the current runtime.

Responsive media handling and motion overlays are documented in
[`css-responsive.md`](css-responsive.md) and [`css-motion.md`](css-motion.md).
