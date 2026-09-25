# Slider

`<slider>` supports `min`, `max`, `value`, `step`, and
`orientation="horizontal|vertical"` (`alignment` is accepted as an alias).
The vertical direction increases bottom-to-top. A track press seeks to the
nearest stepped value; dragging a thumb updates the value continuously.
Keyboard arrows change by `step`, Home/End go to bounds, and PageUp/PageDown
change by ten steps. Disabled sliders cannot seek, drag, or change by keyboard.

`type="range"` creates two stable thumbs. `range-start` and `range-end`
initialize the lower and upper endpoints; neither thumb crosses the other.
`set_slider_value` changes a single slider, while `set_slider_values` changes
range endpoints. Programmatic setters emit no user-originated event.
`SliderChanged` is emitted for live user changes and `SliderCommitted` when a
pointer gesture ends; both carry an optional upper endpoint. A single slider
also emits the generic `NumericValueChanged` message.

Optional `dots="N"`, `show-labels`, `show-tip`, and
`dot-anchor="top|bottom"` create persistent parts. `N` denotes dot intervals
and is capped at 100 to bound entity count. The two endpoint labels and value
tip are noninteractive text parts, not a separate popup engine. The range fill
itself is not currently draggable as a whole; either thumb can be moved.

CSS parts are `::track`, `::fill`, `::thumb`, `::dot`, `::label`, and
`::tooltip`. Two range thumbs share the `::thumb` styling contract. Their
position and fill length are semantic layout overrides; theme and author CSS
own their appearance.

```html
<slider min="0" max="100" value="64" step="1" show-tip />
<slider min="0" max="100" value="35" step="5" dots="5" show-labels="true" dot-anchor="bottom" />
<slider type="range" range-start="20" range-end="80" dots="10" />
```
