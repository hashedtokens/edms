# EDMS application endpoints: Tables

The Tables of EDMS's own HTTP API: every route the webserver registers in
[`backend/webserver/src/main.rs`](../../backend/webserver/src/main.rs), listed
the way a RepoView takeout lists endpoints.

| File | What it is |
|---|---|
| `Tables-001.md`, `Tables-002.md` | One row per endpoint (134 in all), sorted by path, 100 rows per file |
| `Tables-meta.md` | The files and their first/last segment, the path segments with how many endpoints use each, and the takeout facts |

## How they were made

Not written by hand. Each route was added as an endpoint, with the dev API
(`POST /hashedtokens/edms/api/collections/`, `ops: "insert"`), to one
collection, a RepoView was built from that collection, and the RepoView was
taken out (`POST /repoview/:name/takeout`). `Tables-*.md` are the files that
takeout wrote, unchanged: the same generator and the same format as any other
takeout (the *Endpoint Segments* approach, 100 rows per file).

Run against a scratch server with an empty storage folder, on the code in
`tmp` after the preview PR (#97).

## Reading the columns

- **Endpoint** is the route path as registered: `:name` is a path parameter
  and `*path` is a wildcard. The host is never shown.
- **CRUD Type** is the HTTP method.
- **Annotation** is the name of the Rust handler function behind the route
  (for example `create_repoview_entry`), so a row can be found in the code.
- Each endpoint was also tagged with its area (`collections`, `repoview`,
  `webview`, `test-view`, `tags`, `dev-api`, and so on), but the *Endpoint
  Segments* Tables have no Tags column, so the tags don't show here.
- **Query Parameter** is `-` for every row: the routes are listed by method and
  path, and their query parameters are not part of this data.
- **QP Count**, **Data Size**: every endpoint holds one placeholder QP (an
  empty request and an empty response), because the dev API needs one to
  store an endpoint. They say nothing about the route. **Created** and
  **Updated** are when the tables were generated.
- Routes served both with and without a trailing slash
  (`/hashedtokens/edms/api/history` and `.../history/`, and the same for
  `collections`) are two rows, because they are two registered routes.

## Checked

All 134 listed routes were requested on the running server, and none was
answered as "no such route" (404 without a body) or "wrong method" (405).
