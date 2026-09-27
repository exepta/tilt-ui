# TiltUI – Parität mit `bevy_extended_ui`

Vergleichsbasis: Quellcode von `bevy_extended_ui` **1.6.0** und der aktuelle TiltUI-Stand. Die Liste beschreibt beobachtbare Unterschiede, keine Zusage, jede alte API unverändert zu übernehmen. Der alte `UiRegistry`-Pfad ist bei aktiviertem `extended-framework` deaktiviert und steht deshalb separat. Erledigte Punkte stehen am Ende.

## Templates, Komponenten und Bindings

- [ ] **HTML5-Dokumente und Fehlerkorrektur:** Der alte `kuchiki`-Parser verarbeitet ein vollständiges `index.html` mit `html`, `head` und `body` und korrigiert typische Verschachtelungsfehler. TiltUI parst Komponenten mit `quick_xml`, verlangt passende End-Tags und glättet Tabellen-Wrapper. Dokument- und Fragmentverhalten für vorhandene HTML-Quellen ergänzen.
- [ ] **Dokument-Metadaten und Stylesheet-Links:** `<head>`-Angaben wie `link rel="stylesheet"`, `meta name` und `meta controller` aus dem alten Framework haben keinen entsprechenden Ladepfad. TiltUI lädt CSS über Komponenten-Metadaten und Runtime-Registrierung; für alte `index.html`-Dateien fehlt eine direkte Migration.
- [ ] **Template-Kontrollfluss:** `@if (...) { ... }`, `@else` und `@for (item[, index] in liste) { ... }` des alten Konverters fehlen. TiltUI wertet Bindings aus, erzeugt daraus aber keine bedingten oder wiederholten Teilbäume. Widget-Zustand und stabile Identität bei Listenänderungen berücksichtigen.
- [ ] **Ausdruckssprache:** Literale, Pfade, Array-Zugriffe, Arithmetik, Vergleiche, Logik und Ternär-Ausdrücke funktionieren. Alte Ausdrücke mit Methodenaufrufen wie `user.full_name()` und deren Controller-/Store-Auflösung sind noch nicht abgebildet; erlaubte Aufrufe und Auswertungsgrenzen festlegen.
- [ ] **Controller-Kontext pro HTML-Quelle/Komponente:** Das alte `controller`-Attribut beziehungsweise `meta controller` und dessen Daten-/Funktionsauflösung haben kein direktes Gegenstück. TiltUI verwendet registrierte Stores, Shared-Resources und `#[html_fn]`-Handler; ein lokaler Kontext würde Migration ohne globale Umbenennung ermöglichen.
- [ ] **Inline-Aktionen in Event-Attributen:** Altes HTML kann `$set`, `$add` und `$min` mit `$event`-Werten sowie mehrere durch Semikolon getrennte Aufrufe ausführen. TiltUI dispatcht registrierte Rust-Handler nach Namen; diese deklarativen Store-Mutationen fehlen.
- [ ] **Dynamische Attribute vollständig prüfen:** Für alte Property-Bindings prüfen, ob TiltUI den Zielwert am bestehenden Widget anwendet. `innerText`/`innerHtml`, Text, dynamische Klassen und gängige Werte funktionieren; Inline-Styles und Widget-Sonderattribute brauchen eine explizite Kompatibilitätsmatrix.
- [ ] **Beliebige HTML-Quellen zur Laufzeit:** Das alte `HtmlSource` kann ein frei gewähltes HTML-Asset als Quelle für einen UI-Baum verwenden und austauschen. TiltUI lädt registrierte Komponenten oder setzt Fragmente per `set_inner_html`; ein gleichwertiger Dokument-Asset-Pfad fehlt.
- [ ] **Sichtbarkeit bis zur Asset-Bereitschaft:** Das alte HTML-System kennt `HtmlPendingReveal`, `HtmlAllWidgetsSpawned` und `HtmlAllWidgetsVisible`. TiltUI stylt Komponenten während nachladende Author-CSS aussteht zunächst mit verfügbaren Regeln. Optionale „erst vollständig gestylt anzeigen“-Semantik prüfen.

## Routing und Laufzeitkonfiguration

- [ ] **Route-Keep-Alive:** Alte `load!(...)`-Routen behalten Instanz und Zustand beim Wegnavigieren. TiltUI despawnt bisherigen `router-outlet`-Inhalt bei einem Zielwechsel und erstellt ihn beim Zurückkehren neu.
- [ ] **Routentabellen und Pfadnormalisierung:** Das alte `Routes::merge` kombiniert separate Routentabellen und normalisiert Pfade. TiltUI sammelt `#[beu_routes]`-Registrierungen, bietet aber kein entsprechendes `merge` und vergleicht Pfade wörtlich. Redirect und Fallback sind vorhanden.
- [ ] **Laufzeitkonfiguration:** Die alte `ExtendedUiConfiguration` enthält Pfade für Komponenten, Assets, Sprachen und Themes sowie Theme-Namen; Render-Layer und HDR können dort als Resource geändert werden. TiltUI bietet Startkonfiguration und Registrierungs-APIs, aber keine vollständige dynamische Entsprechung.
- [ ] **Dateien aus Verzeichnissen entdecken:** Das alte Projekt kann Themes über `themes_path`/`theme_names` und Sprachen über `language_path` finden. TiltUI unterstützt benannte Themes und Fluent-Kataloge, verlangt aber explizite Registrierung. Für bestehende Asset-Strukturen Entdeckung oder Migrationsadapter ergänzen.

## CSS und Rendering

- [ ] **Inline-CSS und Priorität:** Altes `style="..."` wird als `HtmlStyle` angewendet und `!important` gesondert behandelt. TiltUI übernimmt statische `style`-Attribute nicht in die CSS-Kaskade und modelliert `!important` nicht.
- [ ] **Variablen und Wertfunktionen:** Alte `:root`-Variablen mit `var(--name[, fallback])` sowie `calc()`, `min()`, `max()` und `sin()` fehlen im TiltUI-CSS-Parser. Gemischte Einheiten und Laufzeitauflösung gegen Eltern-/Viewport-Größe prüfen.
- [ ] **Autorendefiniertes Grid:** `display: grid` ist akzeptiert und Tabellen erzeugen interne Tracks. Die alten Eigenschaften `grid-template-rows/columns`, `grid-auto-rows/columns`, `grid-auto-flow`, `grid-row/column`, `repeat()` und `minmax()` fehlen für normale Komponenten.
- [ ] **Weitere Box- und Flex-Eigenschaften:** Alte Border-Shorthands und seitenweise Border-Einstellungen, `box-sizing` und `flex-basis` fehlen. TiltUI unterstützt unter anderem `border-width`, `border-color`, `border-radius`, Flex-Richtung, Wrap, Grow und Shrink.
- [ ] **Bilder und Effekte als CSS-Hintergrund:** TiltUI unterstützt `background-color` und einfache lineare Gradienten. Alte `url(...)`-Hintergründe, `background-position`, `background-size`, `background-attachment` und `backdrop-filter: blur(...)` fehlen.
- [ ] **Schatten und Outline:** Alte `box-shadow`, `text-shadow` und `outline` samt Breite, Farbe und Offset besitzen noch keine TiltUI-Deklaration und kein Rendering.
- [ ] **Typografie aus CSS:** `line-height`, `text-wrap`, `text-transform` und frei benannte Font-Familien aus dem alten Stylesheet-Parser fehlen. TiltUI bietet Größe/Gewicht, Ausrichtung und `sans-serif`, `monospace`, `ui-symbols`.
- [ ] **Interaktions- und Stapel-Eigenschaften aus CSS:** `cursor`, `pointer-events`, `z-index` und `scroll-width` fehlen im CSS-Parser. System- und Bild-Cursor können bereits per `UiCursor` in Rust gesetzt werden; hier geht es um Stylesheet-Syntax und Anwendung.
- [ ] **CSS-Nesting:** Alte Regeln mit `&` als Elternselektor (etwa `button { &:hover { ... } }`) vor Migration expandieren oder im TiltUI-Parser unterstützen.
- [ ] **Visuelle Teilbaum-Transparenz:** `opacity` wird geparst und berechnet, aber nicht auf den gerenderten Bevy-UI-Teilbaum angewendet. Verhalten für Eltern, Text, Bilder und verschachtelte Komponenten festlegen und umsetzen.
- [ ] **Viewport-Parität auf WASM:** Das alte optionale `wasm-breakpoints` berücksichtigt die Browser-Viewport-Größe. TiltUI-Media-Queries nutzen das primäre Bevy-Fenster. Bei abweichender Canvas-/Browser-Größe Breakpoints prüfen und gegebenenfalls angleichen.

## Widgets und Interaktion

- [ ] **Formvalidierung nach Modus:** `validate="always|interact|send"` wird als `FormSettings.validation` gespeichert, steuert die Auswertung aber noch nicht. Einfache Eingabefehler aktualisieren `:invalid` unabhängig vom Modus; der Submit prüft erneut. Alte `Always`-/`Interact`-Semantik und vollständige `validation`-/`pattern`-Regeln implementieren.
- [ ] **Vollständige Formdaten:** `FormSubmitted.data` sammelt benannte Text- und angehakte Controls als `BTreeMap<String, String>`. Mehrfachauswahl, Option-Gruppen, Dateien und wiederholte Feldnamen gehen damit nicht vollständig in den Submit ein. Datenmodell mit Mehrfachwerten und Datei-Metadaten festlegen.
- [ ] **Alte Input-Optionen:** `InputField` kann Label und Bild-Icon integrieren, beim Fokusverlust leeren und Text an Zeichenanzahl oder Node-Breite begrenzen (`cap_text_at`). TiltUI hat eigene Labels, `maxlength` und Bevy-Textbearbeitung, aber diese Widget-Verhaltensoptionen nicht vollständig.
- [ ] **Bild-Icons an Controls:** Alte Button-, Checkbox-, Choice-, Switch- und Toggle-Widgets unterstützen Icon-Pfade beziehungsweise Platzierung. TiltUI nutzt überwiegend Text-/Symbol-Parts; Bild-Icons und Platzierungsoptionen ergänzen. Das vorhandene `hyperlink icon` ist davon getrennt.
- [ ] **Typisierte Optionswerte:** Altes `WidgetValue`/`ReflectedValue` kann in Choice-/List-/Option-Widgets Rust-Werte halten. TiltUI behandelt Optionswerte als Strings; typisierte Werte und entsprechende Change-Events für programmgesteuerte Auswahl fehlen.
- [ ] **DatePicker-Formate:** Das alte Widget akzeptiert frei definierte `format_pattern`-Muster und kann das Format eines gekoppelten Inputs übernehmen. TiltUI unterstützt `mdy`, `dmy`, `ymd` und einen gekoppelten Date-Input, aber keine frei angegebenen Muster. Lokalisierte Monatsnamen sind vorhanden.
- [ ] **Bild-`alt` als sichtbarer Fallback:** Das alte Image-Widget zeigt `alt` bei leerem oder fehlgeschlagenem `src` im UI. TiltUI speichert `alt` nur als Metadatum; sichtbaren Fallback und Rückwechsel nach erfolgreichem Laden ergänzen. Datei-Preview und natives SVG sind vorhanden.
- [ ] **Hyperlink-Ziele und Plattformen:** Das alte `browsers`-Attribut und `open-modal` erlauben Browser-Auswahl und Rückfrage. TiltUI öffnet nur HTTP(S) über native Systemprogramme; im Browser erzeugt es derzeit nur `LinkActivated`. WASM-Navigation und benötigte alte Link-Optionen ergänzen.
- [ ] **Dialog-Ergebnisprotokoll:** TiltUI unterstützt Bevy- und System-Dialoge. Das alte Ergebnis unterscheidet zusätzlich `Dismissed` und `Unavailable` und korreliert Requests über `request_id` sowie Provider/Modal-Art. Diese Informationen fehlen in `DialogClosed`; bei parallelen Dialogen und Backdrop-Klicks kompatible Zuordnung vorsehen.
- [ ] **Explizites `<scroll>` gegenüber Overflow-Scrollbar:** Das alte Tag erzeugt einen horizontalen oder vertikalen `Scrollbar` mit eigener Wert-/Viewport-Semantik. TiltUI materialisiert `<scroll>` über Slider-Parts; Overflow-Container erhalten separate automatische Scrollbars. Die Kopplung eines expliziten `<scroll>` an den Ziel-Viewport prüfen und ergänzen.

## Lokalisierung und Plattform

- [ ] **Alte Sprach-Backends und Platzhalter:** `bevy_extended_ui` kann `.ftl` und `.properties` mit Fluent-vor-Properties-Fallback sowie `{{ KEY }}`, `{{ %variable% }}` und gemischten Platzhaltern nutzen. TiltUI verwendet Fluent-Kataloge mit `{{ i18n.key }}` und Fluent-Argumenten. Für alte Sprachdateien und Templates Adapter oder gezielte Migration bereitstellen.
- [ ] **Automatische Sprachauswahl:** Im alten System kann `<html lang="...">` die Sprache erzwingen; sonst gelten `UILang` und anschließend die Systemsprache. TiltUI startet mit konfiguriertem Fallback und bietet `UiLocalization::set_locale`, liest aber Dokument-`lang` und OS-/Browser-Sprache nicht automatisch.
- [ ] **SVG-Bilder auf WASM:** Der alte `SvgImageLoader` rasterisiert SVG-Assets bei aktiviertem `svg`-Feature auch ohne ausdrückliche WASM-Sperre. TiltUIs direkte SVG-Rasterisierung ist mit `not(target_arch = "wasm32")` geschützt; ein entsprechender Asset-Loader fehlt. Browser-Ziel verifizieren und bei Bedarf ergänzen.

## Optionaler Legacy-Pfad

- [ ] **`UiRegistry` für benannte komplette Screens:** Der alte, vom `extended-framework` getrennte Legacy-Modus kann mehrere HTML-UIs registrieren, aktivieren, deaktivieren und entfernen. TiltUI hat Komponenten und Router, aber keine API mit dieser Screen-Lebensdauer. Nur für direkte Legacy-Migration übernehmen.

## Validierung im laufenden Fenster

- [ ] **Schnelles Body-Scrollen messen:** Radereignisse werden pro Frame gebündelt und der Body bewegt sich ohne Smooth-Scroll-Warteschlange; innere Scrollbereiche glätten mit begrenztem Rückstand. Regressionstests sind vorhanden. Framezeit und Scrollgefühl bei manuellen Rad-Bursts im Showcase prüfen; automatische Mausbedienung war hier ohne macOS-Bedienungshilfen-Zugriff nicht möglich.
- [ ] **ColorPicker-Drag messen:** Hover-bedingte CSS-Neuberechnung wurde im Debug-Showcase von ungefähr 60 ms auf 3 ms reduziert. Drag-Latenz und verbleibende Framezeit-Spitzen im Fenster prüfen.

## Bereits umgesetzt – nicht erneut als Lücke erfassen

- [x] Automatische Komponentenfindung; optionales `template_name`/`template_file`/`styles`-Schema mit mehreren Stylesheets; `#[ui_component]` bleibt Marker.
- [x] Typisierte Stores, Text-/Property-Bindings, häufige Ausdrücke und `@use` für registrierte Shared-Resources; `innerText`/`innerHtml` und reaktive Laufzeit-Inhalte.
- [x] Hover-, Fokus-, Scroll-, Tastatur-, Drag-, Touch- und Init-Events mit Ereignisdaten an `#[html_fn]`.
- [x] System- und Bild-Cursor in Rust mit Vererbung; Beispiele für Cursor und Laufzeit-Inhalt in `examples/component-showcase`.
- [x] Themes und Provider mit lokalem Scope, benannte Theme-Wechsel, Render-Layer und HDR für die automatisch erzeugte UI-Kamera.
- [x] Bildvorschau aus File-Inputs, native SVG-Quellen, Dialoge, Tooltip-Spitze, lokalisierte DatePicker-Monatsnamen sowie Route-Redirects und Fallback.
- [x] Gebündelte Body-Radereignisse und Scrollen ohne aufgestaute Body-Glättung; Regressionstests für Rad-Bursts und Body-Position.
