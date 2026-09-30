# Release-Ablauf

Die Workflows in [CI](../.github/workflows/ci.yml) und [Release](../.github/workflows/release.yml) testen das gesamte Rust-Workspace inklusive Examples. Die Release-Regeln laufen mit Node 20 und benötigen keine npm-Pakete. `main` ist der aktuelle Entwicklungsstand und erzeugt bei normalen Pushes weder einen GitHub Release noch ein crates.io-Paket.

| Auslöser | Ergebnis |
| --- | --- |
| Pull Request | Formatierung, Release-Plan-Tests und `cargo test --workspace --all-features --locked`; ein eigener Check prüft geplante Versionsnummern. |
| PR-Titel beginnt mit `RC:` und PR wird nach `main` gemergt | Die nächste freie Version `vX.Y.Z-rc.N` wird ermittelt. Alle acht Crates werden mit `X.Y.Z-rc.N` veröffentlicht, danach entsteht ein GitHub-Prerelease. Ein offener PR führt keine Veröffentlichung mit Secrets aus. |
| Push auf `release-X.Y.Z` | Wenn `X.Y.Z` der stabilen Workspace-Version in `Cargo.toml` entspricht und noch frei ist: Tests, Veröffentlichung aller acht Crates, Tag `vX.Y.Z` und GitHub Release. |
| Bereits vorhandener Tag oder ein bereits veröffentlichtes Crate dieser stabilen Version | Der Release-Job stoppt. Der Release-Version-Check zeigt im PR eine Warnung und schlägt fehl. |

Die RC-Nummer im PR-Check ist eine Vorschau. Der Workflow vergibt sie nach dem Merge neu, damit zwischenzeitlich veröffentlichte RCs berücksichtigt werden. Der Release-Workflow veröffentlicht nacheinander `tilt-ui-core`, `tilt-ui-html`, `tilt-ui-css`, `tilt-ui-icons`, `tilt-ui-macros`, `tilt-ui-build`, `tilt-ui-runtime` und `tilt-ui`. Die Examples sind nicht veröffentlichbar. Eine Versionsangabe an den lokalen Workspace-Abhängigkeiten sorgt dafür, dass veröffentlichte Pakete dieselbe Version aus crates.io beziehen.

## Einmalige Einrichtung

1. Auf [crates.io](https://crates.io/settings/tokens) einen Publish-Token für die acht `tilt-ui-*`-Crates erstellen. Für die erste Veröffentlichung muss der Token auch neue Crates anlegen dürfen. Die acht Namen müssen deinem Account zur Veröffentlichung offenstehen; bestehende fremde Crates können nicht übernommen werden.
2. Unter **GitHub → Settings → Environments** eine Umgebung `crates-io` anlegen und dort ein Secret `CARGO_REGISTRY_TOKEN` mit diesem Token hinterlegen. Der Workflow setzt daraus nur beim Publish-Schritt die Umgebungsvariable `CARGO_REGISTRY_TOKEN`. Optional kann die Umgebung vor Veröffentlichungen eine manuelle Freigabe verlangen.
3. GitHub Actions aktivieren und prüfen, dass der Release-Job `contents: write` für den automatischen `GITHUB_TOKEN` erhalten darf. `main` sollte über Branch-Schutz erfolgreiche CI-Checks vor dem Merge verlangen; `release-*`-Branches sollten nur Maintainer erstellen oder aktualisieren dürfen.
4. Die Workflow-Dateien zuerst nach `main` mergen. Danach einen `RC:`-PR mergen oder einen `release-X.Y.Z`-Branch pushen.

## Nächste Version vorbereiten

Nach `v0.1.0` zunächst die Basisversion anheben, beispielsweise:

```sh
node scripts/release.mjs prepare 0.1.1
cargo test --workspace --all-features
git add Cargo.toml Cargo.lock
git commit -m "chore: prepare 0.1.1"
```

Dann kann ein `RC:`-PR für `v0.1.1-rc.1` gemergt oder ein `release-0.1.1`-Branch gepusht werden. crates.io-Versionen sind unveränderlich. Falls ein Lauf nach einigen veröffentlichten Crates abbricht, zeigt der nächste Lauf die bereits belegte Version an und stoppt; die fehlenden Pakete und das GitHub-Tag müssen anhand der Actions-Logs gezielt nachgezogen werden, bevor der Release abgeschlossen ist.
