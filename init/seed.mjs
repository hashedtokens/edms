// init/seed.mjs — first-boot demo data for the EDMS stack.
//
// Loads a RECORDED dataset: nothing here calls a live URL. The runs were
// captured once (see capture-demo-data.mjs) and committed as
// demo-data/demo-bundle/ - a takeout holding 30 endpoints with their
// request / response / headers files, status codes, timings and tags.
//
// On first boot this script:
//   1. copies that bundle into the backend's imports folder and imports it
//      with the product's own import (fresh EIDs, QPs, tags, EQP files);
//   2. adds a history row per recorded run;
//   3. builds 4 Collections, a RepoView per Collection plus one combined by
//      tag, and 3 WebViews with front pages in the editor's JSON format;
//   4. removes the temporary import helper (the endpoints stay).
// Everything is ordinary user data afterwards: delete whatever you like.
//
// Safe by default: if the backend already has endpoints it does nothing, so
// a real database is never touched. SEED_FORCE=1 seeds anyway; endpoints
// that already exist for the same URL + method are reused (never
// duplicated) and objects that already exist by name are skipped.
//
// Runs automatically as the `seed` service in docker-compose.yml, then
// exits. Also runnable by hand (EDMS_STORAGE = the backend's storage root):
//   EDMS_BASE=http://localhost:3000 EDMS_STORAGE=/path/to/root node init/seed.mjs
//
// Needs Node 22+ (fetch + WebSocket) and write access to the storage root
// (it only writes the temporary copy under storage/imports/). No internet.

import { cpSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.EDMS_BASE || "http://localhost:3000";
const WS_BASE = BASE.replace(/^http/, "ws");
const STORAGE = process.env.EDMS_STORAGE || "/app/edms_root";
const HERE = dirname(fileURLToPath(import.meta.url));
const BUNDLE = process.env.DEMO_BUNDLE || resolve(HERE, "demo-data", "demo-bundle");
const FORCE = ["1", "true", "yes"].includes(String(process.env.SEED_FORCE || "").toLowerCase());

// The temporary RepoView / imports folder used to bring the bundle in. A
// distinctive name so it can never collide with something a user made.
const HELPER = "edms-demo-bundle";

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

// ── what goes where ─────────────────────────────────────────────────
// The bundle holds the recorded endpoints; this says which Collection each
// belongs to. Keyed by "METHOD url" exactly as recorded.

const JP = "https://jsonplaceholder.typicode.com";
const HB = "https://httpbin.org";
const PE = "https://postman-echo.com";

const LAYOUT = [
  ["placeholder-posts", "GET",    `${JP}/posts`],
  ["placeholder-posts", "GET",    `${JP}/posts/1`],
  ["placeholder-posts", "GET",    `${JP}/posts/1/comments`],
  ["placeholder-posts", "GET",    `${JP}/users/1`],
  ["placeholder-posts", "POST",   `${JP}/posts`],
  ["placeholder-posts", "PUT",    `${JP}/posts/1`],
  ["placeholder-posts", "DELETE", `${JP}/posts/1`],
  ["placeholder-posts", "GET",    `${JP}/posts/9999`],

  ["http-echo-tests", "GET",    `${HB}/get`],
  ["http-echo-tests", "POST",   `${HB}/post`],
  ["http-echo-tests", "PUT",    `${HB}/put`],
  ["http-echo-tests", "PATCH",  `${HB}/patch`],
  ["http-echo-tests", "DELETE", `${HB}/delete`],
  ["http-echo-tests", "GET",    `${HB}/status/404`],
  ["http-echo-tests", "GET",    `${HB}/status/401`],
  ["http-echo-tests", "GET",    `${HB}/uuid`],
  ["http-echo-tests", "GET",    `${PE}/get?foo=bar`],
  ["http-echo-tests", "POST",   `${PE}/post`],
  ["http-echo-tests", "GET",    `${PE}/status/201`],

  ["open-data", "GET", "https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41&current_weather=true"],
  ["open-data", "GET", "https://restcountries.com/v3.1/name/france"],
  ["open-data", "GET", "https://openlibrary.org/search.json?q=the+hobbit&limit=3"],
  ["open-data", "GET", "https://api.agify.io/?name=michael"],
  ["open-data", "GET", "https://dummyjson.com/products?limit=3"],

  ["fun-apis", "GET", "https://pokeapi.co/api/v2/pokemon/ditto"],
  ["fun-apis", "GET", "https://pokeapi.co/api/v2/pokemon?limit=5"],
  ["fun-apis", "GET", "https://catfact.ninja/fact"],
  ["fun-apis", "GET", "https://catfact.ninja/breeds?limit=3"],
  ["fun-apis", "GET", "https://dog.ceo/api/breeds/image/random"],
  ["fun-apis", "GET", "https://official-joke-api.appspot.com/random_joke"],
];

// `tags` are each Collection's own editable tags (the Tags column).
const COLLECTIONS = [
  { name: "placeholder-posts", tags: ["demo", "crud"],        annotation: "CRUD against a fake blog API (JSONPlaceholder)." },
  { name: "http-echo-tests",   tags: ["demo", "http-basics"], annotation: "Echo services for testing methods, status codes and bodies." },
  { name: "open-data",         tags: ["demo", "public"],      annotation: "Public reference data: weather, countries, books, names, products." },
  { name: "fun-apis",          tags: ["demo", "fun"],         annotation: "Small friendly APIs: Pokemon, cats, dogs and jokes." },
];

// RepoViews: one per Collection. `tags` are the view's own editable tags.
const REPOVIEWS = [
  { name: "blog-api",       col: "placeholder-posts", tags: ["stable", "crud"],        approach: "endpoint_segments", annotation: "The blog REST API, every CRUD method." },
  { name: "echo-suite",     col: "http-echo-tests",   tags: ["stable", "http-basics"], approach: "sorted_tags",       annotation: "Methods and status codes, including the deliberate 401/404 cases." },
  { name: "reference-data", col: "open-data",         tags: ["public", "stable"],      approach: "endpoint_segments", annotation: "Read-only public data sources." },
  { name: "playground",     col: "fun-apis",          tags: ["experimental"],          approach: "sorted_tags",       annotation: "Fun APIs; some responses change on every call." },
];
const COMBINED = { name: "all-stable", tags: ["stable"], annotation: "Everything tagged stable, combined." };

// ── rich-text editor documents (what a WebView front page stores) ───
const text = (t, marks) => (marks ? { type: "text", text: t, marks } : { type: "text", text: t });
const heading = (level, t) => ({ type: "heading", attrs: { level }, content: [text(t)] });
const para = (...parts) => ({ type: "paragraph", content: parts.map((p) => (typeof p === "string" ? text(p) : p)) });
const bullets = (items) => ({
  type: "bulletList",
  content: items.map((i) => ({ type: "listItem", content: [para(i)] })),
});
const doc = (...content) => ({ type: "doc", content });

const WEBVIEWS = [
  {
    name: "http-testing-guide", col: "http-echo-tests", tags: ["guide"],
    annotation: "How to read the echo endpoints.",
    page: doc(
      heading(1, "HTTP testing guide"),
      para("These endpoints echo what they receive, so you can check ", text("methods", [{ type: "bold" }]), ", bodies and status codes."),
      heading(2, "What is in here"),
      bullets(["GET, POST, PUT, PATCH and DELETE against httpbin.org", "Fixed status codes: 201, 401 and 404", "A UUID endpoint whose answer changes on every call"]),
      para("Open any of them in Test View to see its request, response and headers."),
    ),
  },
  {
    name: "public-data-guide", col: "open-data", tags: ["guide", "public"],
    annotation: "Where the reference data comes from.",
    page: doc(
      heading(1, "Public data sources"),
      para("Read-only APIs that need no key."),
      bullets(["Open-Meteo: current weather", "REST Countries: country details", "Open Library: book search", "Agify: age from a first name", "DummyJSON: sample store products"]),
    ),
  },
  {
    name: "fun-apis-page", col: "fun-apis", tags: ["fun"],
    annotation: "A lighter page.",
    page: doc(
      heading(1, "Fun APIs"),
      para("Pokemon, cats, dogs and jokes. Some answers change on every call, so run them twice."),
      heading(2, "Try this"),
      bullets(["Fetch a cat fact, then a random dog", "Compare two runs of the joke endpoint"]),
    ),
  },
];

const STATUS_TEXT = {
  200: "OK", 201: "Created", 204: "No Content", 400: "Bad Request", 401: "Unauthorized",
  403: "Forbidden", 404: "Not Found", 429: "Too Many Requests", 500: "Server Error",
};

// ── steps ───────────────────────────────────────────────────────────

async function waitForServer() {
  for (let i = 0; i < 90; i++) {
    try {
      const r = await fetch(BASE + "/home");
      if (r.ok) return;
    } catch {}
    await sleep(1000);
  }
  throw new Error(`backend at ${BASE} never came up`);
}

async function hasEndpoints() {
  const snap = await wsOnce("/test-view/endpoints/load", undefined, "snapshot");
  return (snap?.endpoints?.length || 0) > 0;
}

// The EID of an endpoint that already exists for this exact URL + method.
async function lookupEid(url, method) {
  const q = `endpoint_str=${encodeURIComponent(url)}&method=${encodeURIComponent(method)}`;
  const r = await get(`/endpoints/lookup?${q}`);
  return r.ok ? r.json?.endpoint?.endpoint_id ?? null : null;
}

async function waitJob(id, timeoutMs = 180000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    const r = await get(`/jobs/${id}`);
    if (r.ok && r.json.status !== "running") return r.json;
    await sleep(300);
  }
  throw new Error(`job ${id} did not finish in time`);
}

// Brings the recorded bundle in through the product's own import. Returns
// the new EIDs keyed "METHOD url", or throws.
async function importBundle(manifest) {
  const dest = join(STORAGE, "storage", "imports", "uncompressed", "repo", HELPER);
  mkdirSync(dirname(dest), { recursive: true });
  rmSync(dest, { recursive: true, force: true });
  cpSync(BUNDLE, dest, { recursive: true });
  try {
    // A helper view left by an interrupted run would make the name taken.
    if ((await get(`/repoview/${HELPER}`)).ok) await post(`/repoview/${HELPER}/delete`);

    const started = await post("/repoview/import", { folder: HELPER, name: HELPER });
    if (started.status !== 202) {
      throw new Error(`import was not accepted (${started.status}): ${JSON.stringify(started.json)}`);
    }
    const job = await waitJob(started.json.job_id);
    if (job.status !== "done") throw new Error(`import ${job.status}: ${job.error || JSON.stringify(job.result)}`);
    const result = job.result || {};
    for (const s of result.skipped || []) console.log(`[seed]   skipped ${s.eid}: ${s.reason}`);

    const byOld = new Map(manifest.endpoints.map((e) => [e.eid, e]));
    const eids = new Map();
    for (const m of result.mapping || []) {
      const e = byOld.get(m.old_eid);
      if (e) eids.set(`${(e.method || "GET").toUpperCase()} ${e.endpoint_str}`, m.new_eid);
    }
    return eids;
  } finally {
    // The helper view only existed to carry the import; the endpoints stay.
    await post(`/repoview/${HELPER}/delete`);
    rmSync(dest, { recursive: true, force: true });
  }
}

async function ensureCollection(c, eidsByCol) {
  const exists = (await get(`/collections/${encodeURIComponent(c.name)}`)).ok;
  if (!exists) await post("/collections/create", { name: c.name, annotation: c.annotation });
  if (!exists) {
    for (const t of c.tags || []) {
      await post(`/collections/${encodeURIComponent(c.name)}/membership-tags/add`, { tag: t });
    }
  }
  const members = (await get(`/collections/${encodeURIComponent(c.name)}/endpoints`)).json;
  const have = new Set((members?.endpoints || []).map((m) => m.endpoint_id));
  let added = 0;
  for (const eid of eidsByCol[c.name] || []) {
    if (have.has(eid)) continue;
    await post("/test-view/save/bookmark", { collection: c.name, endpoint_id: eid, notes: "" });
    const saved = await post(`/bookmarks/${encodeURIComponent(c.name)}/${eid}/save`);
    if (saved.ok) added++;
  }
  return { created: !exists, added };
}

async function ensureRepoview(v) {
  if ((await get(`/repoview/${encodeURIComponent(v.name)}`)).ok) return false;
  const made = await post("/repoview/create", { name: v.name, annotation: v.annotation, source_collection: v.col });
  if (!made.ok) {
    console.log(`[seed]   repoview '${v.name}' failed: ${made.json?.error}`);
    return false;
  }
  for (const t of v.tags) await post(`/repoview/${encodeURIComponent(v.name)}/membership-tags/add`, { tag: t });
  await post(`/repoview/${encodeURIComponent(v.name)}/tables/generate`, { approach: v.approach });
  return true;
}

async function ensureWebview(w) {
  if ((await get(`/webview/${encodeURIComponent(w.name)}`)).ok) return false;
  const made = await post("/webview/create", { name: w.name, annotation: w.annotation, source_collection: w.col });
  if (!made.ok) {
    console.log(`[seed]   webview '${w.name}' failed: ${made.json?.error}`);
    return false;
  }
  for (const t of w.tags) await post(`/webview/${encodeURIComponent(w.name)}/membership-tags/add`, { tag: t });
  await post(`/webview/${encodeURIComponent(w.name)}/front-page`, { front_page: w.page });
  return true;
}

async function main() {
  console.log(`[seed] target ${BASE}`);
  if (!existsSync(join(BUNDLE, "repoview-manifest.json"))) {
    throw new Error(`demo bundle not found at ${BUNDLE}`);
  }
  const manifest = JSON.parse(readFileSync(join(BUNDLE, "repoview-manifest.json"), "utf8"));

  await waitForServer();

  if ((await hasEndpoints()) && !FORCE) {
    console.log("[seed] backend already has endpoints — nothing to do (SEED_FORCE=1 to seed anyway).");
    return;
  }

  // 1. endpoints: reuse any that already exist; import the bundle only when
  //    none of them do (importing again would duplicate the ones that do).
  const key = (e) => `${(e.method || "GET").toUpperCase()} ${e.endpoint_str}`;
  const eidFor = new Map();
  for (const e of manifest.endpoints) {
    const eid = await lookupEid(e.endpoint_str, e.method || "GET");
    if (eid) eidFor.set(key(e), eid);
  }
  let imported = false;
  if (eidFor.size === 0) {
    console.log(`[seed] importing the recorded bundle (${manifest.endpoints.length} endpoints) …`);
    for (const [k, v] of await importBundle(manifest)) eidFor.set(k, v);
    imported = true;
  } else {
    console.log(`[seed] ${eidFor.size}/${manifest.endpoints.length} bundle endpoints already exist — reusing them, not importing again.`);
  }
  if (eidFor.size === 0) throw new Error("no endpoints were imported");

  // 2. one history row per recorded run (an import brings the runs, not the
  //    history list), only for endpoints brought in by this run.
  if (imported) {
    console.log("[seed] history …");
    for (const e of manifest.endpoints) {
      const eid = eidFor.get(key(e));
      if (!eid) continue;
      for (const q of e.qps || []) {
        const code = q.status_code;
        const label = code ? `${code} ${STATUS_TEXT[code] || ""}`.trim() : "no response";
        await post("/test-view/save/history", {
          endpoint_id: eid,
          action: "test",
          details: `${label} in ${q.response_time_ms ?? 0}ms`,
        });
      }
    }
  }

  // 3. collections
  console.log("[seed] collections …");
  const eidsByCol = {};
  for (const [col, method, url] of LAYOUT) {
    const eid = eidFor.get(`${method} ${url}`);
    if (eid) (eidsByCol[col] ||= []).push(eid);
  }
  for (const c of COLLECTIONS) {
    const r = await ensureCollection(c, eidsByCol);
    console.log(`[seed]   ${c.name}: ${r.created ? "created" : "exists"}, ${r.added} added`);
  }
  // Leave fun-apis with two bookmarks that are NOT saved into it, so
  // Bookmark View shows both states.
  if (imported) {
    const funEids = new Set(eidsByCol["fun-apis"] || []);
    const spare = [...eidFor.values()].filter((id) => !funEids.has(id)).slice(0, 2);
    for (const id of spare) await post("/test-view/save/bookmark", { collection: "fun-apis", endpoint_id: id, notes: "" });
  }

  // 4. RepoViews, then one combined by tag
  console.log("[seed] repoviews …");
  let views = 0;
  for (const v of REPOVIEWS) if (await ensureRepoview(v)) views++;
  if (!(await get(`/repoview/${encodeURIComponent(COMBINED.name)}`)).ok) {
    const c = await post("/repoview/combine", { name: COMBINED.name, tags: COMBINED.tags, annotation: COMBINED.annotation });
    if (c.ok) {
      views++;
      await post(`/repoview/${encodeURIComponent(COMBINED.name)}/tables/generate`, { approach: "sorted_tags" });
    } else {
      console.log(`[seed]   combine '${COMBINED.name}' failed: ${c.json?.error}`);
    }
  }

  // 5. WebViews
  console.log("[seed] webviews …");
  let pages = 0;
  for (const w of WEBVIEWS) if (await ensureWebview(w)) pages++;

  console.log(
    `[seed] done — ${eidFor.size} endpoints, ${COLLECTIONS.length} collections, ${views} repoviews, ${pages} webviews, tags + history populated.`,
  );
}

main()
  .then(() => process.exit(0))
  .catch((e) => {
    console.error("[seed] FAILED:", e.message || e);
    process.exit(1);
  });
