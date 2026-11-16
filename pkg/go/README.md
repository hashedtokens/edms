

\# EDMS Go Package



A lightweight Go wrapper for the HashedTokens EDMS API.



This package lets Go applications send API requests through EDMS, store request/response data, and receive final EDMS status codes through WebSocket.



\## Features



\- Asynchronous API requests using goroutines and channels

\- HTTP requests through the EDMS backend

\- WebSocket support for final transaction results

\- Default 15-second timeout

\- Variadic configuration options

\- Context cancellation support

\- EDMS status-code handling

\- No third-party dependencies

\- Built entirely with the Go standard library



\## Requirements



\- Go 1.22 or newer

\- A running EDMS backend

\- HTTP and unencrypted WebSocket (`ws`) access to EDMS



The default EDMS server is:



http://127.0.0.1:3000



\## Project Structure



&#x20;   pkg/go/

&#x20;   ├── go.mod

&#x20;   ├── hashedtokens.go

&#x20;   ├── hashedtokens\_test.go

&#x20;   └── README.md



\## Basic Usage



Import the package into your Go application:



```go

import ht "github.com/hashedtokens/edms/pkg/go"

```



Send a request:



```go

package main



import (

&#x20;   "fmt"

&#x20;   "time"



&#x20;   ht "github.com/hashedtokens/edms/pkg/go"

)



func main() {

&#x20;   resultChannel := ht.Request(

&#x20;       "https://httpbin.org/post",

&#x20;       map\[string]any{

&#x20;           "message": "Hello from EDMS",

&#x20;       },

&#x20;       ht.WithMethod("POST"),

&#x20;       ht.WithCollection("go-demo"),

&#x20;       ht.WithTimeout(15\*time.Second),

&#x20;   )



&#x20;   // Request() returns immediately.

&#x20;   // Network operations run in a goroutine.

&#x20;   result := <-resultChannel



&#x20;   if result.Err != nil {

&#x20;       fmt.Println("Error:", result.Err)

&#x20;       return

&#x20;   }



&#x20;   fmt.Println("EDMS status:", result.EDMS.Status.Code)

&#x20;   fmt.Println("Status name:", result.EDMS.Status.Name)

&#x20;   fmt.Println("Endpoint ID:", result.EDMS.EndpointID)

&#x20;   fmt.Println("API response:", string(result.Response))

}

```



The import path above is intended for use once the package is published at that location.



\## API



\### Request



```go

func Request(

&#x20;   endpoint string,

&#x20;   payload any,

&#x20;   options ...Option,

) <-chan Result

```



Starts an asynchronous EDMS request and returns a channel that receives one result.



\### RequestContext



```go

func RequestContext(

&#x20;   ctx context.Context,

&#x20;   endpoint string,

&#x20;   payload any,

&#x20;   options ...Option,

) <-chan Result

```



Works like `Request`, but also supports caller-controlled cancellation.



\## Configuration Options



| Option | Description |

|---|---|

| `WithServer(url)` | Set the EDMS backend URL |

| `WithCollection(name)` | Set the collection name |

| `WithMethod(method)` | Set the target HTTP method |

| `WithTimeout(duration)` | Set the operation timeout |

| `WithHeaders(headers)` | Set headers for the target request |



\### Defaults



\- EDMS server: `http://127.0.0.1:3000`

\- Collection: `go-package`

\- Timeout: 15 seconds

\- HTTP method: `GET` when payload is nil, otherwise `POST`



\### Custom Configuration



```go

result := <-ht.Request(

&#x20;   "https://httpbin.org/get",

&#x20;   nil,

&#x20;   ht.WithServer("http://127.0.0.1:3000"),

&#x20;   ht.WithCollection("my-collection"),

&#x20;   ht.WithMethod("GET"),

&#x20;   ht.WithTimeout(10\*time.Second),

&#x20;   ht.WithHeaders(map\[string]string{

&#x20;       "X-Example": "EDMS-Go",

&#x20;   }),

)

```



\## How It Works



1\. `Request()` starts a goroutine.

2\. The wrapper connects to EDMS over WebSocket.

3\. It subscribes before submitting the HTTP request.

4\. It sends the request to the EDMS collections API.

5\. EDMS performs the target HTTP request and processes its data.

6\. EDMS returns a transaction ID in the HTTP response.

7\. The wrapper waits for the matching WebSocket status event.

8\. The final result is delivered through the Go channel.



EDMS handles the target request. The Go wrapper does not independently call the target API.



\## Result Structure



The returned `Result` includes:



\- `Response`: the response data returned by EDMS

\- `EDMS`: the final EDMS result and status

\- `Err`: transport, timeout, or EDMS operation error



\## EDMS Status Codes



| Code | Name |

|---|---|

| 0 | OK |

| -1 | UNKNOWN |

| 100 | INVALID\_REQUEST |

| 101 | MISSING\_PARAM |

| 102 | UNKNOWN\_OP |

| 103 | INVALID\_PARAM |

| 104 | DUPLICATE\_QP |

| 105 | TEST\_FAILED |

| 106 | TEST\_TIMED\_OUT |

| 107 | COMPUTE\_UNAVAILABLE |

| 108 | COMPUTE\_STALLED |

| 109 | INTERNAL\_ERROR |



Code `104` indicates a duplicate QP and is preserved as an EDMS result rather than converted into a Go error.



Other nonzero status codes are returned with an error.



\## Timeout and Cancellation



The default timeout is 15 seconds for the overall operation.



Override it using:



```go

ht.WithTimeout(30 \* time.Second)

```



For cancellation support:



```go

ctx, cancel := context.WithCancel(context.Background())

defer cancel()



resultChannel := ht.RequestContext(

&#x20;   ctx,

&#x20;   "https://httpbin.org/get",

&#x20;   nil,

)

```



\## WebSocket Support



The package uses a minimal WebSocket client implemented with Go's standard library.



It supports:



\- WebSocket HTTP upgrade

\- Server text frames

\- Fragmented messages

\- Ping and pong handling

\- Close frames

\- Transaction ID matching

\- Context-based cancellation



Only unencrypted WebSocket (`ws`) is currently supported.



\## Running Tests



From the `pkg/go` directory:



```bash

go test -v ./...

```



Format the source code using:



```bash

gofmt -w hashedtokens.go hashedtokens\_test.go

```



The current tests cover:



\- Successful HTTP and WebSocket request flow

\- EDMS error status handling

\- Request timeout behavior



Tests use a simulated EDMS server.



Integration testing against a running EDMS backend is still required.



\## Dependencies



No external Go modules are required.



The implementation uses only Go standard-library packages.



\## Current Limitations



\- Supports HTTP EDMS server URLs, not HTTPS/WSS

\- Uses the EDMS `test\_and\_insert` collections operation

\- Does not implement every WebSocket extension

\- Does not currently fall back to polling the EDMS jobs endpoint



\## Repository



Official EDMS repository:



https://github.com/hashedtokens/edms



Package location:



`pkg/go/`



