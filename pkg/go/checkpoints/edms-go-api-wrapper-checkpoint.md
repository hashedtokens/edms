# EDMS Go API Wrapper — Checkpoint

**Date:** 2026-11-17  
**Task:** Create a native Go wrapper for EDMS API  
**Implementation PR:** https://github.com/hashedtokens/edms/pull/94  
**Implementation branch:** `feature/go-edms-package` → `tmp`  
**Documentation destination:** HashedTokens EDMS repository, `pkg/go/` on `feature/go-edms-package` → `tmp`

## Requirements checklist

- [x] Go package under official EDMS `pkg/go/`
- [x] No third-party Go dependencies
- [x] Single primary `Request(endpoint, payload, options...)` entry point
- [x] Asynchronous execution using goroutine and result channel
- [x] Variadic options with defaults
- [x] 15-second default timeout and context cancellation
- [x] Native unencrypted WebSocket client receives final EDMS status
- [x] EDMS HTTP `test_and_insert` collections submission and transaction matching
- [x] Error/status handling, including code 104 duplicate QP
- [x] README usage and API documentation
- [x] Automated mock-server tests (3 passing)
- [x] GitHub implementation PR opened (#94)
- [ ] Live EDMS backend end-to-end test (optional follow-up; not completed)
- [ ] Maintainer review/merge (pending)

## Verification

Run from `pkg/go`:

```bash
gofmt -w hashedtokens.go hashedtokens_test.go
go test -v ./...
```

Observed test results: `TestRequestSuccess` PASS; `TestRequestStatusError` PASS; `TestRequestTimeout` PASS. The tests used a simulated EDMS server, not a live instance.

## Submitted changes

| Commit | Purpose |
| --- | --- |
| `0d6f8bb` | Add asynchronous Go EDMS wrapper, tests, and README |
| `04ce730` | Set minimum Go module version to 1.22 |

Changed source files: `pkg/go/hashedtokens.go`, `pkg/go/hashedtokens_test.go`, `pkg/go/go.mod`, `pkg/go/README.md`.

## Hand-off

Documentation: `pkg/go/docs/edms-go-api-wrapper.md` and `pkg/go/checkpoints/edms-go-api-wrapper-checkpoint.md`.  
Review focus: Go API ergonomics, EDMS transaction protocol, WebSocket limits, and whether live-server testing is needed.  
Status at documentation time: implementation PR open and mergeable; not merged.
