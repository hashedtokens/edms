package hashedtokens

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha1"
	"encoding/base64"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
	"time"
)

const defaultTimeout = 15 * time.Second
const maxWSMessage = 4 << 20
const wsGUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

// Status is EDMS's final status code.
type Status struct {
	Code    int    `json:"code"`
	Name    string `json:"name"`
	Message string `json:"message"`
}

// APIResult is the final result delivered by EDMS over WebSocket.
type APIResult struct {
	Status            Status          `json:"status"`
	EndpointID        string          `json:"endpoint_id,omitempty"`
	RequestNumber     int             `json:"request_number,omitempty"`
	DuplicateOf       int             `json:"duplicate_of,omitempty"`
	TargetStatus      int             `json:"target_status,omitempty"`
	EndpointCreated   bool            `json:"endpoint_created,omitempty"`
	CollectionCreated bool            `json:"collection_created,omitempty"`
	Response          json.RawMessage `json:"response,omitempty"`
	Detail            string          `json:"detail,omitempty"`
}

// Result contains the API response, EDMS result, and any error.
type Result struct {
	Response json.RawMessage
	EDMS     APIResult
	Err      error
}

type config struct {
	server     string
	collection string
	method     string
	timeout    time.Duration
	headers    map[string]string
}

// Option configures an EDMS request.
type Option func(*config)

func WithServer(server string) Option {
	return func(c *config) {
		c.server = strings.TrimRight(server, "/")
	}
}

func WithCollection(name string) Option {
	return func(c *config) {
		c.collection = name
	}
}

func WithMethod(method string) Option {
	return func(c *config) {
		c.method = strings.ToUpper(method)
	}
}

func WithTimeout(timeout time.Duration) Option {
	return func(c *config) {
		c.timeout = timeout
	}
}

func WithHeaders(headers map[string]string) Option {
	return func(c *config) {
		c.headers = make(map[string]string, len(headers))

		for key, value := range headers {
			c.headers[key] = value
		}
	}
}

// Request executes asynchronously and returns a channel immediately.
// EDMS calls the target API once and stores its request and response.
func Request(
	endpoint string,
	payload any,
	options ...Option,
) <-chan Result {
	return RequestContext(
		context.Background(),
		endpoint,
		payload,
		options...,
	)
}

// RequestContext also supports caller cancellation.
func RequestContext(
	ctx context.Context,
	endpoint string,
	payload any,
	options ...Option,
) <-chan Result {
	c := config{
		server:     "http://127.0.0.1:3000",
		collection: "go-package",
		timeout:    defaultTimeout,
	}

	for _, option := range options {
		if option != nil {
			option(&c)
		}
	}

	if c.method == "" {
		if payload == nil {
			c.method = http.MethodGet
		} else {
			c.method = http.MethodPost
		}
	}

	results := make(chan Result, 1)

	go func() {
		defer close(results)

		if c.timeout <= 0 {
			results <- Result{
				Err: errors.New("timeout must be positive"),
			}
			return
		}

		requestCtx, cancel := context.WithTimeout(ctx, c.timeout)
		defer cancel()

		results <- execute(requestCtx, endpoint, payload, c)
	}()

	return results
}

func execute(
	ctx context.Context,
	endpoint string,
	payload any,
	c config,
) Result {
	// Connect to WebSocket before POSTing so no status event is missed.
	socket, err := connectWS(ctx, c.server)
	if err != nil {
		return Result{
			Err: fmt.Errorf("connect to EDMS WebSocket: %w", err),
		}
	}
	defer socket.conn.Close()

	body := map[string]any{
		"ops":             "test_and_insert",
		"endpoint_str":    endpoint,
		"method":          c.method,
		"collection":      c.collection,
		"return_response": true,
	}

	if payload != nil {
		body["request_body"] = payload
	}

	if len(c.headers) != 0 {
		body["headers"] = c.headers
	}

	encoded, err := json.Marshal(body)
	if err != nil {
		return Result{
			Err: fmt.Errorf("encode EDMS request: %w", err),
		}
	}

	apiURL := c.server + "/hashedtokens/edms/api/collections/"

	req, err := http.NewRequestWithContext(
		ctx,
		http.MethodPost,
		apiURL,
		bytes.NewReader(encoded),
	)
	if err != nil {
		return Result{Err: err}
	}

	req.Header.Set("Content-Type", "application/json")

	response, err := http.DefaultClient.Do(req)
	if err != nil {
		return Result{
			Err: fmt.Errorf("EDMS POST: %w", err),
		}
	}
	defer response.Body.Close()

	if response.StatusCode != http.StatusAccepted {
		message, _ := io.ReadAll(
			io.LimitReader(response.Body, 8192),
		)

		return Result{
			Err: fmt.Errorf(
				"EDMS HTTP %d: %s",
				response.StatusCode,
				message,
			),
		}
	}

	var accepted struct {
		TransactionID string `json:"transaction_id"`
	}

	if err := json.NewDecoder(response.Body).Decode(&accepted); err != nil {
		return Result{
			Err: fmt.Errorf("decode EDMS response: %w", err),
		}
	}

	if accepted.TransactionID == "" {
		return Result{
			Err: errors.New("EDMS did not return a transaction_id"),
		}
	}

	// Wait for the matching WebSocket result.
	final, err := socket.await(ctx, accepted.TransactionID)
	if err != nil {
		return Result{
			Err: fmt.Errorf("EDMS status: %w", err),
		}
	}

	result := Result{
		Response: final.Response,
		EDMS:     final,
	}

	// 0 = OK; 104 = duplicate QP already stored.
	if final.Status.Code != 0 && final.Status.Code != 104 {
		result.Err = fmt.Errorf(
			"EDMS %s (%d): %s",
			final.Status.Name,
			final.Status.Code,
			final.Detail,
		)
	}

	return result
}

// WebSocket implementation using only Go's standard library.

type wsClient struct {
	conn   net.Conn
	reader *bufio.Reader
}

func connectWS(ctx context.Context, server string) (*wsClient, error) {
	u, err := url.Parse(server)

	if err != nil ||
		u.Scheme != "http" ||
		u.Hostname() == "" ||
		(u.Path != "" && u.Path != "/") {
		return nil, fmt.Errorf(
			"EDMS server must be an http://host:port URL",
		)
	}

	port := u.Port()
	if port == "" {
		port = "80"
	}

	address := net.JoinHostPort(u.Hostname(), port)

	conn, err := (&net.Dialer{}).DialContext(
		ctx,
		"tcp",
		address,
	)
	if err != nil {
		return nil, err
	}

	success := false
	defer func() {
		if !success {
			conn.Close()
		}
	}()

	if deadline, ok := ctx.Deadline(); ok {
		conn.SetDeadline(deadline)
	}

	stop := context.AfterFunc(ctx, func() {
		conn.Close()
	})
	defer stop()

	keyBytes := make([]byte, 16)

	if _, err := rand.Read(keyBytes); err != nil {
		return nil, err
	}

	key := base64.StdEncoding.EncodeToString(keyBytes)

	wsURL := "http://" + u.Host + "/hashedtokens/edms/api/ws"

	request, err := http.NewRequestWithContext(
		ctx,
		http.MethodGet,
		wsURL,
		nil,
	)
	if err != nil {
		return nil, err
	}

	request.Header.Set("Connection", "Upgrade")
	request.Header.Set("Upgrade", "websocket")
	request.Header.Set("Sec-WebSocket-Version", "13")
	request.Header.Set("Sec-WebSocket-Key", key)

	if err := request.Write(conn); err != nil {
		return nil, err
	}

	reader := bufio.NewReader(conn)

	response, err := http.ReadResponse(reader, request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	accept := sha1.Sum([]byte(key + wsGUID))
	expected := base64.StdEncoding.EncodeToString(accept[:])

	if response.StatusCode != http.StatusSwitchingProtocols ||
		!strings.EqualFold(
			response.Header.Get("Upgrade"),
			"websocket",
		) ||
		!strings.EqualFold(
			response.Header.Get("Connection"),
			"Upgrade",
		) ||
		response.Header.Get("Sec-WebSocket-Accept") != expected {
		return nil, fmt.Errorf(
			"invalid WebSocket upgrade (HTTP %d)",
			response.StatusCode,
		)
	}

	socket := &wsClient{
		conn:   conn,
		reader: reader,
	}

	// EDMS sends the initial transactions snapshot when ready.
	initial, err := socket.readMessage()
	if err != nil {
		return nil, err
	}

	var snapshot struct {
		Type string `json:"type"`
	}

	if json.Unmarshal(initial, &snapshot) != nil ||
		snapshot.Type != "transactions" {
		return nil, errors.New(
			"unexpected EDMS WebSocket handshake message",
		)
	}

	success = true
	return socket, nil
}

func (ws *wsClient) await(
	ctx context.Context,
	transactionID string,
) (APIResult, error) {
	stop := context.AfterFunc(ctx, func() {
		ws.conn.Close()
	})
	defer stop()

	for {
		message, err := ws.readMessage()

		if err != nil {
			if ctx.Err() != nil {
				return APIResult{}, ctx.Err()
			}
			return APIResult{}, err
		}

		var envelope struct {
			Type string `json:"type"`

			Event struct {
				Type string `json:"type"`

				Payload struct {
					TransactionID string    `json:"transaction_id"`
					Result        APIResult `json:"result"`
				} `json:"payload"`
			} `json:"event"`
		}

		if json.Unmarshal(message, &envelope) != nil {
			continue
		}

		if envelope.Type == "event" &&
			envelope.Event.Type == "DevApiResult" &&
			envelope.Event.Payload.TransactionID == transactionID {

			final := envelope.Event.Payload.Result

			if final.Status.Name == "" {
				return APIResult{}, errors.New(
					"EDMS result has no status",
				)
			}

			return final, nil
		}
	}
}

// readMessage reads a WebSocket text message.
func (ws *wsClient) readMessage() ([]byte, error) {
	var message []byte

	for {
		var header [2]byte

		if _, err := io.ReadFull(ws.reader, header[:]); err != nil {
			return nil, err
		}

		if header[0]&0x70 != 0 || header[1]&0x80 != 0 {
			return nil, errors.New(
				"unsupported WebSocket frame flags",
			)
		}

		opcode := header[0] & 0x0f
		fin := header[0]&0x80 != 0
		length := uint64(header[1] & 0x7f)

		if length == 126 {
			var data [2]byte

			if _, err := io.ReadFull(ws.reader, data[:]); err != nil {
				return nil, err
			}

			length = uint64(binary.BigEndian.Uint16(data[:]))

		} else if length == 127 {
			var data [8]byte

			if _, err := io.ReadFull(ws.reader, data[:]); err != nil {
				return nil, err
			}

			length = binary.BigEndian.Uint64(data[:])
		}

		if length > maxWSMessage ||
			uint64(len(message))+length > maxWSMessage {
			return nil, errors.New("WebSocket message too large")
		}

		if opcode >= 8 && (!fin || length > 125) {
			return nil, errors.New("invalid WebSocket control frame")
		}

		data := make([]byte, int(length))

		if _, err := io.ReadFull(ws.reader, data); err != nil {
			return nil, err
		}

		switch opcode {

		case 8:
			// Close frame.
			return nil, io.EOF

		case 9:
			// Respond to server ping.
			if err := ws.writeControl(10, data); err != nil {
				return nil, err
			}

		case 10:
			// Pong frame.

		case 1:
			// Text frame.
			if len(message) != 0 {
				return nil, errors.New(
					"unexpected new WebSocket text frame",
				)
			}

			message = append(message, data...)

			if fin {
				return message, nil
			}

		case 0:
			// Continuation frame.
			if message == nil {
				return nil, errors.New(
					"unexpected WebSocket continuation",
				)
			}

			message = append(message, data...)

			if fin {
				return message, nil
			}

		default:
			return nil, fmt.Errorf(
				"unsupported WebSocket opcode %d",
				opcode,
			)
		}
	}
}

// writeControl sends a masked WebSocket control frame.
func (ws *wsClient) writeControl(
	opcode byte,
	data []byte,
) error {
	var mask [4]byte

	if _, err := rand.Read(mask[:]); err != nil {
		return err
	}

	frame := []byte{
		0x80 | opcode,
		0x80 | byte(len(data)),
	}

	frame = append(frame, mask[:]...)

	for i, value := range data {
		frame = append(frame, value^mask[i%4])
	}

	for len(frame) > 0 {
		n, err := ws.conn.Write(frame)

		if err != nil {
			return err
		}

		frame = frame[n:]
	}

	return nil
}
