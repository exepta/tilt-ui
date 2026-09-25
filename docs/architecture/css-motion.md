# CSS Motion

`@keyframes`, `animation`, animation longhands, `transition`, and `transform`
are parsed by `tilt-ui-css` into typed values. Keyframes remain local to their
own component stylesheet; an author keyframe resolves before a same-named
default-theme keyframe, and never collides with another component instance.

```css
@keyframes pulse {
    from { transform: scale(1); }
    50% { transform: scale(1.05); }
    to { transform: scale(1); }
}

.pulse { animation: pulse 1.2s ease-in-out infinite alternate; }
button { transition: background-color 150ms ease-out; }
```

Supported timing functions are `linear`, `ease`, `ease-in`, `ease-out`, and
`ease-in-out`, evaluated with cubic-Bezier curves. Animation duration and
delay accept `ms` and `s`; iteration count accepts finite non-negative numbers
and `infinite`; directions are `normal`, `reverse`, `alternate`, and
`alternate-reverse`. Multiple animations and transitions are accepted.

Transitions support shorthand plus `transition-property`, `transition-duration`,
`transition-delay`, and `transition-timing-function`. The supported transition
properties are `all`, `color`, `background-color`, `border-color`,
`border-radius`, `font-size`, `opacity`, `width`, `height`, and `transform`.
`transition: none` removes authored transitions immediately. An interrupted
transition starts from the currently displayed interpolated style rather than
the previous target.

The runtime keeps `RuntimeComputedStyle` as the unanimated base result. Active
entities alone receive `ActiveAnimations` and private transition state.
Animation state stores only its specification, elapsed time, and stylesheet
source; keyframes stay in the shared parsed stylesheet asset rather than being
copied per entity.
Transitions are created from actual computed-style changes and restart from
the currently displayed style when interrupted. The final style path is:

```text
computed style -> transition overlay -> animation overlay -> Bevy UI apply
```

Animation and transition ticks never mark a component `StyleDirty` and never
rerun selector matching. Finite animations return to their base computed style
after completion; CSS fill modes are not implemented. Interpolation supports
colors, opacity in the typed overlay, compatible length units, font size, and
typed translate/scale/rotate transforms. `auto` and mixed length units use a
discrete midpoint because no layout context is available for a correct numeric
conversion. Bevy UI still has no correct subtree-opacity mapping, so opacity
is retained but not visually applied.

Motion reconciliation is centralized, so a future reduced-motion preference can
disable or shorten animations and transitions without changing control code.
