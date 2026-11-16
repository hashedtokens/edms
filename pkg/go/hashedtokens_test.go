package hashedtokens

import (
	"crypto/sha1"
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

// writeTextFrame sends an unmasked WebSocket text frame (server-to-client).
func writeTextFrame(conn net.Conn, body string) error {
	n := len(body)
	header := []byte{0x81}
	if n < 126 {
		header = append(header, byte(n))
	} else {
		header = append(header, 126, 0, 0)
		binary.BigEndian.PutUint16(header[2:4], uint16(n))
	}
	_, err := conn.Write(append(header, body...))
	return err
}

func mockEDMS(t *testing.T, statusCode int, deliver bool) (*httptest.Server, <-chan map[string]any) {
	t.Helper()
	posted := make(chan map[string]any, 1)
	seen := make(chan map[string]any, 1)
	mux := http.NewServeMux()

	mux.HandleFunc("/hashedtokens/edms/api/ws", func(w http.ResponseWriter, r *http.Request) {
		conn, _, err := w.(http.Hijacker).Hijack()
		if err != nil {
			t.Errorf("hijack: %v", err)
			return
		}
		defer conn.Close()

		key := r.Header.Get("Sec-WebSocket-Key")
		digest := sha1.Sum([]byte(key + wsGUID))
		accept := base64.StdEncoding.EncodeToString(digest[:])

		if _, err := fmt.Fprintf(conn,
			"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: %s\r\n\r\n",
			accept); err != nil {
			t.Errorf("handshake: %v", err)
			return
		}

		if err := writeTextFrame(conn, `{"type":"transactions","transactions":[]}`); err != nil {
			t.Errorf("snapshot: %v", err)
			return
		}

		if !deliver {
			_, _ = io.Copy(io.Discard, conn)
			return
		}

		select {
		case <-posted:
		case <-time.After(3 * time.Second):
			t.Error("no POST received")
			return
		}

		name := "OK"
		if statusCode != 0 {
			name = "TEST_FAILED"
		}

		event := fmt.Sprintf(
			`{"type":"event","event":{"type":"DevApiResult","payload":{"transaction_id":"tx-123","op":"api_collections_test_and_insert","result":{"status":{"code":%d,"name":%q,"message":"done"},"endpoint_id":"E0001-AAA","request_number":1,"target_status":200,"response":{"status":200,"body":{"ok":true}}}}}}`,
			statusCode, name,
		)

		if err := writeTextFrame(conn, event); err != nil {
			t.Errorf("event: %v", err)
		}

		_, _ = io.Copy(io.Discard, conn)
	})

	mux.HandleFunc("/hashedtokens/edms/api/collections/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("wrong method %q", r.Method)
		}

		var body map[string]any
		if err := json.NewDecoder(r.Body).Decode(&body); err != nil {
			t.Errorf("POST body: %v", err)
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusAccepted)
		_, _ = io.WriteString(w, `{"transaction_id":"tx-123"}`)

		posted <- body
		seen <- body
	})

	return httptest.NewServer(mux), seen
}

func TestRequestSuccess(t *testing.T) {
	server, posted := mockEDMS(t, 0, true)
	defer server.Close()

	result := <-Request("https://example.com/users", map[string]any{"name": "Ada"},
		WithServer(server.URL),
		WithCollection("demo"),
		WithMethod("POST"),
		WithHeaders(map[string]string{"X-Test": "1"}),
		WithTimeout(2*time.Second))

	if result.Err != nil {
		t.Fatalf("Request failed: %v", result.Err)
	}

	if result.EDMS.Status.Code != 0 || result.EDMS.EndpointID != "E0001-AAA" {
		t.Fatalf("unexpected EDMS result: %+v", result.EDMS)
	}

	if !strings.Contains(string(result.Response), `"ok":true`) {
		t.Fatalf("missing API response: %s", result.Response)
	}

	body := <-posted
	if body["ops"] != "test_and_insert" || body["collection"] != "demo" || body["method"] != "POST" {
		t.Fatalf("wrong POST body: %+v", body)
	}

	if body["return_response"] != true {
		t.Fatal("return_response must be true")
	}
}

func TestRequestStatusError(t *testing.T) {
	server, _ := mockEDMS(t, 105, true)
	defer server.Close()

	result := <-Request("https://example.com/fail", nil,
		WithServer(server.URL),
		WithTimeout(2*time.Second))

	if result.Err == nil || !strings.Contains(result.Err.Error(), "105") {
		t.Fatalf("expected EDMS code 105, got: %+v", result)
	}
}

func TestRequestTimeout(t *testing.T) {
	server, _ := mockEDMS(t, 0, false)
	defer server.Close()

	start := time.Now()

	result := <-Request("https://example.com/slow", nil,
		WithServer(server.URL),
		WithTimeout(100*time.Millisecond))

	if result.Err == nil {
		t.Fatal("expected timeout error")
	}

	if time.Since(start) > 2*time.Second {
		t.Fatal("timeout took too long")
	}
}
