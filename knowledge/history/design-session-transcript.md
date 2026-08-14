---
type: Design Session Transcript
title: ncpages design session (verbatim, German)
description: Primary source. Full transcript and outcome of the 2026-08-15 design session that produced ncpages.
language: de
tags: [provenance, design-session, ncpages]
status: stable
generated: { by: human:heiss, at: 2026-08-15T00:29:00Z }
verified:
  - { by: human:heiss, at: 2026-08-15T00:29:00Z }
---

> **Provenance note.** This file is the verbatim primary source for this bundle and is
> kept in its original German. Every other concept here is derived from it; where an
> English concept and this transcript disagree, this transcript wins.

# ncpages — Designgespräch und Konzept

Protokoll und Ergebnis einer Brainstorming-Session vom 15.08.2026.

**Ausgangsfrage:** Wie kommt ein Obsidian-Blog ohne git-Handgriffe von Nextcloud
ins Netz?

**Ergebnis:** Ein Dienst namens ncpages, der einen Nextcloud-Ordner über WebDAV
beobachtet, bei Änderung baut und das Ergebnis selbst ausliefert. Dazu ein
Sicherheitsmodell, das den Build vom Vault-Inhalt trennt, und eine
Navigationslösung, die Obsidian-Frontmatter als Quelle nutzt.

---

# Teil 1 — Der Gesprächsverlauf

Dieser Teil dokumentiert, wie das Konzept entstanden ist, welche Ideen verworfen
wurden und an welchen Stellen sich die Richtung geändert hat. Wer nur das
Ergebnis braucht, springt zu Teil 2.

## 1.1 Die ursprüngliche Idee

Der Ausgangsentwurf war:

* auf einen Nextcloud-Ordner „lauschen" (inotify oder effizientes Polling)
* bei Änderung eine vorkonfigurierte Routine ausführen
* eine schmale Binary mit zwei Hooks: `install_script` beim Containerstart,
  `run_script` bei Änderungen
* das Ergebnis nach git-pages schieben — oder bei gleicher Maschine per
  Reverse Proxy ausliefern

Alle vier Punkte haben sich im Verlauf verändert, zwei davon grundlegend.

## 1.2 Erste Umdeutung: Das Watching ist nicht das Problem

Der Reflex, mit inotify anzufangen, führt an der eigentlichen Schwierigkeit
vorbei. Dateisystem-Watching ist ein gelöstes Problem — `watchexec` oder das
`notify`-Crate erledigen es in wenigen Zeilen. Was es nicht fertig gibt und was
den Wert ausmacht, ist die Zustandsmaschine dahinter: Debounce, Queue mit
Coalescing, Build-Sandbox, Qualitätsprüfung, atomarer Publish, Statusmeldung.

Zweite Umdeutung im selben Schritt: **nicht ins Dateisystem schauen, sondern
über WebDAV.** Nextcloud propagiert ETag-Änderungen im Verzeichnisbaum nach
oben — genau deshalb kann der Desktop-Client mit wenigen Requests entscheiden,
wo er absteigen muss. Für einen Watcher heißt das:

```
PROPFIND /remote.php/dav/files/<user>/Obsidian/Blog
Depth: 0
<d:prop><d:getetag/></d:prop>
```

Ein HTTP-Request, ein String-Vergleich, und man weiß, ob sich irgendwo unterhalb
des Ordners etwas geändert hat. Das ist billiger als jeder rekursive
inotify-Watch — und funktioniert unabhängig davon, wie und wo Nextcloud die
Dateien tatsächlich speichert.

Damit war klar: der ursprüngliche Wunsch nach „effizientem Polling" ist nicht
nur erfüllbar, er ist der *bessere* Weg, nicht der Kompromiss.

## 1.3 Push: notify_push statt webhook_listeners

Der Wunsch, von Nextcloud angestoßen zu werden statt zu pollen, führt zuerst zur
`webhook_listeners`-App. Die klingt passend, taugt aber nicht: sie triggert über
Background-Jobs mit einem Default-Cron-Intervall von fünf Minuten. Schneller
wird es nur mit mehreren dedizierten `occ`-Worker-Prozessen. Mehr bewegliche
Teile, schlechtere Latenz als simples Polling — verworfen.

Stattdessen **notify_push**: selbst eine Rust-Binary, Redis-PubSub → WebSocket,
Latenz etwa eine Sekunde. Sie sagt nur, *dass* sich etwas geändert hat, nicht
*was* — was perfekt passt: WebSocket weckt den Watcher, der ETag-Check sagt, ob
es real war. Bricht die Verbindung weg, fällt man auf Polling zurück.

Ein Detail, das den Aufbau vereinfacht: notify_push validiert Client-Credentials
gegen Nextcloud selbst. Der Watcher kann sich also direkt im Docker-Netz
verbinden (`ws://notify-push:7867/ws`) und braucht den Proxy-Pfad `/push` gar
nicht — der ist nur für die echten Desktop- und Mobile-Clients.

## 1.4 Der Sicherheitspunkt, der die Architektur bestimmt hat

Der ursprüngliche Entwurf sah vor, ein Bash-Skript zu „hinterlegen". Die
naheliegende Stelle wäre der Vault gewesen — bequem, vom Handy änderbar.

Das ist der schwerwiegendste Punkt der ganzen Session: **der Build ist per
Design Code-Ausführung.** Läge `build.sh` im Nextcloud-Ordner, hätte jeder mit
Schreibzugriff darauf eine Shell auf dem Server. Das schließt ein: ein
kompromittiertes Handy, ein alter Sync-Client mit gespeichertem App-Passwort,
jede Person, mit der der Ordner jemals geteilt wird, jeder Federated Share.

Vorher lief der Build in GitHub Actions — auch Code-Ausführung, aber mit Branch
Protection, Commit-Signaturen und Audit-Log davor. Das ersatzlos wegzuwerfen
wäre kein Fortschritt gewesen.

Die Entscheidung fiel klar: **Skripte und Build-Config liegen außerhalb des
Vaults**, in einem read-only gemounteten Config-Verzeichnis. Der Vault liefert
Inhalt, nicht Ausführungslogik. Der Preis — man kann die Build-Konfiguration
nicht mehr vom Handy ändern — ist ein Feature, kein Bug.

Diese Entscheidung hat später mehrfach Konsequenzen gehabt: sie hat das
`overrides/`-Verzeichnis (Jinja-Templates sind Code) auf die Config-Seite
gezogen, sie hat die Trennung von Watcher und Builder motiviert, und sie hat bei
der Navigationsfrage Variante (b) sofort ausgeschlossen.

## 1.5 Docker: drei Fallstricke

Mit `nextcloud:stable-fpm-alpine` als Basis kamen drei konkrete Probleme dazu.

**FPM spricht kein HTTP.** notify_push und der Watcher machen HTTP-Requests
gegen Nextcloud. Der FPM-Container kann die nicht beantworten — er spricht
FastCGI. `NEXTCLOUD_URL` und die WebDAV-Basis-URL müssen auf den
nginx-Container zeigen. Klingt trivial, produziert sonst aber Fehlermeldungen,
die nach Auth-Problem aussehen.

**Der Symlink-Bind-Mount.** Docker löst Mount-Quellen beim Containerstart auf.
Mountet man `/srv/blog/current` (den Symlink) in den Webserver-Container, bindet
Docker das Ziel, auf das er *gerade* zeigt. Jeder spätere Swap ist unsichtbar —
die Site aktualisiert sich nie, ohne Fehler, ohne Log. Richtig ist, das
Elternverzeichnis zu mounten und den Symlink im Container auflösen zu lassen.

**Die Netz-Isolation des Builders.** Der Wunsch war ein Build ohne
Netzwerkzugang. `network_mode: none` lässt sich aber nicht mit einem
Compose-Netzwerk kombinieren, und der Builder braucht eines für den Trigger.
Erreichbar ist `internal: true`: kein Egress ins Internet, aber Erreichbarkeit
im Stack. Das ist die ehrliche Grenze dessen, was Compose hergibt.

## 1.6 Der GitHub-Workflow: eine gute und drei unangenehme Überraschungen

Die gute: **es gibt kein Obsidian-Preprocessing.** Der Workflow ruft nur
`zensical build --clean`. Wikilinks und Embeds werden von einer eigenen
Markdown-Extension *innerhalb* des Builds behandelt. Der Umzug ist damit
deutlich kleiner als befürchtet.

Die unangenehmen:

**Der Workflow ist zustandsbehaftet.** `actions/cache` hält ein `site-previous`
vor, gegen das `static-webmentions` diffed. Dabei ist ein Bug: `cp -r site
site-previous` kopiert bei existierendem Ziel *hinein* statt darüber, also
`site-previous/site/site/…`. Der Webmention-Diff läuft seit dem zweiten Lauf
gegen einen teils veralteten, teils verschachtelten Baum. `|| true` und
`continue-on-error: true` haben verdeckt, dass das nie richtig funktioniert hat.

Im neuen Aufbau verschwindet das Problem vollständig: der vorherige Build liegt
ohnehin in `releases/`, `oldDir` ist `readlink current`.

**Der 12-Stunden-Cron ist funktional zwingend, nicht redundant.**
`fetch_comments.py` holt Webmentions und Annotationen von außen. Eingehende
Kommentare erscheinen nur, wenn gebaut wird. Ein rein änderungsgetriggerter
Builder würde bedeuten: jemand kommentiert, und der Kommentar taucht erst beim
nächsten Artikel auf. Also braucht ncpages eine dritte Trigger-Quelle neben
Push und Poll — einen Timer, abschaltbar für alle, die ihn nicht brauchen.

**Webmentions sind irreversibel.** Ein Webmention ist ein HTTP-Request an einen
fremden Server. Einmal raus, nicht zurückholbar. Daraus folgt eine harte
Reihenfolge: erst Qualitätsprüfung, dann Publish, dann senden. Und daraus folgt,
dass `cancel-in-progress: true` aus dem alten Workflow falsch wäre — ein Abbruch
zwischen Swap und Send hinterlässt einen Zustand ohne sauberen Rückweg.

Genau diese Beobachtung hat die **Vier-Phasen-Hook-Struktur** erzeugt, die jetzt
den Kern von ncpages ausmacht. Sie ist die Verallgemeinerung eines konkreten
Falls: irreversible Außenwirkung darf erst nach dem Gate laufen. Für
Suchmaschinen-Pings, Cache-Purges und Social-Posts gilt dasselbe.

Zwei weitere Funde: `static-webmentions` und `git-pages-cli` werden mit `latest`
ohne Checksum nach `/usr/local/bin` geladen und als root ausgeführt — in einem
Job, der die Deploy-Credentials in der Umgebung hat. `actions/setup-uv` ist
sauber auf einen Commit-SHA gepinnt, diese beiden nicht.

## 1.7 Die `zensical.toml`: die Prämisse bricht

Zwei Funde aus der Konfiguration.

**`obsidian_md` ist eigener Code**, kein Fremdpaket — eine Python-Markdown-
Extension unter `src/obsidian_md/`, die Zensical per Modulnamen lädt. Das hat
eine angenehme Konsequenz: statt `uv sync` zur Laufzeit (editable Install mit
absoluten Pfaden, die beim Volume-Wechsel brechen) reicht `uv sync --frozen
--no-install-project` im Dockerfile plus `PYTHONPATH`. Damit ist der Builder zur
Laufzeit vollständig netzlos, und der ursprünglich geplante `install_script`-Hook
entfällt ersatzlos. Dependency-Änderungen werden zu bewussten Image-Rebuilds
statt stillem Drift — bei `zensical 0.0.x` mit möglichen Breaking Changes eine
Verbesserung.

**Der explizite `nav`-Baum bricht die ganze Idee.** Die Konfiguration enthält
einen vollständig ausformulierten Navigationsbaum mit sieben Sektionen und
kuratierten Titeln. Zensical leitet Navigation nur dann aus der
Verzeichnisstruktur ab, wenn kein `nav` definiert ist.

Konkret: Man schreibt in Obsidian eine neue Notiz, sie synct, der Watcher
triggert, Zensical baut sie — und sie taucht in der Navigation nicht auf. Um sie
sichtbar zu machen, müsste man die `zensical.toml` editieren. Die liegt nach der
Sicherheitsentscheidung auf dem Server. Also: SSH, Editor, Neustart.

Damit wäre genau das zurück, was man loswerden wollte — nur ist SSH schlechter
als git.

Vier Auswege wurden geprüft:

| | Ansatz | Bewertung |
|---|---|---|
| (a) | `nav` streichen, implizite Navigation | Das `docs/`-Verzeichnis ist überwiegend flach → alphabetische Liste von 46 Seiten. Umbau des Vaults würde URLs ändern und Webmentions ins Leere zeigen lassen. Verworfen. |
| (b) | `zensical.toml` in den Vault | Die Datei referenziert `custom_dir` und Extension-Modulnamen, also Import-Pfade. Vault-editierbar = Code-Ausführung über Nextcloud. Verworfen. |
| (c) | Validiertes `nav`-Fragment im Vault | Funktioniert, braucht aber ein schema-validierendes Merge-Primitive im Kern. |
| (d) | Navigation aus Obsidian-Frontmatter aggregieren | Gewählt. |

Bei der Bewertung von (d) lag eine Fehleinschätzung vor: Es hieß zunächst, (d)
brauche *mehr* Kern-Logik. Das Gegenteil stimmt. Bei (c) käme eine
konfigurationsartige Datei aus dem Vault, die der Kern validieren müsste. Bei
(d) kommt nur Frontmatter in ganz normalen Notizen — die Aggregation ist ein
Hook im Rezept, der Kern bleibt frei von Zensical-Wissen, und die Angriffsfläche
ist null.

Dazu ein Vorteil, der erst später sichtbar wurde: **URLs bleiben stabil.**
Navigation ist von Dateipfaden entkoppelt, Umsortieren bricht keine Links.

## 1.8 Zwei Stacks, Nextcloud als weiche Abhängigkeit

Der Blog sollte einen eigenen Compose-Stack bekommen, aber Nextcloud als
Abhängigkeit haben. Das führte zu einer Präzisierung: „Abhängigkeit" kann
Startup-Unabhängigkeit oder Substituierbarkeit heißen. Beides ließ sich mit
einer Umdeutung erreichen: **das Quellverzeichnis ist eine persistente
Working Copy, kein Cache.** Nextcloud ist nur *ein* Mechanismus, sie zu
aktualisieren.

Damit läuft der Blog weiter, wenn die Nextcloud steht: Timer-Builds holen
weiter Kommentare, die Site bleibt live. Nur der Vault-Sync pausiert.

Ein Detail dabei: das Default-Netz eines Compose-Stacks als `external`
einzubinden ist eine Falle. Ein `docker compose down` des Nextcloud-Stacks
löscht das Netz und legt es mit neuer ID an — die Blog-Container hängen dann an
etwas, das es nicht mehr gibt. Ein manuell angelegtes drittes Netz überlebt das,
hat ein festes Subnetz für `trusted_proxies` und macht die Abhängigkeitsrichtung
explizit.

## 1.9 Die Wende bei der Auslieferung

Bis hierhin ging der Entwurf davon aus, dass es außerhalb des Stacks ein
Publish-Ziel gibt — git-pages oder ein Webserver. Daraus folgte die Frage, ob
das auf derselben Maschine läuft, und daraus wiederum ein ganzes
Publish-Backend-Menü mit rsync, SSH-Keys und Atomaritätsproblemen.

Der Einwand dagegen war berechtigt und traf einen echten Punkt: Wenn ohnehin
alles über WebDAV läuft, ist die Maschine doch egal.

**Für den Input stimmt das vollständig.** WebDAV holt die Dateien, der
Nextcloud-Client macht es nicht anders, das ist generisch und richtig.

**Für den Output nicht.** `rename(2)` auf einem Symlink innerhalb eines
Dateisystems ist atomar — es gibt keinen Moment, in dem ein Request eine halbe
Site sieht. Über ein Netzwerkprotokoll existiert das nicht. Weder WebDAV noch
rsync noch S3 kennen „vertausche zwei Verzeichnisse atomar". Und ohne
Atomarität wird das Gate wirkungslos (die Site ist während des Uploads gemischt)
und die Webmentions falsch getimt (es gibt keinen definierten Zeitpunkt „ist
jetzt live"). Genau das ist das aktuelle Verhalten von `git-pages-cli
--upload-dir`, das man loswerden wollte.

Die Auflösung kam aus der nächsten Idee: **ncpages liefert selbst aus.** Ein
schmaler Webserver im Stack, der Nutzer proxied darauf wie auf jeden anderen
Container. Damit verschwindet die Frage nach der Maschine komplett — und mit ihr
alle Remote-Publish-Backends, SSH-Keys, Deploy-Secrets und die gesamte
Atomaritätsdiskussion. Der Nutzer-Vertrag reduziert sich auf: zwei Volumes, ein
Port.

Das war die stärkste Vereinfachung der Session, und sie kam nicht aus der
Analyse, sondern aus dem Widerspruch dagegen.

## 1.10 Nav-Aggregator: gebaut und verifiziert

Die Konvention wurde festgelegt, ein Migrationsskript und ein Aggregator
geschrieben und gegen die echte Konfiguration getestet.

Der Test: `nav`-Baum → Frontmatter in 46 Dateien → `nav` aus der Config
entfernen → aus dem Frontmatter neu aggregieren → mit dem Original vergleichen.
Ergebnis identisch. Auch bei zufällig gemischter Eingabereihenfolge, was
notwendig ist, weil Dateisystem-Traversierung keine garantierte Ordnung hat.

---

# Teil 2 — Das Konzept

## 2.1 Was ncpages ist

Ein Dienst, der einen Nextcloud-Ordner über WebDAV beobachtet, bei Änderung
einen konfigurierbaren Build ausführt und das Ergebnis über einen eigenen
schmalen Webserver ausliefert.

**Nicht Ziel:** TLS, Zertifikate, DNS, Domain-Routing. Das macht der Reverse
Proxy des Nutzers.

**Abgrenzung.** Wer Nextcloud auf lokalem Storage ohne Server-Verschlüsselung
fährt, auf derselben Maschine baut und keine irreversiblen Post-Publish-Schritte
hat, kommt mit `watchexec` plus einem Shell-Skript aus. Der Mehrwert von ncpages
liegt in fünf Dingen:

* WebDAV statt Dateisystem — funktioniert mit S3-Primary-Storage und
  serverseitiger Verschlüsselung, wo inotify prinzipiell nicht funktionieren kann
* Push statt Polling
* Gate gegen halb-gesyncte Zustände
* atomarer Publish
* garantierte Phasenordnung für irreversible Schritte

## 2.2 Der Ablauf

```
┌── Trigger ──────────────────────────────────────────────┐
│  notify_push (WebSocket)  ~1 s                          │
│  WebDAV-ETag-Poll         30 s   (Sicherheitsnetz)      │
│  Timer                     6 h   (optional)             │
└───────────────────┬─────────────────────────────────────┘
                    │  Debounce 10 s / Hard-Deadline 120 s
                    │  on_busy = queue_latest
                    ▼
  1. SYNC        WebDAV-Delta → src/                    [Netz, Credentials]
  2. ASSEMBLE    src/ + /etc/ncpages/ → build/          [lokal]
  3. pre_build   nav aus Frontmatter, Kommentare holen  [Netz, Credentials]
  4. build       zensical build --clean → build/site/   [ISOLIERT]
  5. post_build  Post-Processing am HTML                [Netz, Credentials]
  6. MOVE        build/site/ → releases/<id>/           [lokal, mv]
  7. GATE        Pflichtdateien, Seitenzahl, Nav-Diff   [lokal]
  8. PUBLISH     current → releases/<id>   rename(2)    [ATOMAR]
  9. post_publish Webmentions senden, Cache purgen      [IRREVERSIBEL]
 10. REPORT      Status nach Nextcloud + ntfy, Retention
```

Bricht ein Schritt ab, bleibt `current` unverändert stehen. Die Site ist nie in
einem Zwischenzustand. Schritt 9 läuft ausschließlich, wenn 8 erfolgreich war.

**Trigger-Details.** Alle drei Quellen speisen denselben Event-Channel und sind
einzeln abschaltbar. Der Poll läuft auch bei aktivem Push weiter, nur mit
größerem Intervall — als Sicherheitsnetz für abgerissene WebSockets. Der Timer
bekommt Jitter (0–10 %), damit nicht alle Installationen gleichzeitig bei
externen APIs aufschlagen; bei aktivem Timer ist „kein Build seit 2 × Intervall"
zugleich ein Liveness-Signal.

**Debounce.** Obsidian speichert alle paar Sekunden automatisch, und ein Rename
mit Link-Update schreibt Dutzende Dateien. Zehn Sekunden Ruhe, spätestens nach
120 Sekunden wird trotzdem gebaut.

**`queue_latest`.** Maximal ein laufender Build plus ein wartender Slot; neue
Events überschreiben den wartenden. Kein Abbruch laufender Builds, weil Schritt
9 irreversibel ist.

## 2.3 Topologie

```
┌─ Stack: nextcloud ────────────┐   ┌─ Stack: netzmuffel ──────────────┐
│  db, redis                    │   │                                  │
│  nextcloud (fpm)              │   │  watcher   [nc-bridge, build]    │
│  nginx        ──┐             │   │  builder   [build]  internal     │
│  notify-push  ──┤             │   │  web       [edge]   :8080        │
└─────────────────┼─────────────┘   └───────────┬──────────────────────┘
                  │                             │
              cloud-bridge (extern, 172.28.0.0/16)
                                                │
                                    Reverse Proxy des Nutzers → :8080
```

**Rollentrennung.** Der Watcher hat Nextcloud-Credentials und Netzzugang, aber
keine Build-Tools. Der Builder hat Build-Tools, aber keine Credentials und
keinen Egress. Getriggert wird über einen internen HTTP-Endpunkt mit Shared
Token — kein Docker-Socket, der wäre gleichbedeutend mit Root auf dem Host.

**Der Webserver hat kein `depends_on`.** Fallen Watcher oder Builder aus, bleibt
die Site live. Er ist der einzige echte Single Point of Failure und hat selbst
keine Abhängigkeiten.

**Volumes**

| Volume | Inhalt | Schreiber | Leser |
|---|---|---|---|
| `src` | Vault-Working-Copy (`docs/`) | watcher | builder (ro) |
| `releases` | `build/`, `releases/<id>/`, `current` | watcher, builder | web (ro) |
| `state` | letzter ETag, Content-Hash, Build-Historie | watcher | — |

**Arbeitsbaum**, auf demselben Volume wie `releases/`, damit `mv` atomar bleibt:

```
/work/build/
├── zensical.toml        ← /etc/ncpages/   (nav wird vom Hook ergänzt)
├── pyproject.toml       ← /etc/ncpages/
├── uv.lock              ← /etc/ncpages/
├── overrides/           ← /etc/ncpages/   (Jinja = Code)
├── src/obsidian_md/     ← /etc/ncpages/
├── docs/                ← Nextcloud-Vault
└── site/                → mv nach releases/<id>/
```

## 2.4 Sicherheitsmodell

Drei Ebenen.

**Trennung von Inhalt und Code.** Der Vault enthält ausschließlich `docs/` —
Markdown, Bilder und `stylesheets/extra.css`. Alles Ausführbare oder
Konfigurierende liegt in `/etc/ncpages/`, root-owned, read-only gemountet.
Ein präparierter Blogpost kann keine Build-Konfiguration verändern.

Der Overlap-Check ist **fail-closed**: Liegt das Hook-Verzeichnis innerhalb von
`source.path`, verweigert ncpages den Start mit erklärender Meldung. Nicht
warnen — jemand würde es sonst aus Bequemlichkeit tun.

**Sandbox.** Der Builder läuft ohne Egress (`internal: true`), mit
`read_only: true`, `cap_drop: ALL`, `no-new-privileges`, als nicht-root, mit
Speicher- und Zeitlimit, tmpfs auf `/tmp`. Selbst wenn eine
Zensical-Extension Code ausführen könnte, kommt er nirgends hin.

**Credential-Isolation.** Der Builder hat keine Secrets. Alle Zugriffe nach
außen — WebDAV, Kommentar-API, Webmentions — laufen im Watcher. Das ist strenger
als der bisherige GitHub-Job, in dem Build und Token-Zugriff im selben Kontext
lagen.

**Sonderfall Endlosschleife.** Der Status-Report wird nach Nextcloud
zurückgeschrieben. Läge der Pfad innerhalb des überwachten Ordners, änderte sich
das Wurzel-ETag → Trigger → Build → Status → unendlich. Pfad-Excludes helfen
nicht, weil das Wurzel-ETag pfadblind ist. Der Status-Pfad muss ein
Geschwisterordner sein, zusätzlich hält der Watcher einen Fingerprint des
zuletzt selbst geschriebenen Zustands.

## 2.5 Hook-Kontrakt

| Phase | Netz | Secrets | Läuft in | Zweck |
|---|---|---|---|---|
| `pre_build` | ja | ja | Watcher | Nav generieren, externe Daten holen |
| `build` | **nein** | **nein** | Builder | `zensical build --clean` |
| `post_build` | ja | ja | Watcher | HTML-Post-Processing, vor dem Gate |
| `post_publish` | ja | ja | Watcher | Irreversibles: Webmentions, Cache-Purge |

Environment für alle Hooks:

```
NCPAGES_SRC_DIR      Vault-Working-Copy
NCPAGES_BUILD_DIR    zusammengesetzter Arbeitsbaum
NCPAGES_OUT_DIR      build/site
NCPAGES_RELEASE_DIR  releases/<id>            (ab post_build)
NCPAGES_PREV_DIR     vorheriger Release       (leer beim ersten Build)
NCPAGES_TRIGGER      push | poll | timer | manual
```

Exit-Codes: `0` = ok, `1` = Warnung (Build läuft weiter, erscheint im Report),
`2` = Abbruch.

Kein Plugin-System, keine dynamischen Module. Skripte plus Environment-Variablen
— eine Schnittstelle, die in fünf Jahren noch funktioniert.

## 2.6 Gate

Läuft nach dem Build, vor dem Swap. Bei Verletzung wird nicht publiziert,
`current` bleibt stehen, es gibt eine laute Meldung.

* Pflichtdateien vorhanden (`index.html`, `sitemap.xml`)
* Mindestseitenzahl
* Seitenzahl-Rückgang gegenüber dem letzten Release unter Schwelle
* Nav-Diff unter Schwelle
* keine doppelten Basisnamen (bricht sonst die Wikilink-Auflösung)
* Konfliktkopien gefiltert **und gemeldet**

Der wichtigste Punkt ist der dritte. Das realistische Szenario: Sync-Fehler oder
versehentliches Löschen auf dem Handy, der Vault ist serverseitig halb leer, der
Build läuft mit Exit 0 durch, und eine dreiseitige Website ersetzt den Blog. Der
Exit-Code allein ist kein ausreichendes Signal.

Konfliktkopien werden nicht nur gefiltert, sondern gemeldet: eine
`… (conflicted copy 2026-08-14 120000).md` bedeutet, dass gerade eine Version
der eigenen Arbeit verloren zu gehen droht.

## 2.7 Navigation

Die Navigation entsteht aus Obsidian-Frontmatter, aggregiert von einem
`pre_build`-Hook.

```yaml
---
title: Bounded Context
nav: Architecture & Strategy/Domain-Driven Design
nav_order: 130
---
```

**Regeln**

* Separator `/`, alternativ eine Liste für Titel mit Schrägstrich.
* **Gruppen ohne eigene Datei** sortieren nach dem Minimum der `nav_order`
  ihrer Nachkommen, bei Gleichstand alphabetisch. Kein Sidecar, deterministisch.
* **Section-Index:** Ist `nav` exakt gleich einem Gruppenpfad statt darunter,
  wird die Seite zur Index-Seite dieser Gruppe.
* **Kein `nav:`** → Seite wird gebaut, ist per Link erreichbar, steht nicht im
  Menü. Für einen Digital Garden legitim; Waisen erscheinen im Status-Report,
  damit es eine Entscheidung bleibt und kein Versehen.
* **`draft: true`** → komplett ausgeschlossen. Ersatz für die Preview-Funktion,
  die mit den Pull Requests wegfällt.
* `nav_order` in Zehnerschritten, damit Einfügen ohne Umnummerieren geht.
  Kollisionen sortieren alphabetisch nach Titel.

**Werkzeuge**

* `nav_lib.py` — Frontmatter-Parsing und Baum-Konvertierung in beide Richtungen.
  Bewusst minimal statt PyYAML: liest nur flache Key-Value-Paare, überspringt
  eingerückte Zeilen, lässt Obsidian-Properties mit Listen unangetastet.
* `migrate_nav.py` — einmalig: `nav` aus der `zensical.toml` → Frontmatter in
  `docs/*.md`. Dry-Run per Default. Meldet fehlende Dateien und Waisen.
* `nav_from_frontmatter.py` — der `pre_build`-Hook. Warnt bei Sektionen mit nur
  einer Seite (Tippfehler-Indikator) und bei doppelten Basisnamen.

**Verifikation.** Round-trip über alle 46 Seiten byte-identisch, auch bei
gemischter Eingabereihenfolge.

## 2.8 Build

* `python:3.13-slim`, **nicht Alpine** — Zensical ist maturin-basiert; ohne
  `musllinux`-Wheels kompiliert pip den Rust-Teil aus dem Source.
* `uv sync --frozen --no-install-project --group dev` im Dockerfile.
  Zur Laufzeit läuft `uv` nicht mehr.
* `PYTHONPATH=/work/build/src` statt editable Install, weil `obsidian_md`
  per Modulnamen geladen wird.
* `static-webmentions` mit Version und sha256 ins Image, nicht zur Laufzeit
  geladen.
* Feste UID, identisch zum Watcher — sonst `EACCES` auf `releases/` beim
  zweiten Build.

## 2.9 Auslieferung

`static-web-server` (Rust, ~5 MB, scratch-Image) mit `--root /site/current`.
Das Volume wird als Elternverzeichnis gemountet, damit der Symlink im Container
aufgelöst wird. Open-File-Caching aus.

Caching-Header sind jetzt eigene Verantwortung, vorher hat git-pages das
gemacht: `immutable` mit langem `max-age` für `assets/` (Zensical erzeugt
gehashte Asset-Namen), `no-cache` für HTML. Falsch gesetzt bedeutet altes HTML
mit neuem CSS nach einem Swap.

`releases/` hält fünf Stände: Rollback, und zugleich `oldDir` für den
Webmention-Diff.

---

# Teil 3 — Red Team

Sammlung aller identifizierten Bruchstellen, thematisch geordnet.

## 3.1 Was die Architektur bestimmt hat

**Build = Code-Ausführung.** Behandelt durch Trennung Vault/Config, Sandbox und
fail-closed Overlap-Check.

**inotify auf dem Nextcloud-Data-Dir bricht dreifach:** bei serverseitiger
Verschlüsselung (verschlüsselte Blobs auf Platte, aus denen man nicht bauen
kann), bei S3-Primary-Storage (es liegen gar keine Dateien im Dateisystem), bei
Group Folders und External Storage (abweichende ETag-Propagation). Behandelt
durch WebDAV als Default.

**Die Endlosschleife durch den Status-Report.** Behandelt durch
Geschwisterordner plus Fingerprint.

**Publish eines leeren oder kaputten Zustands.** Behandelt durch das Gate.

**Irreversible Post-Publish-Schritte.** Behandelt durch die Vier-Phasen-Ordnung
und `queue_latest`.

**Der Symlink-Bind-Mount in Docker.** Behandelt durch Mounten des
Elternverzeichnisses.

## 3.2 Was im Betrieb beißt

* **Zwischenzustände während des Syncs.** Ein Rename mit Link-Update schreibt
  viele Dateien, der Sync überträgt sie nicht atomar. Es gibt keine
  Transaktionsgrenze im WebDAV-Sync, die man abwarten könnte. Debounce macht es
  unwahrscheinlich, das Gate fängt den Rest, der nächste Build repariert.
* **Konfliktkopien** würden ohne Filter zu öffentlichen Seiten.
* **Nextcloud-Maintenance-Mode** → 503 im Poll → exponentielles Backoff statt
  Hot Loop. Bei 401 sofort aufhören (Brute-Force-Schutz).
* **`trusted_proxies`** muss das Subnetz des nginx-Containers enthalten, sonst
  scheitert der notify_push-Selftest. Häufigster Grund für nicht funktionierende
  Setups.
* **State-Verlust beim Neustart.** ETag und Content-Hash müssen persistiert
  werden, sonst Vollbuild nach jedem `compose up` — und die Reconcile-Logik
  bleibt ungetestet, bis sie wirklich gebraucht wird.
* **Bootstrap-Zustand.** Ohne `current` liefert der Webserver 404 auf alles.
  Der Watcher legt beim Start eine Holding-Page an, falls nichts da ist.
* **Volume-Wachstum.** Retention muss aktiv durchgesetzt werden; ein volles
  Root-Dateisystem nimmt die Nextcloud mit.
* **Zeitstempel** sind über Sync-Grenzen unzuverlässig. Änderungserkennung nur
  über ETag und Hash, nie über mtime.
* **Kein Dashboard mehr.** GitHub Actions hat Fehler ins Gesicht geschrieben,
  ein toter systemd-Service schweigt. Deshalb Status-Report, ntfy und
  `/healthz`.

## 3.3 Was der Umzug kostet

* **Historie und Rollback-Granularität.** Git bot Diff, Blame, atomare
  Multi-File-Commits, Revert und über Pull Requests implizit eine Preview.
  Nextcloud-Versionierung ersetzt davon nur die Datei-Historie, ohne
  Commit-Grenzen. Vorschlag als Ausgleich: git als unsichtbare
  Implementierungsschicht, ~15 Zeilen, die vor jedem Build in ein lokales
  Bare-Repo committen. Man fasst git nie an, bekommt aber exakte Diffs,
  reproduzierbare Rebuilds, `git bisect` bei Layout-Regressionen und ein Backup
  unabhängig von Nextcloud. Nicht entschieden.
* **Draft-Vorschau.** `draft: true` schließt aus, zeigt aber nichts. Ein zweiter
  Vault-Ordner mit eigenem Publish-Ziel hinter Basic Auth wäre der Ersatz.
* **Zertifikat und DNS** liegen jetzt beim Nutzer statt beim Pages-Anbieter.

## 3.4 Falls veröffentlicht

* **Der Support-Aufwand ist asymmetrisch.** Die meisten Issues werden fremde
  Deployments sein — kaputtes notify_push, falsche `trusted_proxies`,
  S3-Storage, Verschlüsselung. Ein `ncpages doctor`, das die gesamte
  Red-Team-Liste als ausführbare Checks enthält, plus ein Issue-Template, das
  dessen Ausgabe verlangt, ist die einzige wirksame Gegenwehr.
* **Sicherheitsverantwortung.** Das Tool führt per Design Code aus, wenn sich
  ein Cloud-Ordner ändert. Jemand wird es auf einen geteilten Team-Ordner
  richten. `THREAT_MODEL.md` gehört vor die Installationsanleitung.
* **Frühe Generalisierung.** Der Reflex, Sources für S3, Dropbox und SFTP zu
  abstrahieren, bevor der Nextcloud-Pfad rund läuft, tötet das Projekt. Das
  `Source`-Trait bleibt im Code, ausgeliefert werden `webdav` und `fs`. Die
  einzige Abstraktion, die v1 braucht, ist Kern gegen Rezept.
* **Multi-Arch ist Pflicht.** Zielgruppe ist Homelab und Selfhosting, ein großer
  Teil davon läuft auf arm64.
* **Der Name.** Nextcloud GmbH hält die Wortmarke. Ein Community-Repo ist
  risikoarm, ein Produktname mit Logo und Docker-Image kann nach offizieller
  Zugehörigkeit aussehen — „Pages" verstärkt das, weil GitHub-, GitLab- und
  Codeberg-Pages Erstanbieter-Features sind. Vor dem ersten Release entscheiden.
* **Abandonment-Risiko.** Ein Tool im Publikationspfad fremder Websites, das
  drei Jahre unmaintained ist, ist schlimmer als keins. Ein ehrlicher Satz im
  README ist besser als stille Enttäuschung.

---

# Teil 4 — Offene Punkte

**`fetch_comments.py`** — die einzige Lücke, die die Pipeline-Reihenfolge
betrifft. Schreibt es nach `docs/` (dann `pre_build`) oder liest es `site/`
(dann `post_build`)? `beautifulsoup4` in den Dependencies deutet auf
HTML-Parsing, also Letzteres. Im alten Workflow läuft es *nach* dem Build —
falls es doch nach `docs/` schreibt, hat der aktuelle Blog eine 12-Stunden-
Verzögerung bei eingehenden Kommentaren.

**`google-genai`** in den Dev-Dependencies. Hängt ein LLM-Call im Build-Pfad?
Dann: Nichtdeterminismus, Kosten bei jedem Timer-Lauf, ein weiteres Secret, eine
externe Abhängigkeit. Bräuchte Content-Hash-Caching.

**`/de/` zeigt ins Leere.** `[project.extra] alternate` deklariert Englisch und
Deutsch, aber es gibt keine i18n-Struktur im `nav` und kein Übersetzungs-Plugin
in den Dependencies. Der Sprachumschalter im Header linkt auf 404. Kein Blocker,
aber `google-genai` legt nahe, dass automatische Übersetzung der Plan ist — das
würde die Build-Pipeline erheblich verändern.

**git als interne Schicht** — vorgeschlagen, nicht entschieden. Siehe 3.3.

---

# Teil 5 — Bugs im aktuellen Workflow

Beide entfallen mit dem Umzug, sind aber bis zum Cutover aktiv.

**`cp -r site site-previous`.** `actions/cache` stellt `site-previous` per
`restore-keys` wieder her, das Verzeichnis existiert also bereits, wenn die
Zeile läuft. `cp -r src dst` kopiert bei existierendem `dst` nicht *darüber*,
sondern *hinein*: `site-previous/site/`, beim nächsten Lauf
`site-previous/site/site/`, und daneben liegt weiter die Kopie aus dem ersten
Lauf. Der Webmention-Diff vergleicht seit dem zweiten Lauf gegen einen teils
veralteten, teils verschachtelten Baum — je nach Traversierung entweder
verpasste oder wiederholt gesendete Webmentions. `|| true` und
`continue-on-error: true` haben dafür gesorgt, dass davon nie etwas sichtbar
wurde.

Sofort-Fix: `rm -rf site-previous && cp -r site site-previous`.

**Zwei ungepinnte Binaries als root.** `static-webmentions` und `git-pages-cli`
werden mit `latest` ohne Checksum nach `/usr/local/bin` geladen und ausgeführt —
in einem Job, der `GIT_PAGES_PASSWORD` und `WEBMENTION_IO_TOKEN` in der
Umgebung hat. `actions/setup-uv` ist sauber auf einen Commit-SHA gepinnt, diese
beiden nicht.

---

# Anhang — Dateien aus dieser Session

| Datei | Zweck |
|---|---|
| `ncpages-konzept.md` | Kompaktes Konzept mit Config-Referenz und TODO-Liste in acht Phasen |
| `chat.md` | Dieses Dokument |
| `nav_lib.py` | Frontmatter-Parsing, Baum-Konvertierung in beide Richtungen |
| `migrate_nav.py` | Einmalige Migration `nav` → Frontmatter |
| `nav_from_frontmatter.py` | `pre_build`-Hook: Frontmatter → `nav` |
