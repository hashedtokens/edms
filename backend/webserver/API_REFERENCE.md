# EDMS Backend API Reference — Test View, Collections, Dashboard

Base URL: `http://localhost:3000` (same whether running via `cargo run` or Docker).

---

## Running it

### Option A — plain Docker
```bash
cd backend
docker build -f webserver/Dockerfile -t edms-local .
docker run -d --name edms-local -p 3000:3000 edms-local
```
No data persistence — every `docker run` starts fresh. Fine for quick checks.

### Option B — Docker Compose (recommended, persists data across restarts)
```bash
cd backend
docker compose up --build -d
```
Stop it with `docker compose down`. `edms.db` lands at `backend/webserver/data/edms.db` on your host — you can open it directly with any SQLite tool (Docker auto-creates that folder, no manual setup needed). `edms_data`/`edms_root` persist via named Docker volumes (not directly browsable as host folders — use `docker cp` to pull files out if you need to look at them).

To wipe everything and start fresh:
```bash
docker compose down -v
rm -rf webserver/data
```

> **Two things worth knowing if you're on an older checkout:**
> 1. `edms.db` used to be bind-mounted as a single file (`./webserver/edms.db:/app/edms.db`) rather than a directory. On Docker Desktop for Windows, single-file bind mounts don't reliably sync writes back to the host — the container ran fine, but the host-side copy silently stayed empty forever. Mounting a directory instead fixed it.
> 2. Separately, any endpoint that opens its own SQLite connection per request (not the app's shared one — e.g. collections, tags) could fail with `CannotOpen` under Docker specifically, even after fix #1. Docker Desktop for Windows bind mounts don't reliably support the shared-memory locking WAL mode needs for a second connection to the same file. Fixed by switching `journal_mode` to `DELETE` in `base.rs`.

### Option C — native (Rust toolchain required)
```bash
cd backend/webserver
cargo run --bin rust-webserver
```

### Inspecting the data directly (no SQLite CLI needed)
```bash
python -c "import sqlite3; c = sqlite3.connect('webserver/data/edms.db'); print(c.execute('SELECT * FROM endpoints').fetchall())"
```
Swap the table name (`endpoints`, `tags`, `bookmarks`, `history`, `collections`) or the db path as needed. For a collection's own file, pull it out of the volume first: `docker cp backend-webserver-1:/app/edms_root/storage/collections/<name>.sqlite .` then point the same command at it (table name is `membership` there).

---

## The one rule that matters for WebSocket routes

**WebSocket only notifies — REST delivers the actual data.** Every WS connection below streams messages shaped like:
```json
{ "type": "event", "event": { "type": "<EventName>", "payload": { ... } } }
```
Treat these as "something changed, go re-fetch," not as the final source of truth for anything except live progress (test timers).

---

## Views

Static, no-param metadata routes — mostly a place for the frontend to sanity-check which view it's on.

| Method | Path | Type | Notes |
|---|---|---|---|
| GET | `/home` | REST | `{"view":"home", ...}` |
| GET | `/test-view` | REST | `{"view":"test-view", ...}` |
| GET | `/list-view` | REST | `{"view":"list-view", ...}` |

---

## Endpoints

The only way endpoint definitions currently enter the system — Import (below) extracts files to disk but does not create DB rows yet.

| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/endpoints/create` | `{"endpoint_id","endpoint_str","annotation"?,"method"?}` | `method` optional — one of `GET/POST/PUT/PATCH/DELETE` if given; an endpoint created without one shows as unclassified in the CRUD Operations dashboard breakdown. Rejects a duplicate `endpoint_id` cleanly (400, not a 500) |
| POST | `/endpoints/:endpoint_id/delete` | — | Does **not** cascade — orphaned bookmarks, collection memberships, and history/request/response data can be left behind (see Known limitations) |
| POST | `/endpoints/:endpoint_id/annotation` | `{"annotation"}` | Sets/replaces the endpoint's annotation after creation (previously create-time-only). 404 if the endpoint doesn't exist. Broadcasts `EndpointAnnotationUpdated` on the shared WS channel (same one `/test-view/run` uses) so open List/Test Views know to re-fetch |
| GET | `/endpoints/lookup?endpoint_str=&method=` | — | Looks up an endpoint by its exact `(endpoint_str, method)` pair — lets a caller check "does this already exist" before deciding whether to pass an existing `endpoint_id` vs. let a fresh one get allocated. 404 if none exists |

---

## Test View

| Method | Path | Type | Body / Notes |
|---|---|---|---|
| GET | `/test-view/endpoints/load` | WS | Sends a snapshot of all endpoints on connect, then streams events |
| GET | `/test-view/bookmarks/load` | WS | Same, for the `active` bookmark set |
| GET | `/test-view/history/load` | WS | Same, for history — sends `{"type":"snapshot","history":[{"id","endpoint_id","action","details","timestamp"}]}` on connect, then streams events |
| GET | `/test-view/run` | WS | Send `{"type":"run_test","payload":{"endpoint_id"?,"endpoint_str","method","body","timeout_ms","tick_interval_ms","headers"?,"annotation"?}}` to start a test. **`endpoint_id` is optional as of 2026-09-08** — an endpoint is only created the moment it's tested: omit it to auto-allocate a fresh canonical EID, pass an existing one to re-test it (the common case), or a not-yet-existing one to create it with that exact id. `endpoint_str` is always required (used to create the row if it doesn't exist yet; ignored — the stored value wins — if it does). `annotation` is used only when this run creates a new endpoint. Streams `TestStarted` → `TimerTick`s → `TestFinished`/`TestTimeout` |
| POST | `/test-view/stop` | REST | Body `{"endpoint_id","request_number"}` — cancels the app's tracking of an in-flight test (does not kill the underlying HTTP call already running) |
| POST | `/test-view/save/history` | REST | Body `{"endpoint_id","action","details"}` — manual history entry (History also now auto-records on every completed test, no manual call needed for that case) |
| POST | `/test-view/save/bookmark` | REST | Body `{"endpoint_id","notes"}` — bookmarks into `active`. **Requires a collection to be loaded first** (`/bookmarks/:collection/load`) as of 2026-09-08 — 400 `{"error":"no_collection_loaded","message":"Load a collection first"}` otherwise |
| GET | `/test-view/:bookmark/add` | WS | Send `{"endpoint_id"}` — adds to the named folder. Same "must have a collection loaded" gate as above when `:bookmark` is `active` |
| GET | `/test-view/:bookmark/delete` | WS | Send `{"endpoint_id"}` — removes from the named folder entirely (ungated — clearing your own workspace never requires a loaded collection). For `active`, this is "delete from bookmark view"; it does not touch collection membership |
| GET | `/test-view/:endpoint_id/request/:request_number` | REST | Fetch a saved request body |
| GET | `/test-view/:endpoint_id/response/:request_number` | REST | Fetch a saved response body |
| GET | `/test-view/:endpoint_id/headers/:request_number` | REST | Fetch saved headers — body is `{"request_headers":{...},"response_headers":{...}}` |
| GET | `/test-view/:endpoint_id/qps` | REST | Lists every QP pair (test run) saved for this endpoint, oldest first: `{"ok":true,"qps":[{"request_number","method","timestamp","status_code","response_time_ms"}]}`. `status_code`/`response_time_ms` are `null` if the response hasn't landed yet |
| POST | `/test-view/:endpoint_id/qps/:request_number/delete` | REST | Deletes one QP pair — its `request_metadata`/`response_metadata` rows and the three saved JSON files (request/response/headers). 404 if it doesn't exist. Broadcasts `QpDeleted` on the shared WS channel (same one `/test-view/run` uses) so open views know to re-fetch the list above |
| POST | `/test-view/history/clearall` | REST | Wipes all history |
| POST | `/test-view/bookmark/clearall` | REST | Wipes the `active` bookmark set |

**Bookmarks ↔ Collections (merged as of 2026-09-08 — System 1 is retired):**

"Active bookmarks" is now always the draft state of whichever real Collection is currently loaded — there's no more free-floating or arbitrarily-named bookmark-folder-as-collection. Flow: create an empty collection (Collections table below) → load it here → bookmark tested endpoints into `active` (Test View section above) → per-endpoint, save into or unsave from the loaded collection.

| Method | Path | Type | Notes |
|---|---|---|---|
| GET | `/bookmarks/:collection/load` | WS | Loads a collection's members into `active`, **resolving full endpoint data, not just EIDs** (backs up whatever was in `active` first, into the single rolling `__session_backup__` slot — see Known limitations). Sets this as the loaded collection for every gated action below. Then streams events |
| POST | `/bookmarks/active/:endpoint_id/save` | REST | Persists a bookmarked endpoint's membership (EID + timestamp only) into the loaded collection. 400 if nothing's loaded, or if `:endpoint_id` isn't currently bookmarked in `active` |
| POST | `/bookmarks/active/:endpoint_id/unsave` | REST | Drops that endpoint's membership from the loaded collection — **stays bookmarked/visible in `active`** afterward, only the collection membership is removed. 400 if nothing's loaded |

`GET /test-view/bookmarks/load`'s snapshot now also carries `"active_collection": <name or null>` and, per bookmark entry, `"in_collection": true/false` (computed by cross-referencing the loaded collection's membership set — never stored redundantly) and `"updated": <timestamp or null>` (when that endpoint was bookmarked into the active workspace — previously always null; the timestamp existed in the `bookmarks` table but was never selected).

---

## Collections (per-collection files)

Each collection is its own real file (`storage/collections/{name}.sqlite`), holding just `endpoint_id` + `added_at`. The endpoint's actual data always stays in the central `endpoints` table — Collections never copies it. **A collection is always created empty** — the only way an endpoint becomes a member is the bookmark flow above (`/bookmarks/active/:endpoint_id/save`); there is no longer a direct "add any endpoint to any collection" route.

| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/collections/create` | `{"name","annotation"?}` | Creates the catalog row + the real file, empty. `annotation` is optional |
| GET | `/collections/list` | — | All collections, each with `annotation` (`null` if unset) and `endpoint_count` (`null` if the collection has no file yet) |
| GET | `/collections/:name` | — | One collection's catalog row, including `annotation` and `endpoint_count`; 404 if missing |
| POST | `/collections/:name/rename` | `{"new_name"}` | Renames the catalog entry + moves the file; rejects a name collision cleanly, no data loss |
| POST | `/collections/:name/annotation` | `{"annotation"}` | Sets/replaces the collection's annotation. 404 if the collection doesn't exist |
| POST | `/collections/:name/delete` | — | Removes the catalog row + deletes the file |
| POST | `/collections/:name/endpoints/remove` | `{"endpoint_id"}` | Direct removal stays available — removal doesn't carry the same "must be deliberately curated via testing" risk as addition |
| GET | `/collections/:name/endpoints` | — | Lists members with `added_at` |
| POST | `/collections/:name/tags/import` | `{"tags":[...],"export_existing_tags"?}` | The "Add to Collection" move flow — finds every endpoint carrying any of the given tags (read-only against the central tags table), adds them as members of this collection, and — if `export_existing_tags` is true — copies each one's current tags into this collection's own per-endpoint tag record (capped at 25 tags/endpoint; excess is silently skipped and counted in `tags_skipped_cap`). Central tags table is never modified. Returns `{"endpoints_matched","endpoints_added","tags_exported","tags_skipped_cap"}` |
| GET | `/collections/:name/tags/endpoints` | — | Lists every `(endpoint_id, tag)` pair this collection carries from the import route above — distinct from the central tags table and from the collection-wide tag rollups below |

**Tag rollups** (global count per tag, not per-collection membership):

| Method | Path | Body |
|---|---|---|
| POST | `/collections/tags/create` | `{"name","endpoint_ids"}` |
| POST | `/collections/tags/delete` | `{"names"}` |
| POST | `/collections/tags/rename` | `{"old_name","new_name"}` |
| GET | `/collections/tags/list` | — |

**Membership-tags** (a different concept — tracks which tags a collection has, used for merge classification, not endpoint membership):

| Method | Path | Body |
|---|---|---|
| POST | `/collections/:name/membership-tags/add` | `{"tag"}` |
| POST | `/collections/:name/membership-tags/remove` | `{"tag"}` |
| GET | `/collections/:name/membership-tags` | — |
| GET | `/collections/by-tag/:tagname` | — |

---

## Data View (folder management)

Manages "folders" under `edms_data`/`edms_root` — a separate, older filesystem-folder concept from Collections above. All fire-and-forget except `active`.

| Method | Path | Type | Notes |
|---|---|---|---|
| POST | `/dataview/:folder/delete` | REST | Deletes the folder from disk immediately (not fire-and-forget — this one's synchronous) |
| POST | `/dataview/:folder/merge` | REST | 202 immediately; spawns an `export_merge` child task, result arrives via `/internal/callback` → `ExportReady` event |
| GET | `/dataview/:folder/active` | WS | Marks the folder active (in-memory + spawns a child to sync it to disk), broadcasts `FolderBecameActive`, then streams events |

---

## Tags (per-endpoint)

A different table from Collections' tag rollups above — tracks tags directly on an `endpoint_id`.

| Method | Path | Body |
|---|---|---|
| GET | `/tags/popular` | — returns `[{"tag","count"}]` |
| GET | `/tags/:endpoint_id` | — returns `{"tags":[...]}` |
| POST | `/tags/:endpoint_id/add` | `{"tag"}` |
| POST | `/tags/:endpoint_id/remove` | `{"tag"}` |

---

## Webview / Repoview

Same catalog pattern as Collections (register a name, list, per-view tag rollups) but **not** as far along — no independent SQLite file per instance yet (`file_path` stays `null`), and no endpoint-membership routes (no `webview/:name/endpoints/add` equivalent exists).

| Method | Path | Body |
|---|---|---|
| POST | `/webview/create` | `{"name"}` |
| GET | `/webview/list` | — |
| POST | `/webview/tags/create` | `{"name","endpoint_ids"?}` |
| POST | `/webview/tags/delete` | `{"names"}` |
| POST | `/webview/tags/rename` | `{"old_name","new_name"}` |
| GET | `/webview/tags/list` | — |
| POST | `/repoview/create` | `{"name"}` |
| GET | `/repoview/list` | — |
| POST | `/repoview/tags/create` | `{"name","endpoint_ids"?}` |
| POST | `/repoview/tags/delete` | `{"names"}` |
| POST | `/repoview/tags/rename` | `{"old_name","new_name"}` |
| GET | `/repoview/tags/list` | — |

---

## Repo Export / Import

| Method | Path | Notes |
|---|---|---|
| GET | `/repo/:collection/:filename/export` | Returns markdown immediately (built from the endpoints already fetched for the collection); also fires `export_collection` (zip packaging) and `generate_markdown` child tasks in the background — result arrives via `ExportReady` |
| POST | `/repo/:collection/:filename/import` | 202 immediately; unzips the file at the path `export` writes to (`edms_root/exports/:filename`) back into the path `export` reads its source from (`edms_root/endpoints/reports/:collection`). Result arrives via `/internal/callback` → `ImportReady`. **Only extracts files to disk — does not parse them back into the DB** (no endpoint/bookmark rows are created from an import; that reconciliation isn't built yet) |

---

## Logs

| Method | Path | Notes |
|---|---|---|
| GET | `/logs` | Plain-text tail of `app.log` (last 500 lines) |

---

## Internal — not for frontend use

| Method | Path | Notes |
|---|---|---|
| POST | `/internal/callback` | edms-child → webserver callback channel. Every `ipc::spawn_child` task's result lands here and gets routed to a task-specific handler internally |

---

## Dashboard

| Method | Path | Notes |
|---|---|---|
| GET | `/dataview/dashboard` | Live counts: `{active_folder, endpoints, bookmarks, history}` |
| GET | `/dashboard/snapshot` | Latest periodic snapshot (endpoint/bookmark/tag counts, db size, storage size) |
| GET | `/dashboard/snapshot/history` | Every retained snapshot, oldest first (30-day rolling window) — for trend charts |
| GET | `/dashboard/static` | Config-driven static info: limits, stability/commit info, links |
| GET | `/dashboard/crud-operations` | Breakdown by entity type + HTTP method; 404 until the first refresh runs |
| POST | `/dashboard/crud-operations/refresh` | Fire-and-forget — triggers a recompute, result lands via internal callback |
| GET | `/dashboard/compare?from=YYYY-MM-DD&to=YYYY-MM-DD` | Day-over-day comparison between two daily snapshots |

**Known gap:** Dashboard has no WebSocket connection of its own yet — none of the above pushes live updates. Poll, or re-fetch after triggering a refresh.

---

## Known limitations worth knowing before integrating

- Bookmark actions don't validate that an endpoint exists before bookmarking it (Collections does).
- No size limits enforced anywhere (Collections count, endpoints-per-list, History/Bookmarks caps).
- Deleting an endpoint doesn't cascade — orphaned bookmarks, collection memberships, and history/request/response data can be left behind.
- A QP (request/response pair — see Test View above) is generated automatically by every test run, not created/edited by hand. There's no route to edit a QP's saved request/response in place, only to list and delete.
- Import (`/repo/:collection/:filename/import`) only extracts a zip to disk — it does not create/update endpoint, bookmark, or collection-membership DB rows from the imported files.
- Webview/Repoview have no independent per-instance SQLite file yet (unlike Collections) and no endpoint-membership routes at all.
- The session backup used by `/bookmarks/:collection/load` (`__session_backup__`) is a **single rolling slot, not per-collection**, by design (confirmed, 2026-09-08): load A, leave bookmarks unsaved, load B, load A again — A's unsaved bookmarks are gone, overwritten when B loaded. Only the most recent switch is protected.
- `active_collection` is in-memory (`AppState`, not persisted) — a webserver restart forgets which collection was loaded, even though the `active` bookmarks themselves are still in the DB. The next bookmark-add attempt will correctly demand a fresh `/bookmarks/:collection/load` rather than silently misbehaving, but this is worth knowing.
