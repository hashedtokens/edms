# Status codes

The codes the EDMS API returns, the same for every language. A call pushes one QP for one endpoint (endpoint, request, response, headers, plus EDMS data), so a call has one code.

* `0` means all good, `-1` means unknown (a code not assigned yet).
* Everything else counts up from `100`, in the order it was added.
* A new code goes at the end and keeps its number. A code is never renumbered or reused.

The Wiki links here. The Rust side is `backend/compute/src/api_status.rs`, and a test fails if the two lists differ.

| Code | Name | Meaning |
|---|---|---|
| 0 | `OK` | All good |
| -1 | `UNKNOWN` | Unknown, or a code not assigned yet |
| 100 | `INVALID_REQUEST` | The request body is not valid JSON or not an object |
| 101 | `MISSING_PARAM` | A mandatory parameter is missing |
| 102 | `UNKNOWN_OP` | The ops value is not one this endpoint supports |
| 103 | `INVALID_PARAM` | A parameter has a value that is not allowed |
| 104 | `DUPLICATE_QP` | An identical QP is already stored; it was not added again |
| 105 | `TEST_FAILED` | The test request failed, so nothing was stored |
| 106 | `TEST_TIMED_OUT` | The test request timed out, so nothing was stored |
| 107 | `COMPUTE_UNAVAILABLE` | The compute process could not be started |
| 108 | `COMPUTE_STALLED` | The compute process never reported back |
| 109 | `INTERNAL_ERROR` | The server failed while handling the call |

## How the code is delivered

The call returns `202` at once. The final code arrives over the WebSocket (unsecured `ws`), so the caller keeps a socket open and waits for it, with a timeout of about 15 seconds.

```json
{
  "status": { "code": 104, "name": "DUPLICATE_QP", "message": "..." },
  "endpoint_id": "EID-1",
  "duplicate_of": 1
}
```

Only the fields that apply are present: `endpoint_id`, `request_number` (the QP number it was stored as), `duplicate_of`, `endpoint_created`, `collection_created` (collections ops only) and `detail` (extra text for the log).
