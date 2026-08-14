---
type: Concept Note
title: ncpages concept note (verbatim, German)
description: Primary source. Condensed concept with config reference and the eight-phase TODO list, written at the end of the design session.
language: de
tags: [provenance, concept, ncpages]
status: stable
generated: { by: human:heiss, at: 2026-08-15T00:29:00Z }
verified:
  - { by: human:heiss, at: 2026-08-15T00:29:00Z }
---

> **Provenance note.** Verbatim primary source, kept in the original German.
> Derived English concepts live elsewhere in this bundle; on conflict, this file wins.

# ncpages — Konzept

> Statischer Blog-Publisher, der einen Nextcloud-Ordner überwacht und bei Änderung
> baut und ausliefert. Ersetzt für netzmuffel.de die Kette
> git → GitHub Actions → git-pages.

Stand: 14.08.2026 · Referenz-Deployment: netzmuffel.de (Obsidian + Zensical)

---

## 1. Ziel und Abgrenzung

**Problem.** Publizieren erfordert heute git-Handgriffe. Änderungen aus Obsidian
sollen ohne Umweg live gehen.

**Lösung.** Ein Dienst beobachtet einen Nextcloud-Ordner über WebDAV, baut bei
Änderung mit konfigurierbaren Skripten und liefert das Ergebnis über einen
eigenen schmalen Webserver aus.

**Nicht Ziel.** TLS, Zertifikate, DNS, Domain-Routing. Das macht der
Reverse Proxy des Nutzers (nginx, NPM, Traefik, Caddy) wie bei jedem anderen
Container auch.

**Abgrenzung zu `watchexec` + Bash.** Wer Nextcloud auf lokalem Storage ohne
Server-Verschlüsselung fährt, Builds auf derselben Maschine macht und keine
irreversiblen Post-Publish-Schritte hat, braucht ncpages nicht. Der Mehrwert
liegt in: WebDAV statt Dateisystem (funktioniert mit S3-Primary-Storage und
serverseitiger Verschlüsselung), Push statt Polling, Gate gegen halb-gesyncte
Zustände, atomarer Publish, garantierte Phasenordnung für irreversible Schritte.

---

## 2. Gesamtablauf

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

Bricht ein Schritt ab, bleibt `current` unverändert stehen — die Site ist nie
in einem Zwischenzustand. Schritt 9 läuft ausschließlich, wenn 8 erfolgreich war.

**Warum diese Reihenfolge.** Webmentions sind HTTP-Requests an fremde Server und
nicht zurückholbar. Sie dürfen erst feuern, wenn der Stand tatsächlich live ist.
Das gilt generisch für Suchmaschinen-Pings, Cache-Purges und Social-Posts.

---

## 3. Entscheidungen

### 3.1 Änderungserkennung

| Entscheidung | Begründung |
|---|---|
| **WebDAV-ETag-Poll** als Basis | Nextcloud propagiert ETags im Baum nach oben. Ein `PROPFIND Depth: 0` auf den Wurzelordner = ein Request, ein String-Vergleich, erkennt jede Änderung darunter. Maschinenunabhängig, funktioniert mit S3-Storage und Verschlüsselung. |
| **notify_push** als Beschleuniger | Rust-Binary, Redis-PubSub → WebSocket. Latenz ~1 s. Sagt nur *dass* sich etwas geändert hat → danach ETag-Check. Fällt bei Verbindungsverlust auf Polling zurück. Redis ist im Ziel-Setup vorhanden. |
| **inotify verworfen** (bleibt als `fs`-Source für lokale Setups) | Bricht bei S3-Primary-Storage, bei serverseitiger Verschlüsselung (verschlüsselte Blobs auf Platte) und bei Group Folders. Nicht rekursiv, `max_user_watches`-Limits, Events beim Neustart verloren, Sync-Client schreibt über `.part`-Dateien + `MOVE`. |
| **`webhook_listeners` verworfen** | Läuft über Background-Jobs, Default-Cron-Intervall 5 Minuten. Schneller nur mit mehreren dedizierten `occ`-Workern in tmux. Mehr Teile, schlechtere Latenz als Polling. |
| **Timer optional**, mit Jitter | Braucht nur, wer externe Daten zieht (Webmentions, Kommentare). Ohne Timer erscheinen eingehende Kommentare erst beim nächsten Vault-Edit. Jitter 0–10 %, damit nicht alle Installationen gleichzeitig bei externen APIs aufschlagen. Bei aktivem Timer ist „kein Build seit 2 × Intervall" ein Liveness-Signal für `/healthz`. |
| **`queue_latest`** statt `restart` | Ein Abbruch zwischen Swap und Post-Publish hinterlässt einen Zustand, aus dem es keinen sauberen Weg zurück gibt. Ersetzt `cancel-in-progress: true` aus dem alten Workflow. |

### 3.2 Sicherheit

| Entscheidung | Begründung |
|---|---|
| **Skripte und Build-Config niemals im Vault** | Der Build ist per Design Code-Ausführung. Läge `build.sh` oder `zensical.toml` im Nextcloud-Ordner, hätte jeder mit Schreibzugriff Shell auf dem Server: ein kompromittiertes Handy, ein alter Sync-Client, jede Person, mit der der Ordner je geteilt wird. GitHub Actions hatte Branch Protection und Audit-Log davor — das ersatzlos wegzuwerfen wäre ein Rückschritt. |
| **`overrides/` (custom_dir) gehört zur Code-Seite** | Jinja-Templates sind ausführbarer Code. |
| **Vault enthält nur `docs/`** | Alles andere (`pyproject.toml`, `uv.lock`, `zensical.toml`, `overrides/`, `src/obsidian_md/`, Hooks) kommt aus `/etc/ncpages/`. `docs/stylesheets/extra.css` bleibt im Vault — reines CSS, harmlos, dafür ohne SSH änderbar. |
| **Overlap-Check ist fail-closed** | Liegt das Hook-Verzeichnis innerhalb von `source.path`, verweigert ncpages den Start mit erklärender Fehlermeldung. Nicht warnen — jemand wird es sonst aus Bequemlichkeit tun. |
| **Watcher und Builder getrennt** | Watcher hat Nextcloud-Credentials und Netz, keine Build-Tools. Builder hat Build-Tools, keine Credentials, kein Egress (`internal: true`), `read_only`, `cap_drop: ALL`, non-root, Speicher- und Zeitlimit. Trigger über internen HTTP-Endpunkt mit Shared Token, kein Docker-Socket. |

### 3.3 Build

| Entscheidung | Begründung |
|---|---|
| **Dependencies ins Image gebacken** | `uv sync --frozen --no-install-project --group dev` im Dockerfile. Zur Laufzeit läuft `uv` nicht mehr → Builder ist wirklich netzlos. Der ursprünglich geplante `install_script`-Hook entfällt. Dependency-Änderung = bewusster Image-Rebuild statt stillem Drift. Bei `zensical 0.0.x` mit möglichen Breaking Changes ist das eine Verbesserung. |
| **`PYTHONPATH=/work/build/src`** statt editable Install | `obsidian_md` ist eine Python-Markdown-Extension, die Zensical per Modulnamen lädt. Editable Installs schreiben absolute Pfade in `.pth`-Dateien und brechen beim Volume-Wechsel. |
| **`python:3.13-slim`, nicht Alpine** | `requires-python = ">=3.13"`. Zensical ist maturin-basiert; ohne `musllinux`-Wheels kompiliert pip den Rust-Teil aus dem Source. Image-Größe ist hier irrelevant. |
| **Kein Preprocessing-Schritt nötig** | `obsidian_md` läuft als Markdown-Extension *innerhalb* des Builds, nicht davor. Wikilinks und Embeds sind damit abgedeckt. |

### 3.4 Navigation

| Entscheidung | Begründung |
|---|---|
| **Nav aus Obsidian-Frontmatter** (Variante d) | Der bisherige explizite `nav`-Baum in der `zensical.toml` hätte bedeutet: neue Notiz erscheint nicht in der Navigation, bis jemand per SSH die Config editiert. Das ist schlechter als der git-Workflow, den wir ablösen. |
| Kern bleibt frei davon | Die Aggregation ist ein `pre_build`-Hook im Rezept, genau wie die Webmentions. Bei Variante (c) hätte der Kern ein validierendes `merge_fragment`-Primitive gebraucht; bei (d) kommt aus dem Vault nur Frontmatter in normalen Notizen — Angriffsfläche null. |
| URLs bleiben stabil | Navigation ist von Dateipfaden entkoppelt. Umsortieren bricht keine Links — relevant, weil Webmentions auf URLs zeigen. |

**Konvention.**

```yaml
---
title: Bounded Context
nav: Architecture & Strategy/Domain-Driven Design
nav_order: 130
---
```

* Separator `/`, alternativ Liste für Titel mit Schrägstrich.
* **Gruppen ohne Datei** („Requirement Management") sortieren nach dem Minimum
  der `nav_order` ihrer Nachkommen, bei Gleichstand alphabetisch.
* **Section-Index:** `nav` exakt gleich dem Gruppenpfad → Index-Seite der Gruppe
  (passt zu aktiviertem `navigation.indexes`).
* **Kein `nav:`** → Seite wird gebaut, ist per Link erreichbar, steht nicht im
  Menü. Für einen Digital Garden legitim; Waisen stehen im Status-Report.
* **`draft: true`** → komplett ausgeschlossen. Ersetzt die Preview-Funktion, die
  mit den Pull Requests wegfällt.
* `nav_order` in Zehnerschritten, damit Einfügen ohne Umnummerieren geht.
  Kollisionen sortieren alphabetisch nach Titel.

**Status:** Migration und Aggregator sind gebaut und getestet
(`migrate_nav.py`, `nav_from_frontmatter.py`, `nav_lib.py`). Round-trip über
alle 46 Seiten byte-identisch, auch bei zufällig gemischter Eingabereihenfolge.

### 3.5 Auslieferung

| Entscheidung | Begründung |
|---|---|
| **ncpages liefert selbst aus** | Ein schmaler Webserver (`static-web-server`, Rust, ~5 MB) im Stack. Der Nutzer proxied darauf wie auf jeden anderen Container. Damit entfallen alle Remote-Publish-Backends, SSH-Keys, Deploy-Secrets und die gesamte Atomaritätsdiskussion. |
| **Symlink-Swap als einziges Backend** | `rename(2)` innerhalb eines Dateisystems ist atomar. Über Netzwerkprotokolle gibt es kein „vertausche zwei Verzeichnisse atomar" — genau daher stammt das Halb-Deploy-Problem von `git-pages-cli --upload-dir`. |
| **Webserver ohne `depends_on`** | Fallen Watcher oder Builder aus, bleibt die Site live. Nur der Webserver-Container ist ein echter Single Point of Failure, und er hat keine Abhängigkeiten. |
| **`releases/` ersetzt den Build-Cache** | Der `actions/cache`-Tanz existierte nur, weil GitHub Actions zustandslos ist. `oldDir` für den Webmention-Diff ist `readlink current`. Retention 5 gibt zusätzlich Rollback. |
| **`blog-src` ist persistente Working Copy**, kein Cache | Nextcloud ist nur *ein* Aktualisierungsmechanismus. Ist die Nextcloud down, laufen Timer-Builds weiter — Kommentare werden weiter geholt und publiziert. |

### 3.6 Stack-Topologie

| Entscheidung | Begründung |
|---|---|
| **Eigener Compose-Stack**, unabhängig von Nextcloud | Blog neu deployen, ohne die Nextcloud anzufassen. |
| **Drittes, extern angelegtes Netz** (`cloud-bridge`) | Das Default-Netz des Nextcloud-Stacks als `external` einzubinden bricht bei jedem `docker compose down` des NC-Stacks (Netz wird gelöscht und mit neuer ID neu angelegt). Ein manuell angelegtes Netz überlebt das, hat ein festes Subnetz für `trusted_proxies` und macht die Abhängigkeitsrichtung explizit. |
| Nur `nginx` und `notify-push` hängen im Brückennetz | Der Blog erreicht die Nextcloud-API, nicht deren Datenbank. |
| **`NEXTCLOUD_URL` zeigt auf den nginx-Container** | `nextcloud:stable-fpm-alpine` spricht FastCGI, kein HTTP. Gilt für notify_push und für den WebDAV-Zugriff des Watchers. |
| **`host_header`-Option** | Intern über `http://nginx`, aber `Host:` = echte Domain, damit `server_name`-Matching und `trusted_domains` greifen. |

---

## 4. Topologie

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

**Volumes**

| Volume | Inhalt | Schreiber | Leser |
|---|---|---|---|
| `src` | Vault-Working-Copy (`docs/`) | watcher | builder (ro) |
| `releases` | `build/`, `releases/<id>/`, `current` | watcher, builder | web (ro) |
| `state` | letzter ETag, Content-Hash, Build-Historie | watcher | — |

**Arbeitsbaum** (auf demselben Volume wie `releases/`, damit `mv` atomar ist):

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

**Bind-Mount des Webservers:** `releases:/site:ro`, Root auf `/site/current`.
Niemals den Symlink selbst mounten — Docker löst Mount-Quellen beim Containerstart
auf, der Swap wäre dann unsichtbar und die Site würde stillschweigend nie
aktualisiert. Open-File-Caching im Webserver aus demselben Grund aus.

---

## 5. Schnittstellen

### 5.1 Config (`/etc/ncpages/ncpages.toml`)

```toml
schema_version = 1

[source]
kind          = "webdav"
url           = "http://nginx"
host_header   = "cloud.example.org"
path          = "Obsidian/netzmuffel"
user          = "lars"
password_file = "/run/secrets/nc_app_password"
required      = false          # Start auch bei unerreichbarer Quelle

[triggers]
push  = "ws://notify-push:7867/ws"
poll  = "30s"
timer = "6h"                   # weglassen = aus
jitter = 0.1

[schedule]
debounce  = "10s"
max_delay = "120s"
on_busy   = "queue_latest"

[assemble]
overlay = ["zensical.toml", "pyproject.toml", "uv.lock", "overrides", "src"]
source_subdir = "docs"

[build]
url     = "http://builder:8080"
timeout = "10m"
output  = "site"

[[hooks.pre_build]]
run = "nav_from_frontmatter.py"
[[hooks.pre_build]]
run = "fetch_comments.py"
env_passthrough = ["WEBMENTION_IO_TOKEN"]

[[hooks.post_publish]]
run = "send_webmentions.sh"

[gate]
require_files   = ["index.html", "sitemap.xml"]
min_pages       = 5
max_page_drop   = 0.4
max_nav_churn   = 10

[publish]
kind          = "symlink"
root          = "/work/releases"
keep_releases = 5

[report]
webdav_status_path = "Obsidian/_netzmuffel-status/build.md"   # AUSSERHALB source.path
ntfy_topic         = "https://ntfy.sh/..."
```

`report.webdav_status_path` muss außerhalb von `source.path` liegen, sonst
Endlosschleife: Status schreiben → Wurzel-ETag ändert sich → Trigger → Build →
Status schreiben. Pfad-Excludes helfen nicht, weil das Wurzel-ETag pfadblind ist.
Zusätzlich: Fingerprint des zuletzt selbst geschriebenen Zustands halten.

### 5.2 Hook-Kontrakt

| Phase | Netz | Secrets | Läuft | Zweck |
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

Exit-Codes: `0` = ok, `1` = Warnung (Build läuft, Report), `2` = Abbruch.

Kein Plugin-System, keine dynamischen Module. Skripte plus Env-Variablen —
eine Schnittstelle, die in fünf Jahren noch funktioniert.

### 5.3 Gate

Läuft nach dem Build, vor dem Swap. Bei Verletzung: nicht publishen,
`current` bleibt stehen, laut melden.

* Pflichtdateien vorhanden
* Mindestseitenzahl
* Seitenzahl-Rückgang gegenüber letztem Release unter Schwelle
  (fängt den Fall „Sync-Fehler, Vault halb leer, Build läuft mit Exit 0 durch")
* Nav-Diff unter Schwelle
* Keine doppelten Basisnamen (bricht sonst Wikilink-Auflösung in `obsidian_md`)
* Konfliktkopien (`* (conflicted copy *)`) gefiltert **und gemeldet** — eine
  Konfliktkopie heißt, dass gerade Arbeit verloren zu gehen droht

---

## 6. TODO

### Phase 0 — Vorbereitung (blockiert alles andere)

- [ ] `fetch_comments.py` einordnen: schreibt es nach `docs/` (`pre_build`) oder
      liest es `site/` (`post_build`)? `beautifulsoup4` deutet auf Letzteres.
      Im alten Workflow lief es *nach* dem Build — falls es nach `docs/` schreibt,
      hat der aktuelle Blog eine 12-h-Verzögerung bei Kommentaren.
- [ ] `google-genai` klären: hängt ein LLM-Call im Build-Pfad? Dann
      Nichtdeterminismus, Kosten bei jedem Timer-Lauf, weiteres Secret →
      Content-Hash-Caching nötig.
- [ ] Vault-Layout in Nextcloud anlegen: nur `docs/` (+ `docs/stylesheets/`)
- [ ] Nextcloud-App-Passwort für den Watcher anlegen
- [ ] `trusted_proxies` prüfen: Subnetz des nginx-Containers eingetragen?

### Phase 1 — notify_push

- [ ] `notify_push`-Sidecar in den NC-Stack (`icewind1991/notify_push`,
      `config.php` read-only mounten, `NEXTCLOUD_URL=http://nginx`)
- [ ] `occ app:install notify_push` und `occ notify_push:setup https://…/push`
- [ ] nginx: `map $http_upgrade $connection_upgrade` ins `http{}`,
      `location ^~ /push/` mit Upgrade-Headern und `proxy_read_timeout 900s`
      (`^~`, damit die Regex-Locations der NC-Config nicht dazwischenfunken;
      `Connection` über die `map`, weil notify_push auch normale HTTP-Endpunkte
      unter `/test/*` und `/metrics` bedient)
- [ ] Mit `test_client` verifizieren — inklusive des direkten internen Pfads
      `ws://notify-push:7867/ws`, den der Watcher nutzt
- [ ] `fastcgi_param HTTPS on;` im PHP-Location-Block prüfen

### Phase 2 — Netz und Skelett

- [ ] `docker network create --driver bridge --subnet 172.28.0.0/16 cloud-bridge`
- [ ] `nginx` und `notify-push` im NC-Stack ans Brückennetz hängen
- [ ] Makefile mit idempotentem `net`-Target (Compose kann keine
      stack-übergreifenden Abhängigkeiten)
- [ ] `/etc/ncpages/`-Verzeichnis anlegen, root-owned, mit `zensical.toml`
      (ohne `nav`), `pyproject.toml`, `uv.lock`, `overrides/`, `src/obsidian_md/`

### Phase 3 — Nav-Migration

- [ ] `migrate_nav.py --config zensical.toml --docs docs` als Dry-Run auf einer
      **Vault-Kopie**
- [ ] Ergebnis in Obsidian prüfen (Properties-Ansicht)
- [ ] `--apply`, dann `nav`-Array aus der `zensical.toml` entfernen
- [ ] Alte `zensical.toml` mit `nav` als Referenz aufheben, bis der erste
      Build durch ist
- [ ] `nav_from_frontmatter.py` nach `/etc/ncpages/hooks/`

### Phase 4 — Builder-Image

- [ ] Dockerfile: `python:3.13-slim`, `uv` auf konkreten Tag gepinnt,
      `uv sync --frozen --no-install-project --group dev`
- [ ] `PYTHONPATH=/work/build/src`, `PYTHONDONTWRITEBYTECODE=1`,
      `UV_CACHE_DIR=/tmp/uv`, tmpfs auf `/tmp`
- [ ] `static-webmentions` mit Version **und sha256** ins Image
      (aktuell im Workflow: `latest`, ungeprüft, als root — zusammen mit den
      Deploy-Credentials im selben Job)
- [ ] HTTP-Agent mit `/build` und `/healthz`, Token-Auth
- [ ] Feste UID, identisch zum Watcher (sonst `EACCES` auf `releases/`
      beim zweiten Build)
- [ ] `read_only: true` verifizieren — Python braucht schreibbares `/tmp`

### Phase 5 — Kern

- [ ] `Source`-Trait: `webdav` (ETag `Depth: 0` → `Depth: 1` absteigend),
      `fs` (notify + debouncer). Beide in denselben Event-Channel.
- [ ] notify_push-Client (`tokio-tungstenite`), Reconnect mit Backoff,
      Poll läuft parallel als Sicherheitsnetz weiter
- [ ] Timer-Source mit Jitter
- [ ] Zustandsmaschine: `Idle → Dirty{deadline, hard_deadline} → Fetch →
      Assemble → Build → Gate → Publish → Idle`, `queue_latest`,
      max. 1 laufend + 1 wartend
- [ ] Persistenz: ETag, Content-Hash, letzter erfolgreicher Build auf `state`
      (sonst Vollbuild nach jedem `compose up`, und die Reconcile-Logik bleibt
      ungetestet, bis sie gebraucht wird)
- [ ] Reconcile beim Start
- [ ] Assemble: Overlay aus `/etc/ncpages` + `docs/` aus dem Vault
- [ ] Hook-Runner mit den vier Phasen und dem Env-Kontrakt
- [ ] Gate
- [ ] Symlink-Swap + Retention
- [ ] Bootstrap: fehlt `current`, Holding-Page-Release anlegen
      (sonst 404 auf allem während des ersten Syncs)
- [ ] Status-Report: WebDAV + ntfy, mit Selbstauslöse-Schutz
- [ ] `/healthz`: letzter Build, Sekunden seit letztem Check, Source-Status,
      `degraded` bei unerreichbarer Quelle
- [ ] Backoff bei 503 (NC-Maintenance-Mode), sofortiger Stopp bei 401
      (Brute-Force-Schutz)
- [ ] **Overlap-Check fail-closed**: Hook-Verzeichnis innerhalb `source.path`
      → Start verweigern

### Phase 6 — Cutover

- [ ] Caching-Header: `immutable` + langes `max-age` für `assets/`,
      `no-cache` für HTML (Zensical erzeugt gehashte Asset-Namen).
      Falsch gesetzt = altes HTML mit neuem CSS nach dem Swap.
- [ ] Reverse-Proxy-Block für netzmuffel.de auf `:8080`
- [ ] Parallelbetrieb: neuer Stack unter Testdomain, alter Workflow läuft weiter
- [ ] Ausgabe vergleichen (Seitenzahl, Sitemap, Stichproben)
- [ ] DNS umstellen, altes Ziel stehen lassen bis TTL durch
- [ ] GitHub-Workflow deaktivieren
- [ ] `GIT_PAGES_PASSWORD` und `WEBMENTION_IO_TOKEN` aus den GitHub-Secrets
      entfernen

### Phase 7 — Veröffentlichung (optional, danach)

- [ ] **Name entscheiden.** Nextcloud GmbH hält die Wortmarke. Ein Repo
      `nextcloud-pages` ist gängige Community-Praxis; ein *Produktname* mit Logo,
      Domain und Docker-Image kann nach offizieller Zugehörigkeit aussehen, und
      „Pages" verstärkt das (GitHub/GitLab/Codeberg Pages sind Erstanbieter-
      Features). Aktuelle Trademark-Policy prüfen. Sicher: eigenständiger Name
      plus beschreibender Untertitel. Jetzt entscheiden — ein Rename nach dem
      ersten Release kostet Image-Tags, Doku-Links und Sterne.
- [ ] Lizenz Apache-2.0 (kein Nextcloud-Code gelinkt, reine HTTP-Kommunikation →
      AGPL nicht gefordert; Patentklausel relevant bei Firmen-Deployments)
- [ ] Kern/Rezept trennen: `crates/ncpages` vs. `examples/zensical-obsidian`
- [ ] Weitere Rezepte: `quartz` (größte Obsidian-Publishing-Community,
      „Quartz ohne git" ist dort der fehlende Baustein), `hugo`,
      `mkdocs-material`
- [ ] **`ncpages doctor`**: die gesamte Red-Team-Liste als ausführbare Checks —
      WebDAV, App-Passwort, ETag-Propagation (fällt bei External Storage und
      manchen Group Folders durch), notify_push, `trusted_proxies`,
      Symlink-Mount, UID-Konsistenz, Builder-Egress, Config-Overlap,
      `base_url` vs. eingehender `Host`-Header
- [ ] Issue-Template, das `doctor`-Ausgabe verlangt (die meisten Issues werden
      fremde Deployments sein, nicht eigene Bugs)
- [ ] `THREAT_MODEL.md` **vor** der Installationsanleitung, `SECURITY.md`
      mit Kontaktweg
- [ ] Multi-Arch `amd64` + `arm64` (Zielgruppe ist Homelab; amd64-only
      halbiert die Nutzerbasis)
- [ ] Integrationstests gegen echte Nextcloud-Versionen als Compose-Matrix
- [ ] `docs/` baut sich selbst mit ncpages (Dogfooding als CI-Schritt)
- [ ] README: ehrliche Abgrenzung zu `watchexec` + Bash, und ehrlicher Satz
      zu Maintenance-Erwartung und Bus-Faktor

---

## 7. Offene Punkte

**`fetch_comments.py`** — die einzige Lücke, die die Pipeline-Reihenfolge
betrifft. Siehe Phase 0.

**git als interne Schicht.** Vorschlag, nicht entschieden: vor jedem Build
`git add -A && git commit` in ein lokales Bare-Repo. ~15 Zeilen. Gewinn: exakter
Diff („was hat diesen Build ausgelöst?"), reproduzierbare Rebuilds, `git bisect`
bei Layout-Regressionen, Backup unabhängig von Nextcloud. Ohne das ersetzt die
Nextcloud-Versionierung nur die Datei-Historie, ohne Commit-Grenzen.

**Draft-Vorschau.** `draft: true` schließt aus, zeigt aber nichts. Ein zweiter
Vault-Ordner mit eigenem Publish-Ziel hinter Basic Auth wäre der Ersatz für
PR-Previews. Zwei Sources, ein Kern.

**`/de/` zeigt ins Leere.** `[project.extra] alternate` deklariert Englisch und
Deutsch, aber es gibt keine i18n-Struktur und kein Übersetzungs-Plugin. Der
Sprachumschalter linkt auf 404. Kein Blocker — aber `google-genai` in den
Dev-Dependencies lässt vermuten, dass das der Plan ist, und es würde die
Build-Pipeline erheblich verändern.

---

## 8. Gefundene Bugs im aktuellen Workflow

Beide entfallen mit dem neuen Aufbau, sind aber bis zum Cutover aktiv:

**`cp -r site site-previous`.** `actions/cache` stellt `site-previous` mit
`restore-keys` wieder her, das Verzeichnis existiert also bereits. `cp -r` kopiert
dann *hinein* statt darüber: `site-previous/site/`, `site-previous/site/site/`,
und daneben liegt weiter die Kopie aus dem ersten Lauf. Der Webmention-Diff
vergleicht seit dem zweiten Lauf gegen einen teils veralteten, teils
verschachtelten Baum. `|| true` und `continue-on-error: true` haben verdeckt,
dass das nie richtig funktioniert hat. Sofort-Fix:
`rm -rf site-previous && cp -r site site-previous`.

**Zwei ungepinnte Binaries als root.** `static-webmentions` und `git-pages-cli`
werden mit `latest` ohne Checksum nach `/usr/local/bin` geladen und ausgeführt —
in einem Job, der die Deploy-Credentials in der Umgebung hat. `actions/setup-uv`
ist auf einen Commit-SHA gepinnt, diese beiden nicht.
