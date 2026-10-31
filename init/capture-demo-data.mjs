// init/capture-demo-data.mjs — MAINTAINER TOOL, not run on first boot.
//
// Produces the recorded demo bundle that seed.mjs loads offline. It runs the
// demo endpoints for real against their public APIs on a scratch backend,
// then takes the result out with the product's own takeout:
//
//   init/demo-data/demo-bundle/
//     repoview-manifest.json     every endpoint: URL, method, note, tags and
//                                its QPs' status codes and timings
//     globalEQPData/{eid}/...    the recorded request / response / headers
//
// Re-run it only when the demo endpoint list changes or you want fresher
// recordings; commit the resulting folder.
//
// Usage (start a scratch backend on an EMPTY database first, with its
// storage root somewhere you can read):
//   EDMS_BASE=http://localhost:3000 EDMS_STORAGE=/path/to/that/storage-root \
//     node init/capture-demo-data.mjs
//
// Requires Node 22+ and outbound internet for the backend.

import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.EDMS_BASE || "http://localhost:3000";
const WS_BASE = BASE.replace(/^http/, "ws");
const STORAGE = process.env.EDMS_STORAGE;
const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(HERE, "demo-data", "demo-bundle");

if (!STORAGE && !process.argv.includes("--scrub-only")) {
  console.error("Set EDMS_STORAGE to the backend's storage root (the folder that holds storage/takeout).");
  process.exit(2);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function post(path, body) {
  const r = await fetch(BASE + path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  let json = null;
  try { json = await r.json(); } catch {}
  return { status: r.status, ok: r.ok, json };
}

async function get(path) {
  const r = await fetch(BASE + path);
  let json = null;
  try { json = await r.json(); } catch {}
  return { status: r.status, ok: r.ok, json };
}

function wsOnce(path, send, waitType, timeoutMs = 15000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_BASE + path);
    let done = false;
    const finish = (v) => {
      if (done) return;
      done = true;
      clearTimeout(t);
      try { ws.close(); } catch {}
      resolve(v);
    };
    const t = setTimeout(() => finish(null), timeoutMs);
    ws.addEventListener("open", () => {
      if (send !== undefined) ws.send(typeof send === "string" ? send : JSON.stringify(send));
    });
    ws.addEventListener("message", (e) => {
      let m;
      try { m = JSON.parse(e.data); } catch { return; }
      const ty = m?.event?.type || m?.type;
      if (!waitType || ty === waitType) finish(m);
    });
    ws.addEventListener("error", (err) => {
      if (!done) { done = true; clearTimeout(t); reject(err); }
    });
  });
}

// The demo endpoints. Each was probed before it went in. `runs` is how many
// times to test it (>1 records several request/response sets).
const JP = "https://jsonplaceholder.typicode.com";
const HB = "https://httpbin.org";
const PE = "https://postman-echo.com";

export const ENDPOINTS = [
  { method: "GET",    url: `${JP}/posts`,            note: "List all posts.", runs: 2 },
  { method: "GET",    url: `${JP}/posts/1`,          note: "Fetch one post by ID.", runs: 1 },
  { method: "GET",    url: `${JP}/posts/1/comments`, note: "Comments on a post.", runs: 1 },
  { method: "GET",    url: `${JP}/users/1`,          note: "Fetch the author's profile.", runs: 1 },
  { method: "POST",   url: `${JP}/posts`,            note: "Create a post (simulated - nothing is persisted).", runs: 1 },
  { method: "PUT",    url: `${JP}/posts/1`,          note: "Replace a post.", runs: 1 },
  { method: "DELETE", url: `${JP}/posts/1`,          note: "Delete a post.", runs: 1 },
  { method: "GET",    url: `${JP}/posts/9999`,       note: "A post that does not exist: expect 404.", runs: 1 },

  { method: "GET",    url: `${HB}/get`,              note: "Echo a GET request.", runs: 2 },
  { method: "POST",   url: `${HB}/post`,             note: "Echo a POST body.", runs: 1 },
  { method: "PUT",    url: `${HB}/put`,              note: "Echo a PUT body.", runs: 1 },
  { method: "PATCH",  url: `${HB}/patch`,            note: "Echo a PATCH body.", runs: 1 },
  { method: "DELETE", url: `${HB}/delete`,           note: "Echo a DELETE request.", runs: 1 },
  { method: "GET",    url: `${HB}/status/404`,       note: "Always answers 404 Not Found.", runs: 1 },
  { method: "GET",    url: `${HB}/status/401`,       note: "Always answers 401 Unauthorized.", runs: 1 },
  { method: "GET",    url: `${HB}/uuid`,             note: "Return a fresh random UUID.", runs: 1 },
  { method: "GET",    url: `${PE}/get?foo=bar`,      note: "Echo a GET with a query string.", runs: 1 },
  { method: "POST",   url: `${PE}/post`,             note: "Echo a POST body.", runs: 1 },
  { method: "GET",    url: `${PE}/status/201`,       note: "Always answers 201 Created.", runs: 1 },

  { method: "GET", url: "https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41&current_weather=true", note: "Current weather for Berlin.", runs: 1 },
  { method: "GET", url: "https://restcountries.com/v3.1/name/france",               note: "Country details by name.", runs: 1 },
  { method: "GET", url: "https://openlibrary.org/search.json?q=the+hobbit&limit=3", note: "Book search.", runs: 1 },
  { method: "GET", url: "https://api.agify.io/?name=michael",                       note: "Predict an age from a first name.", runs: 1 },
  { method: "GET", url: "https://dummyjson.com/products?limit=3",                   note: "Sample store products.", runs: 1 },

  { method: "GET", url: "https://pokeapi.co/api/v2/pokemon/ditto",   note: "One Pokemon, with its full detail (a large response).", runs: 1 },
  { method: "GET", url: "https://pokeapi.co/api/v2/pokemon?limit=5", note: "A page of Pokemon names.", runs: 1 },
  { method: "GET", url: "https://catfact.ninja/fact",                note: "A random cat fact.", runs: 1 },
  { method: "GET", url: "https://catfact.ninja/breeds?limit=3",      note: "Cat breeds, paginated.", runs: 1 },
  { method: "GET", url: "https://dog.ceo/api/breeds/image/random",   note: "A random dog picture.", runs: 1 },
  { method: "GET", url: "https://official-joke-api.appspot.com/random_joke", note: "A random joke.", runs: 1 },
];

// Tags go on the endpoints before the takeout, so they travel in the manifest.
function tagsFor(e) {
  const tags = [e.method === "GET" ? "read-only" : "write"];
  if (/\/status\/(401|404)$|\/posts\/9999$/.test(e.url)) tags.push("error-case");
  if (/httpbin|postman-echo/.test(e.url)) tags.push("echo");
  if (/open-meteo|restcountries|openlibrary|agify|dummyjson/.test(e.url)) tags.push("public-data");
  return tags;
}

async function runOnce(e, eid) {
  const payload = { endpoint_str: e.url, method: e.method, body: {}, timeout_ms: 8000, tick_interval_ms: 300 };
  if (eid) payload.endpoint_id = eid;
  else payload.annotation = e.note;
  return wsOnce("/test-view/run", { type: "run_test", payload }, "TestFinished", 18000);
}

async function capture(e) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const first = await runOnce(e, null);
    const eid = first?.event?.payload?.endpoint_id;
    if (eid) {
      for (let k = 1; k < (e.runs || 1); k++) await runOnce(e, eid);
      return eid;
    }
    await sleep(2000);
  }
  return null;
}

async function waitJob(id, timeoutMs = 120000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const r = await get(`/jobs/${id}`);
    if (r.ok && r.json.status !== "running") return r.json;
    await sleep(300);
  }
  throw new Error(`job ${id} did not finish`);
}

// What a recording can leak about the machine that captured it, replaced
// with neutral values so nothing identifying is ever committed:
//   * the caller's PUBLIC IP, which echo services (httpbin, postman-echo)
//     put in their bodies           -> an RFC 5737 documentation address
//   * the CDN data-center code in cf-ray / x-amz-cf-pop, a coarse location
//     hint (e.g. "-EWR" = Newark)   -> XXX
//   * cookie values (Cloudflare's _cfuvid etc.), CDN reporting tokens and
//     request / trace ids, all opaque per-connection identifiers
// Header NAMES and everything else are left alone. Idempotent.
const DOC_IP = "203.0.113.77";
const ZEROS16 = "0000000000000000";
const ZEROS32 = "00000000000000000000000000000000";

const IP_FIELD = /("(?:origin|x-forwarded-for|ip)"\s*:\s*")(\d{1,3}(?:\.\d{1,3}){3}(?:\s*,\s*\d{1,3}(?:\.\d{1,3}){3})*)(")/gi;
const CF_RAY = /("cf-ray"\s*:\s*")[0-9a-f]+-[A-Z]{3}(")/gi;
const CF_POP = /("x-amz-cf-pop"\s*:\s*")[A-Z0-9-]+(")/gi;
const CF_ID = /("x-amz-cf-id"\s*:\s*")[^"]+(")/gi;
const REQ_ID = /("(?:x-request-id|x-amzn-requestid|x-amzn-trace-id|cf-request-id|x-correlation-id)"\s*:\s*")[^"]+(")/gi;
const NEL_TOKEN = /(nel\.cloudflare\.com\?\/report\?\/v4\?s=)[A-Za-z0-9%_.~+=-]+/g;
const VIA_ID = /[0-9a-f]{32}(\.cloudfront\.net)/g;
const AMZN_TRACE = /Root=1-[0-9a-f]{8}-[0-9a-f]{24}/g;

// "_cfuvid=abc; Path=/; HttpOnly" -> "_cfuvid=redacted; Path=/; HttpOnly"
const redactCookie = (c) => c.replace(/^(\s*[^=;\s]+)=([^;]*)/, "$1=redacted");
const SET_COOKIE_STR = /("set-cookie"\s*:\s*")([^"]*)(")/gi;
const SET_COOKIE_ARR = /("set-cookie"\s*:\s*\[)([^\]]*)(\])/gi;

export function scrubText(text) {
  return text
    .replace(IP_FIELD, (_, a, _ip, c) => `${a}${DOC_IP}${c}`)
    .replace(CF_RAY, (_, a, c) => `${a}${ZEROS16}-XXX${c}`)
    .replace(CF_POP, (_, a, c) => `${a}XXX00-P0${c}`)
    .replace(CF_ID, (_, a, c) => `${a}${ZEROS16}${c}`)
    .replace(REQ_ID, (_, a, c) => `${a}redacted${c}`)
    .replace(NEL_TOKEN, (_, a) => `${a}redacted`)
    .replace(VIA_ID, (_, a) => `${ZEROS32}${a}`)
    .replace(AMZN_TRACE, `Root=1-00000000-${"0".repeat(24)}`)
    .replace(SET_COOKIE_STR, (_, a, v, c) => `${a}${redactCookie(v)}${c}`)
    .replace(SET_COOKIE_ARR, (_, a, v, c) => `${a}${v.replace(/"([^"]*)"/g, (_m, s) => `"${redactCookie(s)}"`)}${c}`);
}

function scrub(dir) {
  let changed = 0;
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) { changed += scrub(p); continue; }
    if (!n.endsWith(".json")) continue;
    const before = readFileSync(p, "utf8");
    const after = scrubText(before);
    if (after !== before) {
      JSON.parse(after); // never write a file the scrub broke
      writeFileSync(p, after);
      changed++;
    }
  }
  return changed;
}

async function main() {
  // Re-apply the scrub to the committed bundle without a backend or capture.
  if (process.argv.includes("--scrub-only")) {
    console.log(`[capture] scrubbed ${scrub(OUT)} file(s) in ${OUT}`);
    return;
  }
  const snap = await wsOnce("/test-view/endpoints/load", undefined, "snapshot");
  if ((snap?.endpoints?.length || 0) > 0) {
    throw new Error("the scratch backend must start EMPTY (it already has endpoints)");
  }

  console.log(`[capture] running ${ENDPOINTS.length} endpoints …`);
  const eids = [];
  for (const e of ENDPOINTS) {
    const eid = await capture(e);
    if (!eid) throw new Error(`no result for ${e.method} ${e.url} - is it reachable from the backend?`);
    eids.push(eid);
    for (const t of tagsFor(e)) await post(`/tags/${eid}/add`, { tag: t });
    process.stdout.write(".");
  }
  console.log(`\n[capture] ${eids.length} endpoints recorded`);

  // One temporary collection holding all of them -> one RepoView -> takeout.
  await post("/collections/create", { name: "demo-capture" });
  for (const eid of eids) {
    await post("/test-view/save/bookmark", { collection: "demo-capture", endpoint_id: eid, notes: "" });
    await post(`/bookmarks/demo-capture/${eid}/save`);
  }
  const made = await post("/repoview/create", { name: "demo-bundle", source_collection: "demo-capture" });
  if (!made.ok) throw new Error(`repoview create failed: ${JSON.stringify(made.json)}`);

  const t = await post("/repoview/demo-bundle/takeout", { overwrite: true });
  if (t.status !== 202) throw new Error(`takeout not accepted: ${JSON.stringify(t.json)}`);
  const job = await waitJob(t.json.job_id);
  if (job.status !== "done" || job.result?.ok === false) {
    throw new Error(`takeout failed: ${JSON.stringify(job)}`);
  }
  console.log(`[capture] takeout: ${job.result.eqp_folders_copied} folders, ${job.result.eqp_files_copied} files`);

  const src = join(STORAGE, "storage", "takeout", "demo-bundle");
  if (!existsSync(src)) throw new Error(`expected the takeout at ${src}`);
  // Empty the folder rather than deleting it: on Windows a folder can't be
  // removed while any process (a shell, an editor) has it as its cwd.
  mkdirSync(OUT, { recursive: true });
  for (const n of readdirSync(OUT)) rmSync(join(OUT, n), { recursive: true, force: true });
  cpSync(src, OUT, { recursive: true });
  console.log(`[capture] scrubbed caller IPs from ${scrub(OUT)} file(s)`);

  let files = 0;
  let bytes = 0;
  const walk = (d) => {
    for (const n of readdirSync(d)) {
      const p = join(d, n);
      const s = statSync(p);
      if (s.isDirectory()) walk(p);
      else { files++; bytes += s.size; }
    }
  };
  walk(OUT);
  console.log(`[capture] wrote ${OUT}: ${files} files, ${(bytes / 1024).toFixed(0)} KiB`);
}

main().then(() => process.exit(0)).catch((e) => {
  console.error("[capture] FAILED:", e.message);
  process.exit(1);
});
