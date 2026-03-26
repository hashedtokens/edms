-- EDMS Demo Seed Data
-- Run with: sqlite3 demo.db < seed_data.sql

-- Create endpoints table if it doesn't exist
CREATE TABLE IF NOT EXISTS endpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT UNIQUE NOT NULL,
    endpoint_str TEXT NOT NULL,
    annotation TEXT
);

-- Create bookmarks table if it doesn't exist
CREATE TABLE IF NOT EXISTS bookmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    folder TEXT NOT NULL DEFAULT '__active__',
    notes TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create history table if it doesn't exist
CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    action TEXT NOT NULL,
    details TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create request_metadata table if it doesn't exist
CREATE TABLE IF NOT EXISTS request_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    request_number INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    method TEXT NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create response_metadata table if it doesn't exist
CREATE TABLE IF NOT EXISTS response_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    request_number INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    response_time_ms INTEGER,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ============================================
-- SEED ENDPOINTS
-- ============================================

-- Clear existing demo data (optional - comment out to append)
-- DELETE FROM endpoints;
-- DELETE FROM bookmarks WHERE folder = '__active__';

-- HTTPBin Endpoints (great for testing any HTTP method)
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('httpbin-get', 'https://httpbin.org/get', 'GET request - returns headers, args, origin IP'),
('httpbin-post', 'https://httpbin.org/post', 'POST request - echoes back posted JSON data'),
('httpbin-put', 'https://httpbin.org/put', 'PUT request - echoes back data'),
('httpbin-delete', 'https://httpbin.org/delete', 'DELETE request - confirms deletion'),
('httpbin-patch', 'https://httpbin.org/patch', 'PATCH request - partial update'),
('httpbin-status-200', 'https://httpbin.org/status/200', 'Returns HTTP 200 OK'),
('httpbin-status-201', 'https://httpbin.org/status/201', 'Returns HTTP 201 Created'),
('httpbin-status-400', 'https://httpbin.org/status/400', 'Returns HTTP 400 Bad Request'),
('httpbin-status-401', 'https://httpbin.org/status/401', 'Returns HTTP 401 Unauthorized'),
('httpbin-status-404', 'https://httpbin.org/status/404', 'Returns HTTP 404 Not Found'),
('httpbin-status-500', 'https://httpbin.org/status/500', 'Returns HTTP 500 Server Error'),
('httpbin-status-503', 'https://httpbin.org/status/503', 'Returns HTTP 503 Service Unavailable'),
('httpbin-delay-1', 'https://httpbin.org/delay/1', 'Delays response by 1 second'),
('httpbin-delay-3', 'https://httpbin.org/delay/3', 'Delays response by 3 seconds'),
('httpbin-delay-5', 'https://httpbin.org/delay/5', 'Delays response by 5 seconds (test timeout)'),
('httpbin-headers', 'https://httpbin.org/headers', 'Returns all request headers'),
('httpbin-ip', 'https://httpbin.org/ip', 'Returns your origin IP address'),
('httpbin-user-agent', 'https://httpbin.org/user-agent', 'Returns your User-Agent'),
('httpbin-uuid', 'https://httpbin.org/uuid', 'Returns a random UUID4'),
('httpbin-json', 'https://httpbin.org/json', 'Returns sample JSON data'),
('httpbin-html', 'https://httpbin.org/html', 'Returns sample HTML page'),
('httpbin-xml', 'https://httpbin.org/xml', 'Returns sample XML data'),
('httpbin-gzip', 'https://httpbin.org/gzip', 'Returns gzip-encoded data'),
('httpbin-deflate', 'https://httpbin.org/deflate', 'Returns deflate-encoded data'),
('httpbin-cookies', 'https://httpbin.org/cookies', 'Returns cookie data'),
('httpbin-anything', 'https://httpbin.org/anything', 'Returns anything passed to request');

-- JSONPlaceholder (fake REST API for testing)
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('jp-posts', 'https://jsonplaceholder.typicode.com/posts', 'List all posts (100 items)'),
('jp-post-1', 'https://jsonplaceholder.typicode.com/posts/1', 'Get single post by ID'),
('jp-post-comments', 'https://jsonplaceholder.typicode.com/posts/1/comments', 'Get comments for post #1'),
('jp-users', 'https://jsonplaceholder.typicode.com/users', 'List all users (10 items)'),
('jp-user-1', 'https://jsonplaceholder.typicode.com/users/1', 'Get single user by ID'),
('jp-comments', 'https://jsonplaceholder.typicode.com/comments', 'List all comments (500 items)'),
('jp-albums', 'https://jsonplaceholder.typicode.com/albums', 'List all albums'),
('jp-photos', 'https://jsonplaceholder.typicode.com/photos', 'List all photos (5000 items!)'),
('jp-todos', 'https://jsonplaceholder.typicode.com/todos', 'List all todos (200 items)'),
('jp-todo-1', 'https://jsonplaceholder.typicode.com/todos/1', 'Get single todo by ID');

-- ReqRes (fake user management API)
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('reqres-users', 'https://reqres.in/api/users?page=1', 'List users page 1'),
('reqres-users-p2', 'https://reqres.in/api/users?page=2', 'List users page 2'),
('reqres-user-2', 'https://reqres.in/api/users/2', 'Get user #2 (Janet Weaver)'),
('reqres-user-23', 'https://reqres.in/api/users/23', 'Get user #23 (returns 404)'),
('reqres-create', 'https://reqres.in/api/users', 'Create user (POST with name/job)'),
('reqres-update', 'https://reqres.in/api/users/2', 'Update user (PUT with name/job)'),
('reqres-delete', 'https://reqres.in/api/users/2', 'Delete user (returns 204)'),
('reqres-register', 'https://reqres.in/api/register', 'Register (POST email/password)'),
('reqres-login', 'https://reqres.in/api/login', 'Login (POST email/password)'),
('reqres-delayed', 'https://reqres.in/api/users?delay=3', 'Delayed response (3 seconds)');

-- Fun APIs (publicly accessible, no auth)
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('dog-random', 'https://dog.ceo/api/breeds/image/random', 'Random dog image URL'),
('dog-breeds', 'https://dog.ceo/api/breeds/list/all', 'List all dog breeds'),
('dog-hound', 'https://dog.ceo/api/breed/hound/images/random', 'Random hound image'),
('cat-fact', 'https://catfact.ninja/fact', 'Random cat fact'),
('cat-facts-5', 'https://catfact.ninja/facts?limit=5', 'Five cat facts'),
('cat-breeds', 'https://catfact.ninja/breeds', 'List cat breeds'),
('joke-random', 'https://official-joke-api.appspot.com/random_joke', 'Random joke'),
('joke-programming', 'https://official-joke-api.appspot.com/jokes/programming/random', 'Programming joke'),
('quote-random', 'https://api.quotable.io/random', 'Random inspirational quote'),
('activity-random', 'https://www.boredapi.com/api/activity', 'Random activity suggestion');

-- GitHub Public API (no auth needed)
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('github-zen', 'https://api.github.com/zen', 'GitHub zen wisdom'),
('github-octocat', 'https://api.github.com/octocat', 'Octocat ASCII art'),
('github-rate', 'https://api.github.com/rate_limit', 'Check API rate limit'),
('github-emojis', 'https://api.github.com/emojis', 'List all GitHub emojis'),
('github-events', 'https://api.github.com/events', 'Public events timeline');

-- Weather (no auth, JSON format)  
INSERT OR REPLACE INTO endpoints (endpoint_id, endpoint_str, annotation) VALUES
('wttr-madison', 'https://wttr.in/Madison,WI?format=j1', 'Weather in Madison, WI'),
('wttr-sf', 'https://wttr.in/San+Francisco?format=j1', 'Weather in San Francisco'),
('wttr-nyc', 'https://wttr.in/New+York?format=j1', 'Weather in New York'),
('wttr-london', 'https://wttr.in/London?format=j1', 'Weather in London'),
('wttr-tokyo', 'https://wttr.in/Tokyo?format=j1', 'Weather in Tokyo');

-- ============================================
-- SEED INITIAL BOOKMARKS (Active folder)
-- ============================================

INSERT OR REPLACE INTO bookmarks (endpoint_id, folder, notes) VALUES
('httpbin-get', '__active__', 'Primary test endpoint'),
('httpbin-post', '__active__', 'Test POST requests'),
('jp-posts', '__active__', 'Blog posts API'),
('reqres-users', '__active__', 'User management'),
('dog-random', '__active__', 'Fun: random dogs!');

-- ============================================
-- SEED SAMPLE HISTORY
-- ============================================

INSERT INTO history (endpoint_id, action, details) VALUES
('httpbin-get', 'test_finished', '{"status_code": 200, "response_time_ms": 156}'),
('httpbin-post', 'test_finished', '{"status_code": 200, "response_time_ms": 203}'),
('jp-posts', 'test_finished', '{"status_code": 200, "response_time_ms": 89}'),
('httpbin-status-404', 'test_finished', '{"status_code": 404, "response_time_ms": 112}'),
('httpbin-delay-1', 'test_finished', '{"status_code": 200, "response_time_ms": 1045}');

-- Show summary
SELECT 'Seeded ' || COUNT(*) || ' endpoints' FROM endpoints;
SELECT 'Seeded ' || COUNT(*) || ' bookmarks' FROM bookmarks WHERE folder = '__active__';
SELECT 'Seeded ' || COUNT(*) || ' history entries' FROM history;
