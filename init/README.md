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
populates ~25 real, tested endpoints with history, collections, and tags,
so the app isn't empty on first look. It needs outbound internet (it
tests against a public API) and only runs once — safe to leave in place
on every `docker compose up`.

## Resetting

```bash
docker compose down -v
rm -rf ../backend/webserver/data
```

Your storage folder (`../../edms-data`, or wherever you pointed it) is
untouched by this — delete it yourself if you want a truly clean slate.
