# Frontend Setup

## Start the Frontend

1. Go to the frontend directory:

```bash
cd frontend
```

2. Build and run using Docker:

```bash
docker build -t edms-frontend .
docker run -d --name edms-frontend --network hashedtokens-edms -p 8080:80 edms-frontend
```

## Backend Requirement

- The backend must be running on port `3000`.
- Frontend and backend must be connected to the same Docker network.
- The backend must be resolvable using the hostname configured in `nginx.conf`.
- Currently, the frontend expects the backend hostname `webserver`.

## Cautions

- Do not use the dummy files in `frontend/src/data/` as backend data.
- Make sure the backend is connected to `hashedtokens-edms` before starting the frontend.
- If the backend hostname cannot be resolved, Nginx may fail to start.