# Initialization for EDMS application

This folder is the single entry point for running EDMS — clone the repo
and run one command.

## Run it

```bash
cd init
docker compose up --build
```

That's it — nothing to configure first, no `.env` file.

EDMS keeps all its data — collections, request/response history, everything
under `storage/` — in a plain folder on your machine, not hidden inside a
Docker volume: by default `../../edms-data`, a folder next to this repo.
It's created automatically the first time you run this — you don't need
to make it yourself.

**Want your data somewhere else?** Open `docker-compose.yml`, find this
line under `webserver: volumes:`, and change the left-hand side to your
own path:

```yaml
- ../../edms-data:/app/edms_root
```

Whatever you set it to gets created automatically the same way — no
manual folder setup either way.

First run takes a few minutes (Rust release build). Once it's up:

- **App:** http://localhost:3911
- **Backend API:** http://localhost:3000 (see `backend/webserver/API_REFERENCE.md`)

A one-shot `seed` container runs automatically the first time and
populates the app so it isn't empty on first look:

- 30 endpoints across 11 public APIs (JSONPlaceholder, httpbin, Postman
  Echo, Open-Meteo, PokeAPI and others), each with its recorded run data
  (request, response, headers, status code, timing), history and tags
- 4 themed Collections
- 4 RepoViews (one per Collection) plus one combined by tag, with Index
  Tables
- 3 WebViews with front pages

The runs are **recorded**: they were captured once and are committed in
`demo-data/demo-bundle/`, so first boot needs no internet and calls no
live URLs. The seed brings them in through the product's own import, so
everything is ordinary data afterwards and you can delete whatever you
like. It takes about ten seconds once the backend is up.

It only seeds an empty database, so it is safe to leave in place on every
`docker compose up`: if endpoints already exist it does nothing. To add
the demo data to a database that already has data, run it by hand with
`SEED_FORCE=1` (endpoints that already exist for the same URL and method
are reused, not duplicated). `EDMS_STORAGE` is the backend's storage
folder:

```bash
SEED_FORCE=1 EDMS_BASE=http://localhost:3000 EDMS_STORAGE=../edms-data node init/seed.mjs
```

### Refreshing the recorded data

Only needed if you change the demo endpoints or want fresher recordings
(the bundle is committed, normal use never touches it). Start a scratch
backend on an empty database, then:

```bash
EDMS_BASE=http://localhost:3000 EDMS_STORAGE=/path/to/its/storage-root node init/capture-demo-data.mjs
```

That runs the endpoints for real, takes the result out, replaces
`demo-data/demo-bundle/`, and scrubs anything that identifies the machine
that captured it: the public IP echo services record (replaced with a
documentation address), the CDN data-center codes in `cf-ray` /
`x-amz-cf-pop` (a coarse location hint), cookie values, CDN reporting
tokens and request / trace ids. Commit the result. To re-apply just the
scrub to the committed bundle, with no backend:

```bash
node init/capture-demo-data.mjs --scrub-only
```

## Resetting

```bash
docker compose down -v
rm -rf ../backend/webserver/data
```

Your storage folder (`../../edms-data`, or wherever you pointed it) is
untouched by this — delete it yourself if you want a truly clean slate.
