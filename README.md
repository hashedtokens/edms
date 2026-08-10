# EDMS Webview

This repository serves a static EDMS UI through Nginx.

## What is in this repo

- `webview.html` is the default page Nginx opens when you visit the server.
- `Dashboard.html` is the dashboard view.
- `listview.html` is the list-based view.
- `Single-EQP.html` is the single endpoint/detail view.
- `assets/` contains static UI assets such as styles, icons, and images.
- `data/` contains the smaller trimmed dataset.
- `Scripts/` contains the JavaScript files that power the UI.
- `nginx.conf` is the Nginx configuration used by the Docker container.
- `tailwind.config.js` is the Tailwind CSS configuration.

## Run the server with Docker

Run the Docker command from the repository root, the same folder that contains `nginx.conf` and `webview.html`.

Example path on this machine:

`/Users/shivanshumangal/Coding/HASHEDTOKEN/EDMS-webview`

Open a terminal in that folder, then run:

```bash
docker run --name EDMS-nginx -p 8080:80 -v "$(pwd)":/usr/share/nginx/html:ro -v "$(pwd)/nginx.conf":/etc/nginx/nginx.conf:ro -d nginx
```

## How it works

1. Docker starts an Nginx container.
2. The current folder is mounted into the container as read-only content.
3. Nginx uses `nginx.conf` from this repo.
4. Nginx serves the files directly from the mounted folder.
5. The app is available at `http://localhost:8080`.

## Open the pages

After the container is running, open these in your browser:

- `http://localhost:8080/webview.html`
- `http://localhost:8080/Dashboard.html`
- `http://localhost:8080/listview.html`
- `http://localhost:8080/Single-EQP.html`

If you want the container to start again later, run the same command from the repository root.

## Stop the container

To stop and remove the container:

```bash
docker stop EDMS-nginx
docker rm EDMS-nginx
```