# ProgressBar

`<progress-bar min="0" max="100" value="72" />` represents a bounded
numeric progress value. Static attributes initialize `NumericRange` once.
Values are finite, clamped, and normalized as `(value - min) / (max - min)`;
a zero-span range displays zero fill. `set_progress_value` updates the
semantic value and the same persistent Fill entity without a user event.

Horizontal progress fills left-to-right. `orientation="vertical"` fills
bottom-to-top. The persistent `::track` and `::fill` parts are styled by the
default theme or author CSS. The runtime owns only the percentage geometry;
color, border, radius, and transitions belong to CSS. ProgressBar is a
display-only widget, so it has no drag, keyboard, or change-on-interaction
event. The local legacy ProgressBar exposed only `min`, `max`, and `value`;
vertical orientation is a TiltUI addition.
