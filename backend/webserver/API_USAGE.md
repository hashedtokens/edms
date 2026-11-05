# Using the EDMS API

How to push API calls into EDMS from a script or the command line, without the UI. This is the how-to; the full reference is [`API_REFERENCE.md`](API_REFERENCE.md) and the status codes are in [`pkg/status_codes.md`](../../pkg/status_codes.md).

Two calls, both `POST`, both JSON:

| Call | What it does |
|---|---|
| `POST /hashedtokens/edms/api/history/` with `"ops": "test"` | Sends the request to your API, then stores the endpoint, the request, the response and a History entry, as the **Run** button does |
| `POST /hashedtokens/edms/api/collections/` with `"ops": "test_and_insert"` | Same, and also puts the endpoint in a collection (created if it doesn't exist) |
| `POST /hashedtokens/edms/api/collections/` with `"ops": "insert"` | Stores a request and response **you give it** (nothing is sent) in a collection |

## 1. Start EDMS

```
cd init
docker compose up -d --build
```

- The API is on port **3000**: `http://127.0.0.1:3000`. (The UI is on 3911, but the UI port does not serve the API.)
- Open pages in the UI (History, Endpoints, a collection) reload by themselves after a call.

To stop it later: `docker compose down` (this keeps your data).

## 2. How a call works

1. You `POST` the request. If it is valid you get **`202`** straight away with a `transaction_id`. If it is not valid you get **`400`** with a status code and a `detail` saying what is wrong.
2. EDMS does the work in the background (it may call your API, which takes time).
3. You read the final result, one of two ways:
   - **Poll** `GET /jobs/<transaction_id>` until `status` is no longer `running`. The result is in `result`.
   - **WebSocket** at `ws://127.0.0.1:3000/hashedtokens/edms/api/ws`, which sends the result as soon as it is ready (see [WebSocket](#websocket)).

The result always has a `status.code` (`0` is success, see [Status codes](#status-codes)), plus the fields that apply: `endpoint_id`, `request_number`, `duplicate_of`, `endpoint_created`, `collection_created`, `target_status`, `detail`.

## 3. What you can send

**History `test`**

| Field | | |
|---|---|---|
| `ops` | required | `"test"` |
| `endpoint_str` | required | The URL to call (`http` or `https`) |
| `method` | required | `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD` or `OPTIONS` |
| `body` | optional | JSON body to send. No body is stored as `{}` |
| `headers` | optional | Object of request headers, strings only |
| `timeout_ms` | optional | How long to wait for your API, at most 25000 (default 25000) |
| `annotation` | optional | A note on the endpoint |
| `endpoint_id` | optional | Use this existing endpoint id |

**Collections `test_and_insert`** takes the same fields as above, except that the request body is called `request_body` (not `body`), plus:

| Field | | |
|---|---|---|
| `collection` | required | Collection name. Created empty if it doesn't exist. Can't contain `/ \ : * ? " < > \|` |
| `collection_annotation` | optional | A note on the collection (used when it is created) |
| `notes` | optional | A note on the bookmark |

**Collections `insert`** stores what you give it. Fields: `ops`, `endpoint_str`, `collection`, `response_body` (required), and optionally `method` (default `GET`), `request_body`, `request_headers`, `response_headers`, `annotation`, `collection_annotation`, `notes`. Nothing is sent, so no status code or timing is recorded and no History entry is made.

## 4. Example (PowerShell)

The example tests a public JSON API and saves the result into a collection called `my-first-collection`. Any API works; swap in your own URL.

If your API runs **on your own PC**, use `http://host.docker.internal:<port>/...` instead of `localhost`, because EDMS runs in a container and can't see your PC's `localhost`. (On Linux, add `extra_hosts: ["host.docker.internal:host-gateway"]` to the backend service in `init/docker-compose.yml`, or use your PC's IP address.)

**Set up a small helper once per window.** It posts the call, waits for the result and prints it.

```powershell
$u = "http://127.0.0.1:3000"
```

```powershell
function Dev($route, $body) { $r = Invoke-RestMethod -Method Post -Uri "$u/hashedtokens/edms/api/$route/" -ContentType application/json -Body ($body | ConvertTo-Json -Depth 5); do { Start-Sleep -Milliseconds 300; $j = Invoke-RestMethod "$u/jobs/$($r.transaction_id)" } while ($j.status -eq 'running'); $j.result | ConvertTo-Json }
```

**Test an API and keep the result in History:**

```powershell
Dev history @{ops="test"; endpoint_str="https://jsonplaceholder.typicode.com/todos/1"; method="GET"; annotation="first todo"}
```

```json
{
    "detail": "200 in 85ms",
    "endpoint_created": true,
    "endpoint_id": "E0001-AAA",
    "request_number": 1,
    "status": { "code": 0, "message": "All good", "name": "OK" },
    "target_status": 200
}
```

**Test an API and save it into a collection:**

```powershell
Dev collections @{ops="test_and_insert"; endpoint_str="https://jsonplaceholder.typicode.com/todos/2"; method="GET"; collection="my-first-collection"; collection_annotation="examples from the docs"}
```

**Save a request and response you already have (nothing is sent):**

```powershell
Dev collections @{ops="insert"; endpoint_str="https://api.example.com/login"; method="POST"; collection="my-first-collection"; request_body=@{user="alice"}; response_body=@{token="xyz"}}
```

**POST with a body and headers:**

```powershell
Dev history @{ops="test"; endpoint_str="https://jsonplaceholder.typicode.com/posts"; method="POST"; body=@{title="hello"; userId=1}; headers=@{"X-Example"="1"}}
```

**See what was stored:**

```powershell
Invoke-RestMethod "$u/collections/my-first-collection"
```

```powershell
Invoke-RestMethod "$u/collections/my-first-collection/endpoints"
```

```powershell
Invoke-RestMethod "$u/test-view/history/cards" | ConvertTo-Json -Depth 5
```

```powershell
Invoke-RestMethod "$u/test-view/E0001-AAA/qps" | ConvertTo-Json -Depth 5
```

Open the UI at `http://localhost:3911`: the endpoint is under History and Endpoints, and the collection is in the collections list.

A call that is refused (for example a missing `method`) returns `400` immediately, which PowerShell prints as an error containing `"code":101`. That is expected.

## 5. Example (Command Prompt)

`curl.exe` ships with Windows. In `cmd`, the JSON goes in double quotes with the inner quotes escaped as `\"`.

**Test an API and keep the result in History:**

```bat
curl.exe -X POST http://127.0.0.1:3000/hashedtokens/edms/api/history/ -H "Content-Type: application/json" -d "{\"ops\":\"test\",\"endpoint_str\":\"https://jsonplaceholder.typicode.com/todos/1\",\"method\":\"GET\"}"
```

You get back `{"ok":true,"transaction_id":"job-...","ops":"test"}`. Read the result with that id:

```bat
curl.exe http://127.0.0.1:3000/jobs/job-1234567890-1
```

**Test an API and save it into a collection:**

```bat
curl.exe -X POST http://127.0.0.1:3000/hashedtokens/edms/api/collections/ -H "Content-Type: application/json" -d "{\"ops\":\"test_and_insert\",\"endpoint_str\":\"https://jsonplaceholder.typicode.com/todos/2\",\"method\":\"GET\",\"collection\":\"my-first-collection\"}"
```

**Save a request and response you already have:**

```bat
curl.exe -X POST http://127.0.0.1:3000/hashedtokens/edms/api/collections/ -H "Content-Type: application/json" -d "{\"ops\":\"insert\",\"endpoint_str\":\"https://api.example.com/login\",\"method\":\"POST\",\"collection\":\"my-first-collection\",\"request_body\":{\"user\":\"alice\"},\"response_body\":{\"token\":\"xyz\"}}"
```

**See what was stored:**

```bat
curl.exe http://127.0.0.1:3000/collections/my-first-collection
```

```bat
curl.exe http://127.0.0.1:3000/test-view/history/cards
```

On macOS or Linux, the same calls work with `curl` and single quotes around the JSON: `curl -X POST ... -d '{"ops":"test", ...}'`.

## 6. What a call leaves behind

Everything a Run, a Bookmark and a Save leave behind, so the UI treats it like anything made by hand:

- **The endpoint**, found by URL and method, or created with a new id (`endpoint_created` says which).
- **One QP** (request, response, headers). The number is the endpoint's next one.
- **A History entry** with the status and time (`"404 in 120ms"`), for `test` and `test_and_insert`. An `insert` makes none.
- **The collection** (`test_and_insert` and `insert`): created if new, the endpoint is bookmarked into it, and its size is kept up to date.

Rules worth knowing:

- **A 404 (any HTTP status) from your API is a completed test.** It is stored like a 200 and the code is `0`; `target_status` tells you what your API answered. Only no answer at all is a failure.
- **No answer stores nothing:** `106` if your API took longer than `timeout_ms`, `105` if it couldn't be reached.
- **A duplicate** (an identical request and response already stored for that endpoint) ends with `104` and `duplicate_of` (the QP number). Nothing is added, but for a collections call the endpoint still joins the collection. The same call through `history` and `collections` is recognised as the same QP.
- Calls in parallel are safe: they don't take the same QP number or create two endpoints for one URL.

## 7. Status codes

| Code | Name | Meaning |
|---|---|---|
| 0 | `OK` | All good |
| -1 | `UNKNOWN` | Unknown, or a code not assigned yet |
| 100 | `INVALID_REQUEST` | The body is not valid JSON or not an object |
| 101 | `MISSING_PARAM` | A mandatory parameter is missing |
| 102 | `UNKNOWN_OP` | The `ops` value is not supported by this route |
| 103 | `INVALID_PARAM` | A parameter has a value that is not allowed |
| 104 | `DUPLICATE_QP` | An identical QP is already stored; it was not added again |
| 105 | `TEST_FAILED` | The test request failed (unreachable), nothing was stored |
| 106 | `TEST_TIMED_OUT` | The test request timed out, nothing was stored |
| 107 | `COMPUTE_UNAVAILABLE` | EDMS could not start its compute process |
| 108 | `COMPUTE_STALLED` | The compute process never reported back (after 30 seconds) |
| 109 | `INTERNAL_ERROR` | The server failed while handling the call |

Codes `100` to `103` come back immediately as HTTP `400` (`107` as `503`, `109` as `500`). All the others only arrive in the result.

## WebSocket

Instead of polling, keep a socket open and wait for the result. Node 22 or later (it has `WebSocket` built in):

```js
const ws = new WebSocket("ws://127.0.0.1:3000/hashedtokens/edms/api/ws");
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.type === "event" && msg.event.type === "DevApiResult") {
    console.log(msg.event.payload.transaction_id, msg.event.payload.result);
  }
};
```

On connect the socket first sends `{"type":"transactions", ...}` (the calls still running). A call that finished before you connected is read with `GET /jobs/<transaction_id>`. Wait up to about 15 seconds for a result.

## Troubleshooting

| What you see | Why |
|---|---|
| `404` on the `/hashedtokens/...` route | The backend image is older than this feature: rebuild with `docker compose up -d --build`. Also check the port: it is 3000, not 3911 |
| `105` for an API on your own PC | Use `host.docker.internal` instead of `localhost` in `endpoint_str` |
| `106` | Your API was slower than `timeout_ms` (at most 25 seconds) |
| `107` | The backend can't start its compute helper (is `edms-child` built?) |
| `108` | The compute process crashed or hung; check `docker logs hashedtokens-edms-backend` |
| PowerShell prints a red error for `101`, `102` or `103` | The call was refused straight away; read the `detail` in the message |
| The UI doesn't show the new data | Reload the page; open pages normally update on their own |
