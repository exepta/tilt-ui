•
[x] Provider und Themes: Erweiterbare, layout-neutrale Provider-Registry mit <theme-provider>, lokalem Theme-Scope und benanntem Theme-Wechsel. CSS-Themes koennen zur Laufzeit registriert und umgeschaltet werden.
•
[x] Weitere HTML-Events: Hover-, Fokus-, Scroll-, Tastatur-, Drag-, Touch- und Initialisierungsereignisse werden mit Ereignisdaten an #[html_fn] weitergeleitet und im Showcase demonstriert.
•
[ ] Bildfunktionen: img preview="input-id" für die Vorschau einer gewählten Datei und SVG-Rasterisierung fehlen. Siehe alte Widget-Doku und TiltUI-Bildmodell.
•
[ ] Laufzeit-Inhalt und Cursor: Das alte HtmlInnerContent bietet Setter für innerText/innerHtml/Bindings; außerdem sind benutzerdefinierte Cursor möglich. TiltUI nutzt hier derzeit einfache Text-Bindings und fest zugeordnete System-Cursor. Siehe alter HTML-Inhalt und TiltUI-Cursor.
•
[~] Komponenten-Metadaten: Automatisches Laden funktioniert, aber das alte template_name/template_file/styles-Schema mit mehreren Stylesheets ist noch nicht abgebildet; TiltUI verlangt Dateitripel mit genau einer .component.css. #[ui_component] war übrigens auch im alten Projekt nur ein Marker. Siehe TiltUI-Validierung und alte Komponentendefinition.
•
[~] Bindings und Kamera: Bindings verstehen aktuell Literale und einfache Datenpfade, keine komplexen Ausdrücke oder die alte @use-Direktive. Die Kamera ist konfigurierbar, aber die früheren Plugin-Optionen für Render-Layer und HDR sind nicht direkt übernommen. Siehe Binding-Grenzen und alte Konfiguration.
