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

## EQP report export

| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/reports/:endpoint_id/export` | `{"format":"pdf|html|md","output_filename"?}` | Reads the EID's request, response, and header JSON files from the configured `edms-data/storage/globalEQPData/` folder, combines them with endpoint metadata and tags, and sends the resulting payload to the compute converter. Missing, unreadable, or invalid QP JSON files become null in the affected field; available fields and other QPs still export. Returns `202 Accepted`; the timestamped file is written under `edms-data/takeout/PDF`, `HTML`, or `MD`. |

---

## Test View

| Method | Path | Type | Body / Notes |
|---|---|---|---|
| GET | `/test-view/endpoints/load` | WS | Sends a snapshot of all endpoints on connect, then streams events |
| GET | `/test-view/:collection/bookmarks/load` | WS | Same, for a specific collection's bookmark set — see the multi-collection redesign note below |
| GET | `/test-view/history/load` | WS | Same, for history — sends `{"type":"snapshot","history":[{"id","endpoint_id","action","details","timestamp"}]}` on connect, then streams events |
| GET | `/test-view/run` | WS | Send `{"type":"run_test","payload":{"endpoint_id"?,"endpoint_str","method","body","timeout_ms","tick_interval_ms","headers"?,"annotation"?}}` to start a test. **`endpoint_id` is optional as of 2026-09-08** — an endpoint is only created the moment it's tested: omit it to auto-allocate a fresh canonical EID, pass an existing one to re-test it (the common case), or a not-yet-existing one to create it with that exact id. `endpoint_str` is always required (used to create the row if it doesn't exist yet; ignored — the stored value wins — if it does). `annotation` is used only when this run creates a new endpoint. Streams `TestStarted` → `TimerTick`s → `TestFinished`/`TestTimeout` |
| POST | `/test-view/stop` | REST | Body `{"endpoint_id","request_number"}` — cancels the app's tracking of an in-flight test (does not kill the underlying HTTP call already running) |
| POST | `/test-view/save/history` | REST | Body `{"endpoint_id","action","details"}` — manual history entry (History also now auto-records on every completed test, no manual call needed for that case) |
| POST | `/test-view/save/bookmark` | REST | Body `{"collection","endpoint_id","notes"}` — bookmarks into the named collection. `collection` is required (400 if missing) — as of 2026-09-22 there's no more implicit "loaded" collection to fall back to |
| GET | `/test-view/:collection/add` | WS | Send `{"endpoint_id"}` — bookmarks into the named collection. `:collection` is always a real collection name now, no more `active` alias |
| GET | `/test-view/:collection/delete` | WS | Send `{"endpoint_id"}` — removes the bookmark entirely (does not touch collection membership) |
| GET | `/test-view/:endpoint_id/request/:request_number` | REST | Fetch a saved request body |
| GET | `/test-view/:endpoint_id/response/:request_number` | REST | Fetch a saved response body |
| GET | `/test-view/:endpoint_id/headers/:request_number` | REST | Fetch saved headers — body is `{"request_headers":{...},"response_headers":{...}}` |
| GET | `/test-view/:endpoint_id/qps` | REST | Lists every QP pair (test run) saved for this endpoint, oldest first: `{"ok":true,"qps":[{"request_number","method","timestamp","status_code","response_time_ms"}]}`. `status_code`/`response_time_ms` are `null` if the response hasn't landed yet |
| POST | `/test-view/:endpoint_id/qps/create` | REST | Creates a QP pair by hand — no real HTTP test runs. Body: `{"method","request_body","response_body"}` (the two bodies are JSON-encoded strings, same shape a saved file already round-trips as). Writes the request/response files directly, plus an empty headers file (`{"request_headers":{},"response_headers":{}}`) so `GET .../headers/:n` doesn't 404 for it. `status_code`/`response_time_ms` are left `null` — there's no real response. 404 if the endpoint doesn't exist. Response: `{"ok":true,"request_number"}`. Broadcasts `QpCreated` on the shared WS channel |
| POST | `/test-view/:endpoint_id/qps/:request_number/update` | REST | Overwrites an existing QP's request/response bodies in place. Body: `{"request_body","response_body"}`. Leaves `status_code`/`response_time_ms` and the headers file untouched. 404 if that QP doesn't exist. Broadcasts `QpUpdated` on the shared WS channel |
| POST | `/test-view/:endpoint_id/qps/:request_number/delete` | REST | Deletes one QP pair — its `request_metadata`/`response_metadata` rows and the three saved JSON files (request/response/headers). 404 if it doesn't exist. Broadcasts `QpDeleted` on the shared WS channel (same one `/test-view/run` uses) so open views know to re-fetch the list above |
| POST | `/test-view/history/clearall` | REST | Wipes all history |
| POST | `/test-view/:collection/bookmark/clearall` | REST | Wipes that collection's bookmark set only, not every collection's |

**Bookmarks ↔ Collections — multiple collections at once (redesigned 2026-09-22):**

Previously, "active bookmarks" was the draft state of whichever single Collection was loaded into one shared server-side value (`active_collection`) — only one collection could ever be loaded anywhere, for every connected client at once, so opening a second collection in another tab silently evicted the first. Now every bookmark route takes the collection explicitly (path param or body field), and the central `bookmarks` table's `folder` column holds the real collection name directly instead of one shared `__active__` bucket. Two tabs can have two different collections open with no interference, and two tabs on the *same* collection both see the same live updates via `BookmarksUpdated`/`CollectionMembershipUpdated`, which now carry a `collection` field to filter by. There's no more "load a collection first" gate, and no more session-backup mechanism — nothing is shared, so nothing needs backing up when switching.

| Method | Path | Type | Notes |
|---|---|---|---|
| GET | `/bookmarks/:collection/load` | WS | Snapshot of that collection's bookmark count, then streams events. Pure read now — no wipe, no copy, no backup |
| POST | `/bookmarks/:collection/:endpoint_id/save` | REST | Persists a bookmarked endpoint's membership (EID + timestamp only) into the named collection. 400 if `:endpoint_id` isn't currently bookmarked there. Broadcasts `CollectionMembershipUpdated { collection }` — previously this emitted nothing at all, so no other tab could ever know a save/unsave happened |
| POST | `/bookmarks/:collection/:endpoint_id/unsave` | REST | Drops that endpoint's membership from the named collection — **stays bookmarked** afterward, only the collection membership is removed. Same new broadcast as save |

`GET /test-view/:collection/bookmarks/load`'s snapshot carries `"collection": <name>` and, per bookmark entry, `"in_collection": true/false` (cross-referenced against that collection's own membership set) and `"updated": <timestamp>`.

**Cascades that come with storing bookmarks centrally by folder name, not a per-collection file:** deleting a collection also deletes its bookmarks (`DELETE FROM bookmarks WHERE folder = ?`); renaming a collection also renames its bookmarks' folder value, so they stay attached; deleting an endpoint also deletes all of its bookmarks, across every collection it was bookmarked into. None of these existed before this redesign — a deleted/renamed collection, or a deleted endpoint, used to leave orphaned rows behind.

---

## Collections (per-collection files)

Each collection is its own real file (`storage/collections/{name}.sqlite`), holding just `endpoint_id` + `added_at`. The endpoint's actual data always stays in the central `endpoints` table — Collections never copies it. **A collection is always created empty** — the only way an endpoint becomes a member is the bookmark flow above (`/bookmarks/:collection/:endpoint_id/save`); there is no longer a direct "add any endpoint to any collection" route.

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

## Webview

**v1.0 (Ravi, 2026-10-05/06): a WebView is the same list as a RepoView** ("UI is 99% identical for both"), so it has the same routes with `/webview` in place of `/repoview` and the same behaviour - see the Repoview section for what each one does. Its folder (`storage/webview/:name/`) holds the SQLite index (`webview.sqlite`) and `front-page.json` (an empty `{}` from the moment it is created, combined or imported, until the pop-up saves one), and no EQP data; row-level tags live in `webview_tag_memberships`, and `webview.source` records the Collection it was built from. The backend serves both from the same handlers (`handlers/view_flavor.rs`), so a fix to one is a fix to both, and a RepoView and a WebView with the same name don't interfere.

| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/webview/create` | `{"name","annotation"?,"source_collection","endpoint_ids"?}` | Builds the list from a Collection, exactly like `/repoview/create`. **Changed in v1.0:** it used to register an empty catalog row from `{"name"}` alone; `source_collection` is now required |
| GET | `/webview/list` | - | Same row shape as `/repoview/list` (camelCase UI fields plus the snake_case ones) |
| GET | `/webview/:name` | - | One WebView, same data as a list row |
| POST | `/webview/:name/rename` | `{"new_name"}` | Moves the folder (so `front-page.json` comes along) and carries its row tags |
| POST | `/webview/:name/annotation` | `{"annotation"}` | |
| POST | `/webview/:name/delete` | - | Removes the folder and its row tags |
| POST | `/webview/delete` | `{"names"}` | Bulk delete, one result per name |
| POST | `/webview/:name/duplicate` | `{"new_name"}` | Copies the folder (including `front-page.json`) and row tags |
| POST | `/webview/:name/convert-to-collection` | `{"collection","on_exists"?,"new_name"?}` | Same merge/rename 409 flow as RepoView |
| POST | `/webview/combine` | `{"name","sources"?,"tags"?,"annotation"?,"on_exists"?,"new_name"?}` | Combines WebViews (never mixes in RepoViews: a RepoView name is "not found" here). Same rules as `/repoview/combine` |
| POST | `/webview/:name/takeout` | `{"dest_name"?,"overwrite"?}` | Same as RepoView takeout (a job, 202); the takeout folder also holds `front-page.json` and the SQLite index is stripped (which is what compute's `validate_webview_format` requires: JSON only, no SQLite). Its manifest says `"kind":"webview"` |
| POST | `/webview/import` | `{"folder"?,"zip"?,"name"?,"replace_extracted"?}` | Same as RepoView import (a job, folder or zip) but from `storage/imports/uncompressed/webview/` or `storage/imports/compressed/webview/` (compute's folders for it). `front-page.json` is restored (`front_page_restored`; a takeout without one gets the default `{}`). A takeout of the other kind is refused |
| POST | `/webview/:name/membership-tags/add` | `{"tag"}` | Row-level tags |
| POST | `/webview/:name/membership-tags/remove` | `{"tag"}` | |
| GET | `/webview/:name/membership-tags` | - | |
| GET | `/webview/by-tag/:tagname` | - | `{"webviews":[...]}` |
| GET | `/webview/:name/front-page` | - | **Modify Frontpage** (the pop-up's load): `{ok, name, exists, front_page}`; a new WebView returns `{}`. 404 if the WebView doesn't exist |
| POST | `/webview/:name/front-page` | `{"front_page": <any JSON>}` | **Modify Frontpage** (save): replaces `front-page.json`. It holds the rich-text editor's JSON document (Shivanshu: `{"type":"doc","content":[...]}`); the backend only stores and returns JSON and does no HTML conversion - the frontend turns it back into editable content or into HTML. Any valid JSON value is accepted except `null` (400, it would read back as "no front page"). Up to 10 MB (editor documents can embed images); larger is refused with 413 |
| POST | `/webview/tags/create` | `{"name","endpoint_ids"?}` | Central tag-count rollup |
| POST | `/webview/tags/delete` | `{"names"}` | |
| POST | `/webview/tags/rename` | `{"old_name","new_name"}` | |
| GET | `/webview/tags/list` | - | |

A WebView has **no `tables` routes** (those generate a RepoView's `Tables-*.md`; a WebView's folder holds `front-page.json` instead). Reserved names are the same as RepoView's (`list`, `create`, `delete`, `import`, `combine`, `tags`, `by-tag`).

---

## Jobs and the Import/Export table

Takeout, import, unzip and format check work on many files, so they run in `edms-child` and the request that starts one returns **202** with a `job_id` at once. The child reports back through `/internal/callback`; the webserver finishes the job and broadcasts the result. Quick checks (a name that is taken, a bad request) are still answered in the response itself, before any job exists.

| Method | Path | Notes |
|---|---|---|
| GET | `/jobs` | Recent jobs, newest first, running ones included. Kept in memory (last 200, resets on restart) |
| GET | `/jobs/:id` | `{id, op, view, name, status, progress, result, error, age_ms}`. `status` is `running`, `done`, `failed`, or `stalled` (running over 30 minutes: the child most likely died without calling back). `op` is `takeout`, `import` or `format_check`. A **done** job can still carry problems in its `result` (`ok:false`, `skipped`, `missing_eqp_data`) - `failed` means nothing was produced |
| GET (WS) | `/jobs/ws` | On connect `{"type":"jobs","jobs":[...running...]}`, then only job events as `{"type":"event","event":{...}}`: `ViewIoProgress {job_id, op, view, name, progress:{step, done, total}}` while it runs, and `ViewIoDone {job_id, op, view, name, ok, result, error}` when it finishes (`ok` is whether the job completed; read `result` for what it found). Take the snapshot, then `GET /jobs/:id` for any job you started that isn't in it - it may already be done |
| GET | `/import-export/table` | What sits in `storage/imports/`, scanned live (nothing indexed), one row per zip or takeout folder: `{name, type:"Import", view:"compressed"\|"uncompressed", purpose, size, size_bytes, formatCheck:null, section, path, import_with}`. `purpose` comes from the folder the item is in (`repo/`, `webview/`, `collections/`). `import_with` says which route imports the row (`{route, folder}` or `{route, zip}`) |
| POST | `/import-export/check` | `{"section":"compressed"\|"uncompressed","path":"repo/name"}` (a row's `section` and `path`) -> **202** `{job_id}`. The job's `result` is `{passed, report:{passed, sqlite_status, edms_data_status, details}}` from compute's format check. A zip is unpacked with the strict extractor first. 400 for a path that isn't a listed row |

**Unzipping (`zip` imports and the format check).** The archive is untrusted, so the extractor refuses the *whole* archive if any entry could land outside the destination (`..`, absolute paths, backslashes, a `:` drive prefix, symlinks), or if it has over 100,000 entries or would unpack to over 20 GiB (declared and actual). It unpacks next to the destination and moves it into place only once it checks out (and holds a `repoview-manifest.json` at the top or inside the one folder that wraps everything), so a refused or failed zip leaves nothing behind.

**Not wired on purpose: compute's `move_item_to_view`.** It copies a raw folder to `storage/{repoview,webview,collections}/{name}`, and `storage/repoview` and `storage/webview` are where registered RepoViews and WebViews live, so a "move" could write into one - and it would create no catalog row, fresh EIDs or index. For RepoViews and WebViews, importing a row *is* the move.

**Format check and takeouts.** Compute's format check for a RepoView expects a SQLite database, but a v1.0 takeout has the index stripped, so `check` reports `passed:false` ("No SQLite database found") for the RepoView takeouts this API makes. That is expected (Ravi, 2026-10-07): a takeout is a one-time handoff of data into an unmonitored, unindexed folder, not something checked afterwards. WebView takeouts pass. The check is compute's existing behaviour and is reported as it is.


---

## Repoview

**v1.0 (Ravi, 2026-10-05/06): a RepoView is a list, not a copy of the data.** Its folder (`storage/repoview/:name/`) holds only a SQLite index (`repoview.sqlite`) and the generated `Tables-meta.md` / `Tables-NNN.md`. The index lists the members and snapshots, for each one, its endpoint row (URL, method, annotation), its tags, its QP metadata and the size of its EQP data - everything the list view shows, and everything Import/Export will need to know what to copy. **No EQP data is stored in a RepoView**; it is copied out of `globalEQPData` only at Import/Export (takeout) time, which also keeps `create` instant. Because the index describes itself, a RepoView's numbers and Tables files don't change when endpoints are later deleted from the central tables.

The UI shows statistics per row (endpoint count, QP count, method counts, tags in the data, endpoint segments) but never lists the EQP data itself, so there are **no routes to browse, add or remove a RepoView's endpoints, and no endpoint-level tag operations** - those only exist in Collections (Bookmark / Test View).

| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/repoview/create` | `{"name","annotation"?,"source_collection","endpoint_ids"?}` | Names that collide with a fixed route (`list`, `create`, `delete`, `import`, `combine`, `tags`, `by-tag`) are rejected. Builds the list from the chosen members of `source_collection` (all of them if `endpoint_ids` is omitted/empty): members, each one's current central tags, a snapshot of its endpoint row and QP metadata, and the size of its EQP data (measured, **not copied**). Returns `endpoints_added`, `tags_copied`, `qps_snapshotted`, `data_size_bytes`. **Name rules** (also enforced on `rename`, `duplicate`'s `new_name`, and the Collection names `convert-to-collection` creates): the name becomes a file/folder name, so it can't be empty, `.`/`..`, a fixed `/repoview` route word (`list`, `create`, `delete`, `import`, `tags`, `by-tag` - those routes would shadow it), longer than 100 characters, start/end with a space, end with a dot, or contain `/ \ : * ? " < > \|` or control characters. 400 if the name exists, the source collection doesn't, or a requested id isn't a member of it. **Changed from the first RepoView release:** `source_collection` is required, and `endpoints_with_data_copied` is gone (nothing is copied) |
| GET | `/repoview/list` | — | Every RepoView with **all the columns the UI table reads, in one call**: `id`, `name`, `annotation`, `source`, `tags` (the row's own tags), `dataTags`, `crud` (count per HTTP method), `segments` (distinct URL path pieces across its endpoints, sorted), `segmentCount`, `segmentFrequency` (each path piece and how many times it occurs across the endpoints' URLs, e.g. `/a/b/c` and `/z/a/c` give `{"a":2,"b":1,"c":2,"z":1}`; every occurrence counts, so `/a/a` adds 2 to `a`), `eidCount`, `qpCount`, `indexLists` (number of `Tables-NNN.md` files), `dateCreated`, `dataSizeBytes` - plus the earlier `file_path` and `created_at`. A RepoView whose index can't be read comes back with zeros and an `error` string instead of failing the whole list. The UI's field names are camelCase; the rest of the API is snake_case |
| GET | `/repoview/:name` | — | One RepoView, same data in snake_case: `eid_count`, `qp_count`, `data_size_bytes`, `tags_in_data`, `crud_types`, `segments`, `segment_count`, `segment_frequency`, `index_lists`, `source`, `tags`, `annotation`, `created_at`. Computed from the RepoView's own index, never the central tables. 404 if it doesn't exist |
| POST | `/repoview/:name/rename` | `{"new_name"}` | Renames the catalog entry and moves the whole RepoView folder to match |
| POST | `/repoview/:name/annotation` | `{"annotation"}` | |
| POST | `/repoview/:name/duplicate` | `{"new_name"}` | Clones the whole folder (index, generated Tables files) under a new name, plus the row's annotation, source and tags. 400 if `new_name` exists or the source doesn't |
| POST | `/repoview/:name/delete` | — | Removes the catalog row and the whole folder |
| POST | `/repoview/delete` | `{"names":[...]}` | Multi-select delete - each name independently, so one bad name doesn't block the rest. Always `200`; check each entry's own `"ok"` in `results`; top-level `"ok"` is `true` only if every name succeeded |
| POST | `/repoview/:name/convert-to-collection` | `{"collection","on_exists"?,"new_name"?}` | The RMB "convert to Collection" pop-up. If `collection` doesn't exist it is created. If it **does** exist and no `on_exists` is given: **409** `{conflict:true, options:["merge","rename"]}` and nothing changes - this is the warning. Retry with `on_exists:"merge"` to add to the existing Collection, or `on_exists:"rename"` plus `new_name` to create a new one (a `new_name` that also exists is another 409). Only endpoints that **still exist in the central tables, and are still the same URL+method**, are added; the rest come back in `skipped` with a reason (top-level `ok` is then `false`). 400 if none of them exist (no empty Collection is created) |
| POST | `/repoview/:name/takeout` | `{"dest_name"?,"overwrite"?}` | **Export**, as a job: returns **202** `{job_id, status:"running", destination, endpoints}` and `edms-child` does the copying; the result arrives as a `ViewIoDone` event on `GET /jobs/ws` (and on `GET /jobs/:id`), with `ViewIoProgress` events while it runs. It copies the EQP data of every listed endpoint out of `globalEQPData` into `storage/takeout/{dest_name}/globalEQPData/{eid}/`, next to the generated `Tables-*.md` and a `repoview-manifest.json`. The SQLite index is **never** included (stripped by the existing `compute::table_view::takeout_item`), so the folder is git/web friendly. The manifest carries what the SQLite would have: the RepoView's name/annotation/source/tags and, per endpoint, its URL, method, annotation, tags and QP status codes/timings - a real EQP folder holds only request/response/header files, so an import couldn't rebuild the index without it. `dest_name` defaults to the RepoView's name. If that takeout folder exists: **409** `{conflict:true, options:["overwrite","rename"]}` straight away, no job. The job's `result` has `endpoints`, `eqp_folders_copied`, `eqp_files_copied`, `index_files_stripped`, `missing_eqp_data` (endpoints that have QPs listed but no data on disk; `result.ok` is `false` if any) - what this route used to return directly. An endpoint with no QPs has no EQP folder, which is normal |
| POST | `/repoview/import` | `{"folder"?,"zip"?,"name"?,"replace_extracted"?}` | **Import**, as a job (**202** `{job_id, status:"running"}`). Give exactly one of `folder` (an unzipped takeout under `storage/imports/uncompressed/repo/`) or `zip` (a `.zip` under `storage/imports/compressed/repo/`, unzipped first into `uncompressed/repo/{zip name}/` - see the strict rules under Jobs below). Every endpoint gets a **fresh EID** from the allocator - "IE always have their own unique EIDs even if the same endpoint exists in a different EID", so an existing endpoint with the same URL is not reused. Three parts: (1) the webserver validates, checks the name and allocates the EIDs; (2) `edms-child` copies the EQP files **renamed to the new EID** (`E0001-AAA-request-1.json` becomes `E0042-AAA-request-1.json`; a leftover folder at that EID from a deleted endpoint is cleared first); (3) the webserver writes the central endpoint, QP and tag rows, builds the index (same code as `create`), restores the tags and regenerates the Tables with the same approach, since the takeout's Tables name EIDs that no longer exist. `name` defaults to the manifest's. The job's `result` has `mapping` (old to new EID, with QP counts), `skipped` (endpoints that couldn't be imported, with reasons; `ok` is `false` if any), `tables_regenerated`. If none can be imported nothing is created and the job fails; a failure part-way rolls that endpoint back. **Answered at once:** **409** `{conflict:true, options:["rename"]}` if the name is taken (retry with `name`); **409** `{options:["replace"]}` if the zip is already unzipped (retry with `replace_extracted:true`); **400** for a missing folder/zip, both or neither given, a takeout of the other kind, or a manifest that isn't ours, has an unsupported version, repeats an EID, or has an id that isn't a real EID. **Found after unzipping** (not a takeout, bad manifest, the manifest's name is taken) fails the job, with the reason in its `error` |
| POST | `/repoview/combine` | `{"name","sources"?,"tags"?,"annotation"?,"on_exists"?,"new_name"?}` | **Combine** (the list-level tag op: "combines data from two or more folders, nothing comes from Collections"). A RepoView is only an index, so this merges indexes - members, each endpoint's snapshot row, QP metadata and tags - and touches no EQP data or central table. Pick the inputs by `sources` (RepoView names; an unknown one is a 400, nothing partial) and/or `tags` (every RepoView carrying any of those row tags). If `name` is new, a RepoView is created from at least two inputs (to copy one, use `duplicate`). If `name` exists: **409** `{conflict:true, options:["merge","rename"]}`, nothing changes; retry with `on_exists:"merge"` (merge the inputs into it, keeping rows it already has; its Tables are regenerated with the same approach if it had any) or `on_exists:"rename"` + `new_name`. The same EID in several inputs is kept once. Inputs are never changed. The new RepoView's `source` lists the Collections its inputs came from and it gets the union of their row tags. Returns `name`, `created`/`merged`, `sources`, `endpoints_added`, `endpoints_total`, `tables_regenerated` |
| POST | `/repoview/:name/membership-tags/add` | `{"tag"}` | Tags on the RepoView itself (the list-level tag ops) - separate from `dataTags`, the tags of the endpoints inside it |
| POST | `/repoview/:name/membership-tags/remove` | `{"tag"}` | |
| GET | `/repoview/:name/membership-tags` | — | |
| GET | `/repoview/by-tag/:tagname` | — | Returns `{"repoviews":[...]}` |
| POST | `/repoview/tags/create` | `{"name","endpoint_ids"?}` | Central tag-count rollup, same as Collections/Webview |
| POST | `/repoview/tags/delete` | `{"names"}` | |
| POST | `/repoview/tags/rename` | `{"old_name","new_name"}` | |
| GET | `/repoview/tags/list` | — | |
| POST | `/repoview/:name/tables/generate` | `{"batch_size"?,"approach"?}` | Generates the Index Table files (Ravi's Index Table wiki) from the RepoView's own index. `approach` is `"endpoint_segments"` (default; one row per endpoint, sorted by URL path) or `"sorted_tags"` (tag to segment-count pivot, untagged last); only one is active at a time since both write the same `Tables-NNN.md` / `Tables-meta.md`, and `Tables-meta.md`'s first line names which. `batch_size` (default 100) counts whole rows; a tag's breakdown is never split across files. Replaces every existing `Tables-*.md` on each call and isn't run automatically. A JSON body is always required, even `{}`. 400 on an unknown `approach` |
| GET | `/repoview/:name/tables` | — | Which generated files exist and which approach produced them: `{generated, approach, files:[{name,size_bytes}]}` |
| GET | `/repoview/:name/tables/:file` | — | One generated file's markdown as `{ok, file, content}` - what the UI's "Endpoint Data table" loads. Only `Tables-meta.md` and `Tables-<number>.md` are ever served (400 otherwise); 404 if not generated |

**Removed in v1.0** (they shipped briefly in the first RepoView release; nothing in the frontend called them): `GET /repoview/:name/endpoints`, `POST .../endpoints/add`, `POST .../endpoints/remove`, `POST .../tags/import`, `GET .../tags/endpoints`, and `POST .../export-to-collection` (replaced by `convert-to-collection`).

**Not built yet:** importing a *Collections* row from the Import/Export table (a Collection still comes in through `/repo/:collection/:filename/import`), and live-update events for the quick list operations (create, rename, combine, convert...), which stay synchronous because they only touch indexes.

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
- Deleting an endpoint now cascades its bookmarks correctly (2026-09-22), but **not** collection memberships or history/request/response data — those can still be left behind, orphaned, referencing a dead endpoint.
- A QP (request/response pair — see Test View above) is generated automatically by every test run, not created/edited by hand. There's no route to edit a QP's saved request/response in place, only to list and delete.
- Import (`/repo/:collection/:filename/import`) only extracts a zip to disk — it does not create/update endpoint, bookmark, or collection-membership DB rows from the imported files.
- WebView's "formatted data" is the rich-text editor's JSON document, stored in `front-page.json` and returned as is (Shivanshu, 2026-10-07); the backend does no HTML conversion. The editor JSON isn't validated beyond being valid JSON. A WebView takeout reuses the RepoView manifest file name (`repoview-manifest.json`, with `"kind":"webview"`).
- A RepoView is a point-in-time list: it doesn't follow its source Collection or the central tables afterwards (an endpoint renamed or deleted centrally keeps its old row in the index), and the generated `Tables-*.md` are a snapshot until `tables/generate` is called again. Its `source` is the Collection it was created from.
- A RepoView's name, annotation, source and row tags still live in the central database's `repoview*` tables, not in its folder, so a RepoView folder carried to a brand-new instance can't be re-registered yet. Import/Export (not built) is the intended way across instances.
- RepoViews created before 2026-10-04 have no endpoint snapshot in their index, so their stats and Tables files come out empty; re-create them. Ones created between 2026-10-04 and v1.0 still have a leftover `globalEQPData/` folder inside, which is now ignored (it is removed with the RepoView).
- `(endpoint_str, method)` is only a plain index centrally, not a unique one, so nothing in the DB itself stops two endpoints sharing a URL+method; the frontend should use `GET /endpoints/lookup` first.
- Takeout, import, unzip and format check are jobs (see Jobs above). The job list is in memory only. If `edms-child` dies without calling back, its job stays `running` (shown as `stalled` after 30 minutes); for an import that also leaves the EIDs it allocated unreleased (the orphan audit/purge tools can clean them up), and the request-side rollback only runs when the child reports a failure. Imported QPs get new timestamps, not their original ones. The webserver finds the child binary relative to its working directory and prefers `target/release`: a stale release build of `edms-child` from before these tasks existed answers "unknown task", so rebuild it (`cargo build --release --bin edms-child` in `compute/`) after pulling.
- RepoView and WebView folders live under `storage/repoview/{name}/` and `storage/webview/{name}/`, the folders the folder manager creates (Ravi's schema). RepoViews created before this change sit under `storage/repoviews/` and aren't found any more; re-create them (they hold no data of their own, only the index).
