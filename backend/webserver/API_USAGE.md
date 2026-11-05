# Using the EDMS API

Send an API call to EDMS from a script or the command line. EDMS runs it, saves the request and the response, and adds it to History or to a collection, the same as pressing **Run** in the app.

With EDMS running, the API is at `http://127.0.0.1:3000`. (If a command below says it can't connect, EDMS isn't running.)

## What you can do

| Call | What it does |
|---|---|
| `POST /hashedtokens/edms/api/history/` with `"ops": "test"` | Calls your API and saves the result in History |
| `POST /hashedtokens/edms/api/collections/` with `"ops": "test_and_insert"` | Calls your API, saves the result and adds it to a collection (created if it doesn't exist) |
| `POST /hashedtokens/edms/api/collections/` with `"ops": "insert"` | Saves a request and response that you provide into a collection. Nothing is sent |

## What to send

The body is JSON, and every `POST` needs the header `Content-Type: application/json`. The examples below already include it.

**History `test`**

| Field | | |
|---|---|---|
| `ops` | required | `"test"` |
| `endpoint_str` | required | The URL to call |
| `method` | required | `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD` or `OPTIONS` |
| `body` | optional | The JSON to send |
| `headers` | optional | Request headers, as an object |
| `timeout_ms` | optional | How long to wait for your API (at most 25000) |
| `annotation` | optional | A note on the endpoint |

**Collections `test_and_insert`** takes the same fields, except that the body is called `request_body`, plus:

| Field | | |
|---|---|---|
| `collection` | required | The collection name. It is created if it doesn't exist |
| `collection_annotation` | optional | A note on the collection |
| `notes` | optional | A note on the saved endpoint |

**Collections `insert`** takes `ops`, `endpoint_str`, `collection`, `response_body` (required), and optionally `method` (default `GET`), `request_body`, `request_headers`, `response_headers`, `annotation`, `collection_annotation` and `notes`.

## How you get the answer

A call returns straight away with a `transaction_id`. Use it to read the result:

`GET /jobs/<transaction_id>`

While `status` is `running`, ask again a moment later. When it is `done`, `result` holds the outcome. The important part is `result.status.code`: **`0` means it worked**. The other codes are listed at the end.

The result also tells you the `endpoint_id` and `request_number` that were saved and, for a test, the status your API answered with (`target_status`).

## Example: PowerShell

This tests a public API and saves it into a collection. Replace the URL with your own API.

Set up a helper once per window. It sends the call, waits for the result and prints it:

```powershell
$u = "http://127.0.0.1:3000"
```

```powershell
function Dev($route, $body) { $r = Invoke-RestMethod -Method Post -Uri "$u/hashedtokens/edms/api/$route/" -ContentType application/json -Body ($body | ConvertTo-Json -Depth 5); do { Start-Sleep -Milliseconds 300; $j = Invoke-RestMethod "$u/jobs/$($r.transaction_id)" } while ($j.status -eq 'running'); $j.result | ConvertTo-Json }
```

Test an API and save it in History:

```powershell
Dev history @{ops="test"; endpoint_str="https://jsonplaceholder.typicode.com/todos/1"; method="GET"}
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

Test an API and save it into a collection:

```powershell
Dev collections @{ops="test_and_insert"; endpoint_str="https://jsonplaceholder.typicode.com/todos/2"; method="GET"; collection="my-first-collection"}
```

Save a request and response you already have:

```powershell
Dev collections @{ops="insert"; endpoint_str="https://api.example.com/login"; method="POST"; collection="my-first-collection"; request_body=@{user="alice"}; response_body=@{token="xyz"}}
```

Send a request with a body and headers:

```powershell
Dev history @{ops="test"; endpoint_str="https://jsonplaceholder.typicode.com/posts"; method="POST"; body=@{title="hello"; userId=1}; headers=@{"X-Example"="1"}}
```

Open the app: the endpoint is in History and Endpoints, and the collection is in your collections list.

## Example: Command Prompt

`curl.exe` comes with Windows. In `cmd`, put the JSON in double quotes and escape the inner quotes as `\"`.

Test an API and save it in History:

```bat
curl.exe -X POST http://127.0.0.1:3000/hashedtokens/edms/api/history/ -H "Content-Type: application/json" -d "{\"ops\":\"test\",\"endpoint_str\":\"https://jsonplaceholder.typicode.com/todos/1\",\"method\":\"GET\"}"
```

It answers with a `transaction_id`. Read the result with it (use the id you got, not this one):

```bat
curl.exe http://127.0.0.1:3000/jobs/job-1234567890-1
```

Test an API and save it into a collection:

```bat
curl.exe -X POST http://127.0.0.1:3000/hashedtokens/edms/api/collections/ -H "Content-Type: application/json" -d "{\"ops\":\"test_and_insert\",\"endpoint_str\":\"https://jsonplaceholder.typicode.com/todos/2\",\"method\":\"GET\",\"collection\":\"my-first-collection\"}"
```

## Good to know

- **A 404 is not a failure.** If your API answers with any status, 404 included, the answer is saved and the code is `0`. `target_status` shows what your API answered.
- **If your API doesn't answer,** nothing is saved: code `106` if it took longer than `timeout_ms`, `105` if it couldn't be reached.
- **Duplicates are skipped.** If the same request and response is already saved for that endpoint, the code is `104` and nothing new is added (the endpoint is still added to the collection).
- **Your API runs on your own PC?** Use `host.docker.internal` in place of `localhost` in `endpoint_str`, for example `http://host.docker.internal:8080/health`. EDMS can't see your PC's `localhost`.
- **A call with a mistake** (for example no `method`) is refused at once with an error that says what's wrong.

## Status codes

| Code | Meaning |
|---|---|
| 0 | Worked |
| 100 | The body is not valid JSON |
| 101 | A required field is missing |
| 102 | The `ops` value isn't supported by this call |
| 103 | A field has a value that isn't allowed |
| 104 | The same request and response is already saved, so nothing was added |
| 105 | Your API couldn't be reached, so nothing was saved |
| 106 | Your API took too long to answer, so nothing was saved |
| 107, 108, 109 | EDMS had a problem handling the call. Try again; if it keeps happening, tell the EDMS team |

For more detail, see [`API_REFERENCE.md`](API_REFERENCE.md).
