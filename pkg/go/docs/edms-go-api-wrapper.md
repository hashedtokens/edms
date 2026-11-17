# EDMS Go API Wrapper — Implementation & Verification

**Project:** HashedTokens Endpoint Data Management System (EDMS)  
**Deliverable:** Native Go API wrapper (`pkg/go/`)  
**Implementation PR:** [hashedtokens/edms#94](https://github.com/hashedtokens/edms/pull/94)  
**Source branch:** `pranoopreddy1:feature/go-edms-package`  
**Target branch:** `hashedtokens/edms:tmp`  
**Documentation date:** 2026-11-17

## 1. Goal and scope

Create a small Go package that uses the EDMS API to test a target HTTP endpoint and retain its request/response data, while returning the final EDMS transaction result. The wrapper is asynchronous and uses only the Go standard library. Its primary entry point is `Request(endpoint, payload, options...)`, with variadic options, sensible defaults, and a 15-second operation timeout.

This implementation targets EDMS's **collections** operation `test_and_insert`, rather than the full set of EDMS operations.

## 2. Delivered files

| File | Purpose |
| --- | --- |
| `pkg/go/go.mod` | Go module: `github.com/hashedtokens/edms/pkg/go`; minimum Go version 1.22 |
| `pkg/go/hashedtokens.go` | Public API, HTTP orchestration, status parsing, and native WebSocket client |
| `pkg/go/hashedtokens_test.go` | Mock-server tests of success, EDMS error status, and timeout behavior |
| `pkg/go/README.md` | Installation assumptions, usage examples, options, semantics, test instructions, and limitations |

The package is proposed in [PR #94](https://github.com/hashedtokens/edms/pull/94), which is **open** and has not been merged as of this documentation.

## 3. Public API

```go
func Request(endpoint string, payload any, options ...Option) <-chan Result
func RequestContext(ctx context.Context, endpoint string, payload any, options ...Option) <-chan Result
```

`Request` returns a buffered channel immediately; a goroutine performs the network work and writes exactly one `Result` before closing the channel. `RequestContext` additionally allows caller cancellation. Reading from the returned channel (for example, `result := <-ht.Request(...)`) waits for the outcome; this is the caller's choice, not a blocking network call in `Request` itself.

### Supported options

| Option | Meaning |
| --- | --- |
| `WithServer(url)` | EDMS backend base URL; default `http://127.0.0.1:3000` |
| `WithCollection(name)` | EDMS collection; default `go-package` |
| `WithMethod(method)` | Target HTTP method; default `GET` for nil payload, `POST` otherwise |
| `WithTimeout(duration)` | Whole-operation deadline; default `15*time.Second` |
| `WithHeaders(map[string]string)` | Headers passed to EDMS as target request headers |

### Example

```go
package main

import (
    "fmt"
    "time"

    ht "github.com/hashedtokens/edms/pkg/go"
)

func main() {
    done := ht.Request(
        "https://httpbin.org/post",
        map[string]any{"message": "hello from Go"},
        ht.WithMethod("POST"),
        ht.WithCollection("go-demo"),
        ht.WithTimeout(15*time.Second),
    )

    result := <-done
    if result.Err != nil {
        fmt.Println("Request failed:", result.Err)
        return
    }
    fmt.Println("EDMS status:", result.EDMS.Status.Code)
    fmt.Println("Target response:", string(result.Response))
}
```

This example requires a running EDMS server and outbound access from EDMS to the target URL. The example was not used as evidence of a live-backend test.

## 4. Request/response workflow

1. Spawn a goroutine with an operation-scoped context and deadline.
2. Establish an unencrypted WebSocket (`ws`) connection to EDMS at `/hashedtokens/edms/api/ws` using a standard-library RFC 6455 handshake.
3. Read the initial `transactions` snapshot, confirming the WebSocket subscriber is ready before submitting work.
4. Submit `POST /hashedtokens/edms/api/collections/` with `ops: "test_and_insert"`, the target URL, HTTP method, collection, optional body/headers, and `return_response: true`.
5. EDMS returns HTTP `202 Accepted` and a `transaction_id` when the operation is queued.
6. Read WebSocket `event` messages and select the `DevApiResult` whose `payload.transaction_id` matches the accepted ID.
7. Parse `result.status` and other EDMS result fields. Return the EDMS result, included `response` field, and any error through the channel.

The target HTTP request is executed by EDMS, **not independently by the Go package**.

### Wire-format outline

Submission to EDMS:

```json
{
  "ops": "test_and_insert",
  "endpoint_str": "https://httpbin.org/post",
  "method": "POST",
  "collection": "go-demo",
  "request_body": {"message": "hello from Go"},
  "return_response": true
}
```

Accepted HTTP response:

```json
{"transaction_id": "<transaction-id>"}
```

Final WebSocket event:

```json
{
  "type": "event",
  "event": {
    "type": "DevApiResult",
    "payload": {
      "transaction_id": "<transaction-id>",
      "result": {"status": {"code": 0, "name": "OK", "message": "..."}}
    }
  }
}
```

## 5. WebSocket behavior

A minimal native client is implemented using `net`, `net/http`, `bufio`, `crypto/rand`, `crypto/sha1`, `encoding/base64`, and `encoding/binary`. It validates the server upgrade response, reads text frames and continuations, answers ping frames with masked pong frames, and enforces a 4 MiB maximum message size. It receives final EDMS status over the WebSocket. `context` and connection deadlines enforce timeout/cancellation.

Current design does not support TLS (`wss`/HTTPS EDMS base URLs), extensions such as compression, or all possible WebSocket protocol features. It uses a dedicated WebSocket connection for each request.

## 6. EDMS results and errors

- **Code 0 (`OK`)**: returned as a successful `Result`.
- **Code 104 (`DUPLICATE_QP`)**: retained as an EDMS result, without conversion into a Go error.
- **Other nonzero EDMS status codes**: returned in `Result.EDMS` and represented by `Result.Err`.
- **Transport/serialization/handshake errors, unexpected HTTP status, or deadline expiry**: returned in `Result.Err`.

The Go result exposes `Response` as `json.RawMessage`, so callers should inspect the EDMS response shape before relying on target body fields.

## 7. Verification evidence

The following command was executed locally on Windows with Go 1.27.0 after the module's minimum version was set to 1.22:

```text
go test -v ./...
=== RUN   TestRequestSuccess
--- PASS: TestRequestSuccess (0.01s)
=== RUN   TestRequestStatusError
--- PASS: TestRequestStatusError (0.00s)
=== RUN   TestRequestTimeout
--- PASS: TestRequestTimeout (0.10s)
PASS
ok      github.com/hashedtokens/edms/pkg/go     2.586s
```

| Test | Verified behavior |
| --- | --- |
| `TestRequestSuccess` | Accepted HTTP submission, matching WebSocket event, success result, and submitted options |
| `TestRequestStatusError` | Nonzero EDMS code 105 surfaced as an error |
| `TestRequestTimeout` | Request deadline triggers an error when no final WebSocket event arrives |

Tests were performed with `net/http/httptest` and a simulated EDMS WebSocket server. **They do not constitute an end-to-end test with a running production/development EDMS backend**, nor a build test with Go 1.22 specifically.

The source was formatted with `gofmt -w hashedtokens.go hashedtokens_test.go`.

## 8. Known limitations and follow-up work

- Live EDMS backend integration test has not been performed; recommended before merging if the reviewer requires end-to-end verification.
- The package currently exposes only collections `test_and_insert`, not history or other operations.
- No fallback polling of `GET /jobs/{transaction_id}` when the WebSocket result is missed.
- The WebSocket client is intentionally minimal and supports only unencrypted EDMS servers (`http` / `ws`).
- No published standalone package repository or release has been verified; the module currently resides under `pkg/go/` in the EDMS monorepo.
- Tests cover three primary paths; additional tests for cancellation, duplicate-QP, protocol edge cases, and live server compatibility are reasonable future improvements.

## 9. Submission and review

**Implementation:** [GitHub PR #94 — feat: add asynchronous Go EDMS API wrapper](https://github.com/hashedtokens/edms/pull/94)  
**Target:** `hashedtokens/edms` → `tmp`  
**Head:** `pranoopreddy1:feature/go-edms-package`  
**Implementation commits:** `0d6f8bb` and `04ce730`  
**Documentation location:** HashedTokens EDMS repository at `pkg/go/docs/edms-go-api-wrapper.md` (with checkpoint at `pkg/go/checkpoints/edms-go-api-wrapper-checkpoint.md`), proposed through PR #94.

**Review request:** Please review API compatibility, native WebSocket behavior, expected EDMS response shapes, and whether a live-backend run is required before merge.
