# Dynamic property bindings

TiltUI parses `[property]="expression"` once with the template and evaluates the expression again when binding data changes. The following matrix describes whether the value reaches the existing widget entity. It is an audit of the old HTML attribute surface and the current TiltUI runtime, rather than a promise that every old creation-time option is reactive.

| Binding / attribute | Existing entity behavior | Limits |
| --- | --- | --- |
| `{{ expression }}`, `[text]`, `[innerText]`, `[textContent]` | Updates visible text. | Specialized controls use their existing label part where possible. |
| `[innerHtml]`, `[innerHTML]` | Replaces the target's child fragment. | Child entities are recreated; the target entity stays. |
| `[class]`, `[class.name]` | Combines dynamic classes with authored `class`; individual boolean class bindings can add or suppress a name. CSS selectors refresh. | Invalid/empty class names in `class.name` are ignored. |
| `style="..."`, `[style]` | Parses supported CSS declarations and applies them after component CSS. A dynamic style augments/overrides the static inline style; clearing it restores the static declarations. | 8192 source bytes; unsupported CSS syntax and `!important` have no compatibility behavior yet. |
| `[disabled]`, `[checked]`, `[selected]` | Updates control state and pseudo-class projection. Option selection goes through the choice/list selection logic, including single-select exclusivity. | Unsupported on elements without matching control state. |
| `[value]` | Updates editable text, numeric controls, badge, option value, date picker (ISO date), or color picker (supported CSS color syntax). | Invalid date/color/numeric values leave the widget unchanged. |
| `[readonly]`, `[required]`, `[minlength]`, `[maxlength]`, `[max-lines]`, `[name]` | Changes existing editable state/options and recomputes `:invalid`; `name` participates in form data. | Applies to editable controls only. |
| `[min]`, `[max]`, `[step]`, `[range-start]`, `[range-end]` | Reconfigures existing numeric/range widgets and refreshes persistent fill/thumb geometry. | Range endpoints apply to two-thumb sliders; invalid/nonfinite numbers are ignored. |
| `[src]`, `[alt]` | Updates image metadata/source. Avatar source changes also update fallback visibility; avatar `alt` changes its initials. | Plain image `alt` remains metadata; visible missing-image fallback is tracked separately in `TODO.md`. |
| `[placeholder]`, `[href]`, `[title]` | Updates editable placeholder, hyperlink destination, and dialog title respectively. | `title` is not a universal browser tooltip. |
| `[open]` | Opens/closes choice boxes, date pickers, and color pickers. | Dialog and tooltip opening retain their dedicated APIs/trigger behavior. |

The following options are read when the widget is created and are **not** reactive property bindings: input `type`, file input `folder`/`extensions`/`max-size`, slider `orientation`/`dots`/`show-tip`/`show-labels`, tooltip `for`/`trigger`/placement, form `action`/`validate`, and dialog renderer/layout. Changing these would alter widget anatomy or control registration, so a future implementation needs an explicit reconfiguration path. Unknown property names are ignored; they do not mutate arbitrary ECS components or silently rebuild a widget.

The inline CSS path expects a CSS declaration string and uses the same typed parser as component styles. For example, `<button class="base" [class.active]="state.enabled" style="width: 40px" [style]="state.extra_css">` keeps its base class and width while applying the reactive additions. TiltUI parses when the style string changes, then stores the declarations on the element; unrelated store revisions do not parse it again. Inline declarations win over author styles in the current cascade, while `!important` priority remains a separate CSS parity task.
