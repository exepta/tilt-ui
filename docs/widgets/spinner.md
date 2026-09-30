# Spinner

`<spinner />` is a standalone loading indicator. It has no button or form behavior and keeps one stable UI entity. The default theme draws a rotating ring; it can be used anywhere in a component template.

```html
<div class="loading-row"><spinner /> <p>Loading content</p></div>
```

CSS controls its size, border, colors, opacity, and animation. For example:

```css
.loading-row spinner {
  width: 36px;
  height: 36px;
  border: 4px solid #A833EA44;
  border-top-color: #A833EA;
  animation: tilt-spinner-spin 1s linear infinite;
}
```

Use `display: none` (including through a bound style or class) to hide it when work finishes. The spinning ring is distinct from the generated `button::spinner` part on a loading button.
