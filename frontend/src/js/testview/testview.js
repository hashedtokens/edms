// ============================================================
// EDMS TEST VIEW
// ============================================================

let endpoints = [];
let bookmarks = [];
let historyRecords = [];
let filteredTestEndpoints = [];

let selectedTestEndpoint = null;
let selectedTestQP = null;
let historyPage = 1;
const HISTORY_PAGE_SIZE = 5;
let lastFilterSignature = "";
let selectedQPIds = new Set();
let selectedHistoryIds = new Set();
let activeSidebarTab = "endpoints";
let activeTestMethod = "ALL";
let activeTimeFilter = "all";

let activeRequestTab = "body";
let activeResponseTab = "body";

let latestResponseMeta = null;

let runTimer = null;

let testWS = null;
let endpointWS = null;
let bookmarkWS = null;
let historyWS = null;

let testRunStartedAt = null;
let activeRequestNumber = null;

/*
 * Collection context — set when testview is opened via
 * ?collection=<name> from the Collection View right-click menu.
 * activeCollectionEndpointIds is the Set of endpoint IDs that
 * belong to the collection; null means "show all".
 */
let activeCollectionFilter = null;
let activeCollectionEndpointIds = null;

const LOCAL_QP_KEY = "edmsTestViewQPs";

const API_BASE = "http://localhost:3000";

// ============================================================
// DOM
// ============================================================

const testEndpointList =
    document.getElementById("testEndpointList");

const testQPPanel =
    document.getElementById("qpPanel");

const qpMenuButton =
    document.getElementById("qpMenuButton");

const qpMenu =
    document.getElementById("qpMenu");

const testSearchInput =
    document.getElementById("testSearchInput");

const urlFilter =
    document.getElementById("urlFilter");

const timeFilter =
    document.getElementById("timeFilter");

const testMethod =
    document.getElementById("Method");

const baseUrl =
    document.getElementById("URL-prefix");

const endpointPath =
    document.getElementById("Endpoint-path");

const runButton =
    document.getElementById("runRequest");

const saveButton =
    document.getElementById("saveRequest");

const annotationInput =
    document.getElementById("annotations");

const tagInput =
    document.getElementById("tagInput");

const addTagButton =
    document.getElementById("addTagButton");

const endpointTags =
    document.getElementById("endpointTags");

const requestBox =
    document.getElementById("requestBox");

const responseBox =
    document.getElementById("responseBox");

const requestContent =
    document.getElementById("requestContent");

const responseContent =
    document.getElementById("responseContent");

const testSidebar =
    document.getElementById("testSidebar");

const sidebarCollapseToggle =
    document.getElementById("sidebarCollapseToggle");

const addressModeToggle =
    document.getElementById("addressModeToggle");

const addressSplit =
    document.getElementById("addressSplit");

const urlFullInput =
    document.getElementById("URL-full");

// ============================================================
// INIT
// ============================================================

document.addEventListener(
    "DOMContentLoaded",
    initTestView
);

async function initTestView() {

    try {

        await loadTestData();

    } catch (error) {

        console.error(
            "Failed to initialize Test View:",
            error
        );

    }

    setupTestSearch();
    setupMethodFilters();
    setupTimeFilter();
    setupURLFilter();
    setupRunner();
    setupTags();
    setupTabs();
    setupPanelControls();
    setupSidebarTabs();
    setupSidebarCollapse();
    setupAddressMode();
    setupQPMenu();
    setupHistoryContextMenu();
    setupHistoryMenu();
    updateMethodButtons();
    updateSidebarTabButtons();
    applyTestFilters();

    await initCollectionContext();

}

// ============================================================
// COLLECTION CONTEXT (launched via ?collection= URL param)
// ============================================================

async function initCollectionContext() {

    const params =
        new URLSearchParams(
            window.location.search
        );

    const collectionName =
        params.get('collection');

    if (!collectionName) {
        return;
    }

    activeCollectionFilter = collectionName;

    /*
     * Fetch the endpoint IDs that belong to this collection
     * so we can pre-filter the sidebar list.
     */
    try {

        const api = window.EdmsAPI;

        if (
            api &&
            typeof api.listCollectionEndpoints === 'function'
        ) {

            const response =
                await api.listCollectionEndpoints(
                    collectionName
                );

            const data =
                response?.data ?? response;

            const items =
                Array.isArray(data)
                    ? data
                    : Array.isArray(data?.endpoints)
                        ? data.endpoints
                        : Array.isArray(data?.items)
                            ? data.items
                            : [];

            activeCollectionEndpointIds = new Set(
                items.map(
                    item =>
                        String(
                            item?.endpoint_id ??
                            item?.id ??
                            item ??
                            ''
                        )
                ).filter(Boolean)
            );

        }

    } catch (error) {

        console.error(
            'Failed to load collection membership for Test View filter:',
            error
        );

        activeCollectionEndpointIds = null;

    }

    /*
     * Show the banner and populate it.
     */
    const banner =
        document.getElementById(
            'collectionContextBanner'
        );

    const nameEl =
        document.getElementById(
            'collectionContextName'
        );

    const countEl =
        document.getElementById(
            'collectionContextCount'
        );

    const clearBtn =
        document.getElementById(
            'clearCollectionFilter'
        );

    if (nameEl) {
        nameEl.textContent =
            collectionName;
    }

    if (countEl) {
        const count =
            activeCollectionEndpointIds
                ? activeCollectionEndpointIds.size
                : '?';

        countEl.textContent =
            `${count} endpoint${count === 1 ? '' : 's'}`;
    }

    if (banner) {
        banner.classList.remove('hidden');
        banner.classList.add('flex');
    }

    if (clearBtn) {
        clearBtn.addEventListener(
            'click',
            () => {

                activeCollectionFilter = null;
                activeCollectionEndpointIds = null;

                if (banner) {
                    banner.classList.add('hidden');
                    banner.classList.remove('flex');
                }

                applyTestFilters();

            }
        );
    }

    /*
     * Re-apply filters now that the collection
     * membership set is populated.
     */
    applyTestFilters();

}

// ============================================================
// LOAD DATA
// ============================================================

async function loadTestData() {

    await loadEndpointsFromBackend();

    await loadBookmarksFromBackend();

    await loadHistoryFromBackend();

    restoreLocalQPs();

    /*
     * Endpoint tags are stored separately from the
     * endpoint snapshot, so load them from the tags API.
     */
    await loadEndpointTagsFromBackend();

}

// ============================================================
// TAG API
// ============================================================

async function fetchEndpointTags(endpointId) {

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        return [];

    }

    const response =
        await fetch(
            `${API_BASE}/tags/${encodeURIComponent(endpointId)}`
        );

    if (!response.ok) {

        throw new Error(
            `Failed to load endpoint tags: ${response.status}`
        );

    }

    const data =
        await response.json();

    return Array.isArray(data?.tags)
        ? data.tags
        : [];

}

// ============================================================
// QP API
// ============================================================

async function fetchEndpointQPs(endpointId) {

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        return [];

    }

    const response =
        await fetch(
            `${API_BASE}/test-view/${encodeURIComponent(endpointId)}/qps`
        );

    if (!response.ok) {

        throw new Error(
            `Failed to load endpoint QPs: ${response.status}`
        );

    }

    const data =
        await response.json();

    if (!data.ok || !Array.isArray(data.qps)) {
        return [];
    }

    return data.qps.map(qp => ({
        id: qp.request_number,
        name: String(qp.request_number),
        method: qp.method,
        timestamp: qp.timestamp,
        status_code: qp.status_code,
        response_time_ms: qp.response_time_ms,
        request: {},
        response: {}
    }));

}

async function addEndpointTag(endpointId, tag) {

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        throw new Error(
            "Endpoint ID is required to add a tag."
        );

    }

    const response =
        await fetch(
            `${API_BASE}/tags/${encodeURIComponent(endpointId)}/add`,
            {
                method: "POST",

                headers: {
                    "Content-Type":
                        "application/json"
                },

                body:
                    JSON.stringify({
                        tag
                    })
            }
        );

    const data =
        await parseTagResponse(
            response
        );

    if (!response.ok) {

        throw new Error(
            data?.message ||
            data?.error ||
            `Failed to add tag: ${response.status}`
        );

    }

    return data;

}

async function removeEndpointTag(endpointId, tag) {

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        throw new Error(
            "Endpoint ID is required to remove a tag."
        );

    }

    const response =
        await fetch(
            `${API_BASE}/tags/${encodeURIComponent(endpointId)}/remove`,
            {
                method: "POST",

                headers: {
                    "Content-Type":
                        "application/json"
                },

                body:
                    JSON.stringify({
                        tag
                    })
            }
        );

    const data =
        await parseTagResponse(
            response
        );

    if (!response.ok) {

        throw new Error(
            data?.message ||
            data?.error ||
            `Failed to remove tag: ${response.status}`
        );

    }

    return data;

}

async function loadPopularEndpointTags() {

    const response =
        await fetch(
            `${API_BASE}/tags/popular`
        );

    if (!response.ok) {

        throw new Error(
            `Failed to load popular tags: ${response.status}`
        );

    }

    const data =
        await response.json();

    return Array.isArray(data)
        ? data
        : [];

}

async function parseTagResponse(response) {

    const text =
        await response.text();

    if (!text) {
        return {};
    }

    try {

        return JSON.parse(text);

    } catch {

        return {
            message: text
        };

    }

}

// ============================================================
// LOAD ENDPOINT TAGS
// ============================================================

async function loadEndpointTagsFromBackend() {

    if (
        !Array.isArray(endpoints) ||
        endpoints.length === 0
    ) {

        return;

    }

    /*
     * Load each endpoint's tags from the dedicated
     * endpoint-tags API.
     *
     * A failure for one endpoint should not prevent
     * the rest of Test View from loading.
     */

    await Promise.all(
        endpoints.map(
            async endpoint => {

                if (
                    endpoint?.id === undefined ||
                    endpoint?.id === null
                ) {

                    return;

                }

                try {
                    await loadTagsAndQPsInto(endpoint);
                } catch (error) {

                    console.warn(
                        `Could not load tags or QPs for endpoint ${endpoint.id}:`,
                        error
                    );

                    if (
                        !Array.isArray(
                            endpoint.tags
                        )
                    ) {

                        endpoint.tags = [];

                    }

                    if (
                        !Array.isArray(
                            endpoint.qps
                        )
                    ) {

                        endpoint.qps = [];

                    }

                }

            }
        )
    );

    if (selectedTestEndpoint) {

        const refreshedEndpoint =
            findEndpoint(
                selectedTestEndpoint.id
            );

        if (refreshedEndpoint) {

            selectedTestEndpoint =
                refreshedEndpoint;

            renderSelectedEndpointTags();

        }

    }

}

// ============================================================
// BACKEND ENDPOINT SNAPSHOT
// ============================================================

function loadEndpointsFromBackend() {

    return new Promise(
        (resolve, reject) => {

            const ws =
                window.EdmsAPI
                    .connectEndpointLoader();

            endpointWS = ws;

            let finished = false;

            ws.addEventListener(
                "open",
                () => {

                    console.log(
                        "Endpoint WebSocket connected."
                    );

                }
            );

            ws.addEventListener(
                "message",
                event => {

                    try {

                        const message =
                            JSON.parse(
                                event.data
                            );

                        console.log(
                            "Endpoint snapshot:",
                            message
                        );

                        if (
                            message.type ===
                            "snapshot" &&
                            Array.isArray(
                                message.endpoints
                            )
                        ) {

                            endpoints =
                                message.endpoints.map(
                                    normalizeBackendEndpoint
                                );

                            const wasFinished = finished;
                            finished = true;

                            console.log(
                                `Loaded ${endpoints.length} endpoints from backend.`
                            );

                            if (!wasFinished) {
                                resolve();
                            } else {
                                applyTestFilters();
                            }

                        } else if (message.type === "event" && message.event) {
                            const evtType = message.event.type;
                            if (
                                evtType === "CrudOperationsUpdated" ||
                                evtType === "EndpointAnnotationUpdated" ||
                                evtType === "CollectionLoaded" ||
                                evtType === "ViewRefresh"
                            ) {
                                try { ws.close(); } catch { }
                                setTimeout(() => loadEndpointsFromBackend().then(() => applyTestFilters()), 0);
                                return;
                            }
                        }

                    } catch (error) {

                        console.error(
                            "Endpoint WS message error:",
                            error
                        );

                    }

                }
            );

            ws.addEventListener(
                "error",
                error => {

                    console.error(
                        "Endpoint WebSocket error:",
                        error
                    );

                    if (!finished) {

                        reject(
                            new Error(
                                "Could not connect to backend endpoint WebSocket."
                            )
                        );

                    }

                }
            );

            ws.addEventListener(
                "close",
                () => {

                    endpointWS = null;

                }
            );

        }
    );

}

// ============================================================
// NORMALIZE ENDPOINT
// ============================================================
function normalizeBackendEndpoint(endpoint) {

    const id = endpoint.id ?? endpoint.endpoint_id;

    const fullURL = String(
        endpoint.endpoint_str ??
        endpoint.endpoint ??
        endpoint.url ??
        ""
    ).trim().replace(/^["']+|["']+$/g, "");   // NEW: strip wrapping quotes
    const split = splitAddress(fullURL);

    const base =
        endpoint.baseUrl ||
        endpoint.base_url ||
        split.prefix;

    // Never let the path contain the base
    let path = fullURL;
    if (base && fullURL.startsWith(base)) {
        path = fullURL.slice(base.length);
    } else {
        path = split.path || fullURL;
    }

    return {
        ...endpoint,
        id,
        method: String(endpoint.method || "GET").toUpperCase(),
        endpoint: path,
        baseUrl: base,
        endpoint_str: endpoint.endpoint_str || fullURL,
        qps: Array.isArray(endpoint.qps) ? endpoint.qps : [],
        tags: Array.isArray(endpoint.tags) ? endpoint.tags : []
    };
}
// ============================================================
// BACKEND BOOKMARK SNAPSHOT
// ============================================================

function loadBookmarksFromBackend() {

    // If there's no collection context, don't try to load bookmarks.
    if (!activeCollectionFilter) {
        return Promise.resolve();
    }

    const collectionForThisLoad = activeCollectionFilter;

    return new Promise(
        (resolve, reject) => {

            const ws =
                window.EdmsAPI
                    .connectBookmarkLoader(collectionForThisLoad);

            bookmarkWS = ws;

            let finished = false;

            ws.addEventListener(
                "open",
                () => {

                    console.log(
                        "Bookmark WebSocket connected."
                    );

                }
            );

            ws.addEventListener(
                "message",
                event => {

                    try {

                        const message =
                            JSON.parse(
                                event.data
                            );

                        console.log(
                            "Bookmark snapshot:",
                            message
                        );

                        if (
                            message.type ===
                            "snapshot" &&
                            Array.isArray(
                                message.bookmarks
                            )
                        ) {

                            bookmarks =
                                message.bookmarks;

                            const wasFinished = finished;
                            finished = true;

                            console.log(
                                `Loaded ${bookmarks.length} bookmarks for collection '${collectionForThisLoad}' from backend.`
                            );

                            if (!wasFinished) {
                                resolve();
                            } else {
                                applyTestFilters();
                            }

                        } else if (message.type === "event" && message.event) {
                            const evtType = message.event.type;
                            const evtCollection = message.event.collection;

                            if (evtType === "ViewRefresh") {
                                // Always reload on a view refresh.
                                try { ws.close(); } catch { }
                                setTimeout(() => loadBookmarksFromBackend().then(() => applyTestFilters()), 0);
                                return;
                            }

                            if (
                                evtType === "BookmarksUpdated" &&
                                evtCollection === collectionForThisLoad
                            ) {
                                // Only reload if the update is for *our* collection.
                                try { ws.close(); } catch { }
                                setTimeout(() => loadBookmarksFromBackend().then(() => applyTestFilters()), 0);
                                return;
                            }
                        }

                    } catch (error) {

                        console.error(
                            "Bookmark WS message error:",
                            error
                        );

                    }

                }
            );

            ws.addEventListener(
                "error",
                error => {

                    console.error(
                        "Bookmark WebSocket error:",
                        error
                    );

                    if (!finished) {
                        reject(error);
                    }

                }
            );

            ws.addEventListener(
                "close",
                () => {

                    bookmarkWS = null;

                }
            );

        }
    );

}

// ============================================================
// BACKEND HISTORY SNAPSHOT
// ============================================================

function loadHistoryFromBackend() {

    return new Promise(
        (resolve, reject) => {

            const ws =
                window.EdmsAPI
                    .connectHistoryLoader();

            historyWS = ws;

            let finished = false;

            ws.addEventListener(
                "open",
                () => {

                    console.log(
                        "History WebSocket connected."
                    );

                }
            );

            ws.addEventListener(
                "message",
                event => {

                    try {

                        const message =
                            JSON.parse(
                                event.data
                            );

                        console.log(
                            "History snapshot:",
                            message
                        );

                        if (
                            message.type ===
                            "snapshot" &&
                            Array.isArray(
                                message.history
                            )
                        ) {

                            historyRecords =
                                message.history.map(
                                    normalizeBackendHistory
                                );

                            historyRecords =
                                historyRecords.map(
                                    history => ({

                                        ...history,

                                        saved:
                                            isEndpointBookmarked(
                                                history.endpointId
                                            )

                                    })
                                );

                            const wasFinished = finished;
                            finished = true;

                            console.log(
                                `Loaded ${historyRecords.length} history records from backend.`
                            );

                            if (!wasFinished) {
                                resolve();
                            } else {
                                applyTestFilters();
                            }

                        } else if (message.type === "event" && message.event) {
                            const evtType = message.event.type;
                            if (
                                evtType === "HistoryUpdated" ||
                                evtType === "ViewRefresh"
                            ) {
                                try { ws.close(); } catch { }
                                setTimeout(() => loadHistoryFromBackend().then(() => applyTestFilters()), 0);
                                return;
                            }
                        }

                    } catch (error) {

                        console.error(
                            "History WS message error:",
                            error
                        );

                    }

                }
            );

            ws.addEventListener(
                "error",
                error => {

                    console.error(
                        "History WebSocket error:",
                        error
                    );

                    if (!finished) {

                        reject(
                            new Error(
                                "Could not connect to backend history WebSocket."
                            )
                        );

                    }

                }
            );

            ws.addEventListener(
                "close",
                () => {

                    historyWS = null;

                }
            );

        }
    );

}

// ============================================================
// NORMALIZE HISTORY
// ============================================================

function normalizeBackendHistory(history) {

    let details =
        history.details;

    if (
        typeof details ===
        "string"
    ) {

        try {

            details =
                JSON.parse(
                    details
                );

        } catch {

            details = {
                raw:
                    details
            };

        }

    }

    details =
        details ||
        {};

    const endpointId =
        history.endpoint_id ??
        history.endpointId;

    const status =
        details.status ??
        details.status_code ??
        history.status;

    const elapsedMs =
        details.elapsed ??
        details.elapsed_ms ??
        details.response_time_ms ??
        history.elapsedMs;

    const requestNumber =
        details.requestNumber ??
        details.request_number ??
        history.requestNumber ??
        history.request_number;

    return {

        ...history,

        id:
            history.id ??
            history.history_id ??
            `history-${history.timestamp}`,

        endpointId,

        action:
            history.action,

        details,

        requestNumber,

        status,

        elapsedMs,

        testedAt:
            history.timestamp ??
            history.testedAt,

        saved:
            false

    };

}

// ============================================================
// LOCAL QP STORAGE
// ============================================================

function loadLocalQPs() {

    try {

        const saved =
            JSON.parse(
                localStorage.getItem(
                    LOCAL_QP_KEY
                ) ||
                "{}"
            );

        return (
            saved &&
            typeof saved === "object"
        )
            ? saved
            : {};

    } catch (error) {

        console.error(
            "Failed to load local QPs:",
            error
        );

        return {};

    }

}

function saveLocalQPs() {

    try {

        const qpStore = {};

        endpoints.forEach(
            endpoint => {

                if (
                    !endpoint ||
                    endpoint.id === undefined ||
                    !Array.isArray(endpoint.qps)
                ) {
                    return;
                }

                qpStore[
                    String(endpoint.id)
                ] =
                    endpoint.qps;

            }
        );

        localStorage.setItem(
            LOCAL_QP_KEY,
            JSON.stringify(qpStore)
        );

    } catch (error) {

        console.error(
            "Failed to save local QPs:",
            error
        );

    }

}

function restoreLocalQPs() {

    const qpStore =
        loadLocalQPs();

    endpoints.forEach(
        endpoint => {

            if (
                !endpoint ||
                endpoint.id === undefined
            ) {
                return;
            }

            const savedQPs =
                qpStore[
                String(endpoint.id)
                ];

            if (
                Array.isArray(savedQPs)
            ) {

                endpoint.qps =
                    savedQPs;

            }

        }
    );

}

// ============================================================
// SIDEBAR ITEMS
// ============================================================

function getActiveSidebarItems() {

    if (
        activeSidebarTab ===
        "history"
    ) {

        return historyRecords.map(
            history => {

                const endpoint =
                    findEndpoint(
                        history.endpointId
                    );

                return {

                    type:
                        "history",

                    id:
                        history.id,

                    endpointId:
                        history.endpointId,

                    qpId:
                        history.qpId,

                    method:
                        history.method ||
                        endpoint?.method ||
                        "",

                    endpoint:
                        history.endpoint ||
                        endpoint?.endpoint ||
                        "",

                    updated:
                        history.testedAt,

                    status:
                        history.status,

                    saved:
                        isEndpointBookmarked(
                            history.endpointId
                        ),

                    source:
                        history,

                    endpointRef:
                        endpoint

                };

            }
        );

    }

    if (
        activeSidebarTab ===
        "bookmarks"
    ) {

        return bookmarks
            .map(
                bookmark => {

                    const endpointId =
                        getBookmarkEndpointId(
                            bookmark
                        );

                    const endpoint =
                        findEndpoint(
                            endpointId
                        );

                    return {

                        type:
                            "bookmark",

                        id:
                            bookmark.id ??
                            bookmark.bookmark_id,

                        endpointId,

                        qpId:
                            bookmark.qpId ??
                            bookmark.qp_id,

                        method:
                            endpoint?.method ||
                            "",

                        endpoint:
                            endpoint?.endpoint ||
                            "",

                        updated:
                            bookmark.bookmarkedAt ??
                            bookmark.bookmarked_at,

                        saved:
                            true,

                        source:
                            bookmark,

                        endpointRef:
                            endpoint

                    };

                }
            )
            .filter(
                item =>
                    item.endpointRef
            );

    }

    return endpoints.map(
        endpoint => ({

            type:
                "endpoint",

            id:
                endpoint.id,

            endpointId:
                endpoint.id,

            method:
                endpoint.method,

            endpoint:
                endpoint.endpoint,

            updated:
                endpoint.updated ||
                endpoint.addedAt ||
                endpoint.added_at,

            saved:
                isEndpointBookmarked(
                    endpoint.id
                ),

            source:
                endpoint,

            endpointRef:
                endpoint

        })
    );

}

// ============================================================
// RENDER SIDEBAR
// ============================================================

function renderTestEndpoints() {

    if (!testEndpointList) return;

    testEndpointList.innerHTML = "";

    if (
        filteredTestEndpoints.length === 0
    ) {

        testEndpointList.innerHTML = `
            <div class="p-4 text-sm text-slate-500">
                No ${escapeHTML(activeSidebarTab)} found.
            </div>
        `;

        return;

    }

    // Go back to page 1 when the tab or a filter changes
    const signature = [
        activeSidebarTab,
        activeTestMethod,
        activeTimeFilter,
        testSearchInput?.value,
        urlFilter?.value
    ].join("|");

    if (signature !== lastFilterSignature) {
        historyPage = 1;
        lastFilterSignature = signature;
    }

    const paginate = true;
    let visibleItems = filteredTestEndpoints;
    let totalPages = 1;

    if (paginate) {
        totalPages = Math.max(
            1,
            Math.ceil(filteredTestEndpoints.length / HISTORY_PAGE_SIZE)
        );
        historyPage = Math.min(Math.max(historyPage, 1), totalPages);

        const start = (historyPage - 1) * HISTORY_PAGE_SIZE;
        visibleItems = filteredTestEndpoints.slice(
            start,
            start + HISTORY_PAGE_SIZE
        );
    }

    visibleItems.forEach(
        item => {

            testEndpointList.appendChild(
                createTestEndpointCard(item)
            );

        }
    );

    if (paginate && totalPages > 1) {
        testEndpointList.appendChild(
            createHistoryPager(totalPages)
        );
    }

    updateSelectedEndpointHighlight();

}
function createHistoryPager(totalPages) {

    const bar = document.createElement("div");

    bar.className =
        "sticky bottom-0 flex items-center justify-between gap-2 " +
        "border-t border-slate-800 bg-slate-950 px-3 py-2 " +
        "text-xs text-slate-400";

    bar.innerHTML = `
        <button type="button" data-page="prev"
            class="rounded-md border border-slate-700 px-3 py-1 hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed">
            Prev
        </button>
        <span>Page ${historyPage} of ${totalPages}</span>
        <button type="button" data-page="next"
            class="rounded-md border border-slate-700 px-3 py-1 hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed">
            Next
        </button>
    `;

    const prev = bar.querySelector('[data-page="prev"]');
    const next = bar.querySelector('[data-page="next"]');

    prev.disabled = historyPage <= 1;
    next.disabled = historyPage >= totalPages;

    prev.addEventListener("click", () => {
        historyPage--;
        renderTestEndpoints();
        testEndpointList.scrollTop = 0;
    });

    next.addEventListener("click", () => {
        historyPage++;
        renderTestEndpoints();
        testEndpointList.scrollTop = 0;
    });

    return bar;

}

// ============================================================
// ENDPOINT CARD
// ============================================================

function createTestEndpointCard(item) {

    const card =
        document.createElement("div");

    const endpoint =
        item.endpointRef ||
        item.source ||
        item;

    const isHistory =
        item.type === "history";

    card.className = `
        endpoint-card
        group
        rounded-lg
        border
        border-slate-800
        bg-slate-900
        hover:bg-slate-800/80
        hover:border-cyan-500/50
        transition-all
        duration-200
        cursor-pointer
        px-3
        py-3
    `;

    card.dataset.id =
        item.id ??
        endpoint.id;

    card.dataset.endpointId =
        item.endpointId ??
        endpoint.id;

    card.innerHTML = `

        <div class="flex items-center justify-between">

            <div class="flex items-center gap-2">

                ${isHistory
            ? `
                            <input
                                type="checkbox"
                                class="
                                    history-checkbox
                                    hidden
                                    group-hover:block
                                    h-3.5
                                    w-3.5
                                    accent-cyan-500
                                    cursor-pointer
                                "
                                data-history-id="${escapeHTML(
                String(
                    item.id ??
                    endpoint.id
                )
            )}"
                            />
                        `
            : ""
        }

                <span
                    class="px-2 py-1 rounded-md text-[11px]
                           font-semibold border
                           ${getMethodColor(
            item.method ||
            endpoint.method
        )}">
                    ${escapeHTML(
            item.method ||
            endpoint.method ||
            ""
        )}
                </span>

            </div>

            <div class="flex items-center gap-2">

                <span
                    class="text-[11px]
                           text-slate-500
                           group-hover:text-slate-300">
                    ${escapeHTML(
            formatEndpointDate(
                item.updated
            )
        )}
                </span>

                ${isHistory
            ? `
                            <button
    type="button"
    class="
        history-delete
        hidden
        group-hover:flex
        items-center
        justify-center
        w-7
        h-7
        rounded-md
        text-slate-400
        hover:text-red-400
        hover:bg-red-500/10
        transition
    "
    data-history-id="${escapeHTML(
                String(
                    item.id ??
                    endpoint.id
                )
            )}"
    title="Delete history"
>
    <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="w-4 h-4"
    >
        <path d="M3 6h18"></path>
        <path d="M8 6V4h8v2"></path>
        <path d="M19 6l-1 14H6L5 6"></path>
        <path d="M10 11v5"></path>
        <path d="M14 11v5"></path>
    </svg>
</button>
                        `
            : ""
        }

            </div>

        </div>

        <p
            class="mt-2 text-sm font-medium
                   text-slate-200 truncate
                   group-hover:text-white">
            ${escapeHTML(
            item.endpoint ||
            endpoint.endpoint ||
            ""
        )}
        </p>

        ${isHistory
            ? ""
            : `
                    <div
                        class="mt-2 flex items-center
                               justify-between text-[11px]
                               text-slate-500">

                        <span>
                            ${escapeHTML(
                getItemBadge(item)
            )}
                        </span>

                        <span>
                            ${getQPCountLabel(endpoint)}
                        </span>

                    </div>
                `
        }
    `;
    if (isHistory) {

        const checkbox =
            card.querySelector(
                ".history-checkbox"
            );

        if (checkbox) {

            checkbox.addEventListener(
                "click",
                event => {

                    event.stopPropagation();

                }
            );

            checkbox.addEventListener(
                "change",
                () => {

                    const historyId =
                        String(
                            item.id ??
                            endpoint.id
                        );

                    if (
                        checkbox.checked
                    ) {

                        selectedHistoryIds.add(
                            historyId
                        );

                        card.classList.add(
                            "ring-2",
                            "ring-cyan-400"
                        );

                    } else {

                        selectedHistoryIds.delete(
                            historyId
                        );

                        card.classList.remove(
                            "ring-2",
                            "ring-cyan-400"
                        );

                    }

                }
            );

        }

    }
    if (isHistory) {

        const deleteButton =
            card.querySelector(
                ".history-delete"
            );

        if (deleteButton) {

            deleteButton.addEventListener(
                "click",
                async event => {

                    event.stopPropagation();

                    const historyId =
                        String(
                            item.id ??
                            endpoint.id
                        );

                    try {

                        await deleteHistoryItem(
                            historyId
                        );

                        await loadHistoryFromBackend();

                        applyTestFilters();

                    } catch (error) {

                        console.error(
                            "Failed to delete history:",
                            error
                        );

                        window.alert(
                            error?.message ||
                            "Could not delete history."
                        );

                    }

                }
            );

        }

    }
    card.addEventListener(
        "click",
        () => {

            selectSidebarItem(
                item,
                card
            );

        }
    );

    if (isHistory) {

        card.addEventListener(
            "contextmenu",
            event => {

                showHistoryContextMenu(
                    event,
                    item.source
                );

            }
        );

    }

    return card;

}
async function deleteHistoryItem(historyId) {

    if (!historyId) {
        return;
    }

    const response =
        await fetch(
            `${API_BASE}/test-view/history/${encodeURIComponent(historyId)}/delete`,
            { method: "POST" }
        );

    if (!response.ok) {

        throw new Error(
            `History delete failed: ${response.status}`
        );

    }

    selectedHistoryIds.delete(
        String(historyId)
    );

}
// ============================================================
// HISTORY MENU ACTION
// ============================================================

function handleHistoryMenuAction(action) {
    switch (action) {
        case "select-all":
            doSelectAllHistory();
            break;

        case "invert-selection":
            doInvertSelectionHistory();
            break;

        case "clear-selection":
            doClearSelectionHistory();
            break;

        case "delete-selected":
            doDeleteSelectedHistory();
            break;
    }
}
function doSelectAllHistory() {

    selectedHistoryIds = new Set(
        historyRecords.map(
            history => String(history.id)
        )
    );

    document
        .querySelectorAll(".history-checkbox")
        .forEach(checkbox => {

            checkbox.checked = true;

            const card =
                checkbox.closest(".endpoint-card");

            if (card) {
                card.classList.add(
                    "ring-2",
                    "ring-cyan-400"
                );
            }

        });

}
function doClearSelectionHistory() {

    selectedHistoryIds = new Set();

    document
        .querySelectorAll(".history-checkbox")
        .forEach(checkbox => {

            checkbox.checked = false;

            const card =
                checkbox.closest(".endpoint-card");

            if (card) {
                card.classList.remove(
                    "ring-2",
                    "ring-cyan-400"
                );
            }

        });

}
// ============================================================
// DELETE SELECTED HISTORY
// ============================================================

async function doDeleteSelectedHistory() {

    if (
        selectedHistoryIds.size === 0
    ) {

        console.warn(
            "No history items selected."
        );

        return;
    }

    const deletePromises = [];

    for (const historyId of selectedHistoryIds) {

        deletePromises.push(
            deleteHistoryItem(
                historyId
            ).catch(error => {

                console.error(
                    `Error deleting history ${historyId}:`,
                    error
                );

            })
        );

    }

    if (
        deletePromises.length > 0
    ) {

        await Promise.all(
            deletePromises
        );

    }

    selectedHistoryIds =
        new Set();

    await loadHistoryFromBackend();

    applyTestFilters();

}
// ============================================================
// HISTORY CONTEXT MENU
// ============================================================

function setupHistoryContextMenu() {

    if (
        document.getElementById(
            "testHistoryContextMenu"
        )
    ) {
        return;
    }

    const menu =
        document.createElement(
            "div"
        );

    menu.id =
        "testHistoryContextMenu";

    menu.className = `
        fixed
        z-[9999]
        hidden
        min-w-[190px]
        rounded-lg
        border
        border-slate-700
        bg-slate-900
        shadow-xl
        overflow-hidden
    `;

    menu.innerHTML = `

        <button
            type="button"
            data-history-action="bookmark"
            class="
                w-full
                px-4
                py-2.5
                text-left
                text-sm
                text-slate-200
                hover:bg-slate-800
            "
        >
            Add to Bookmark
        </button>

        <button
            type="button"
            data-history-action="clear"
            class="
                w-full
                px-4
                py-2.5
                text-left
                text-sm
                text-red-400
                hover:bg-slate-800
            "
        >
            Clear All
        </button>
    `;

    document.body.appendChild(
        menu
    );

    menu.addEventListener(
        "click",
        async event => {

            const button =
                event.target.closest(
                    "[data-history-action]"
                );

            if (!button) return;

            const action =
                button.dataset.historyAction;

            const history =
                menu._historyItem;

            hideHistoryContextMenu();

            if (!history) {
                return;
            }

            if (
                action ===
                "bookmark"
            ) {

                await addHistoryToBookmark(
                    history
                );

            }

            if (
                action ===
                "clear"
            ) {

                await clearAllHistory();

            }

        }
    );

    document.addEventListener(
        "click",
        () => {

            hideHistoryContextMenu();

        }
    );

    window.addEventListener(
        "blur",
        hideHistoryContextMenu
    );

    document.addEventListener(
        "scroll",
        hideHistoryContextMenu,
        true
    );

}

function showHistoryContextMenu(
    event,
    historyItem
) {

    event.preventDefault();
    event.stopPropagation();

    setupHistoryContextMenu();

    const menu =
        document.getElementById(
            "testHistoryContextMenu"
        );

    if (!menu) return;

    menu._historyItem =
        historyItem;

    const alreadySaved =
        isEndpointBookmarked(
            historyItem.endpointId
        );

    const bookmarkButton =
        menu.querySelector(
            '[data-history-action="bookmark"]'
        );

    if (bookmarkButton) {

        bookmarkButton.disabled =
            alreadySaved;

        bookmarkButton.textContent =
            alreadySaved
                ? "Already Bookmarked"
                : "Add to Bookmark";

        bookmarkButton.classList.toggle(
            "opacity-50",
            alreadySaved
        );

        bookmarkButton.classList.toggle(
            "cursor-not-allowed",
            alreadySaved
        );

    }

    menu.classList.remove(
        "hidden"
    );

    const menuWidth =
        menu.offsetWidth;

    const menuHeight =
        menu.offsetHeight;

    const left =
        Math.min(
            event.clientX,
            window.innerWidth -
            menuWidth -
            8
        );

    const top =
        Math.min(
            event.clientY,
            window.innerHeight -
            menuHeight -
            8
        );

    menu.style.left =
        `${Math.max(left, 8)}px`;

    menu.style.top =
        `${Math.max(top, 8)}px`;

}

function hideHistoryContextMenu() {

    const menu =
        document.getElementById(
            "testHistoryContextMenu"
        );

    if (!menu) return;

    menu.classList.add(
        "hidden"
    );

    menu._historyItem =
        null;

}

// ============================================================
// HISTORY → ACTIVE COLLECTION
// ============================================================

async function addHistoryToBookmark(
    historyItem
) {

    const endpointId =
        historyItem.endpointId;

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        console.warn(
            "History item has no endpoint_id."
        );

        return;

    }

    await saveEndpointToActiveCollection(
        endpointId
    );

}

// ============================================================
// SAVE SELECTED ENDPOINT → ACTIVE COLLECTION
// ============================================================

async function saveSelectedEndpoint() {

    if (!selectedTestEndpoint) {

        console.warn(
            "No endpoint selected."
        );

        return;

    }

    await saveEndpointToActiveCollection(
        selectedTestEndpoint.id
    );

}

// ============================================================
// SAVE ENDPOINT → ACTIVE COLLECTION
// ============================================================

async function saveEndpointToActiveCollection(
    endpointId
) {

    if (
        endpointId === undefined ||
        endpointId === null
    ) {

        return;

    }

    if (
        isEndpointBookmarked(
            endpointId
        )
    ) {

        console.log(
            "Endpoint is already in Active Collection:",
            endpointId
        );

        return;

    }

    try {

        await window.EdmsAPI.addActiveBookmark(
            activeCollectionFilter,
            endpointId
        );

        const result =
            await window.EdmsAPI.saveActiveBookmark(
                activeCollectionFilter,
                endpointId
            );

        if (!result.ok) {

            throw new Error(
                result.data?.message ||
                `Bookmark save failed: ${result.status}`
            );

        }

        console.log(
            "Added endpoint to Active Collection:",
            endpointId
        );

        await loadBookmarksFromBackend();

        historyRecords =
            historyRecords.map(
                history => ({

                    ...history,

                    saved:
                        isEndpointBookmarked(
                            history.endpointId
                        )

                })
            );

        applyTestFilters();

    } catch (error) {

        console.error(
            "Failed to add endpoint to Active Collection:",
            error
        );

        window.alert(
            error?.message ||
            "Could not add this endpoint to Bookmark."
        );

    }

}

// ============================================================
// CLEAR ALL HISTORY
// ============================================================

async function clearAllHistory() {

    const confirmed =
        window.confirm(
            "Clear all history? This cannot be undone."
        );

    if (!confirmed) {
        return;
    }

    try {

        const result =
            await window.EdmsAPI.clearHistory();

        if (!result.ok) {

            throw new Error(
                `Clear history failed: ${result.status}`
            );

        }

        console.log(
            "Backend history cleared."
        );

        await loadHistoryFromBackend();

        applyTestFilters();

    } catch (error) {

        console.error(
            "Failed to clear history:",
            error
        );

        window.alert(
            "Could not clear history."
        );

    }

}

// ============================================================
// METHOD COLOR
// ============================================================

function getMethodColor(method) {

    switch (
    String(method || "")
        .toUpperCase()
    ) {

        case "GET":
            return "bg-emerald-500/15 text-emerald-400 border-emerald-500/20";

        case "POST":
            return "bg-amber-500/15 text-amber-400 border-amber-500/20";

        case "PUT":
            return "bg-sky-500/15 text-sky-400 border-sky-500/20";

        case "DELETE":
            return "bg-red-500/15 text-red-400 border-red-500/20";

        default:
            return "bg-slate-700 text-slate-300 border-slate-600";

    }

}

// ============================================================
// BADGE
// ============================================================

function getItemBadge(item) {

    if (
        item.type ===
        "history"
    ) {

        const status =
            item.status
                ? `${item.status} ${getStatusText(item.status)}`
                : "Tested";

        return item.saved
            ? `${status} - Saved`
            : `${status} - Not saved`;

    }

    if (
        item.type ===
        "bookmark"
    ) {

        return "Bookmarked";

    }
    return "";

}

// ============================================================
// QP COUNT
// ============================================================

function getQPCountLabel(endpoint) {

    const count =
        Array.isArray(endpoint?.qps)
            ? endpoint.qps.length
            : 0;

    return `${count} QP${count === 1 ? "" : "s"}`;

}

// ============================================================
// SELECT SIDEBAR ITEM
// ============================================================

function selectSidebarItem(
    item,
    card
) {

    const endpoint =
        item.endpointRef ||
        item.source ||
        item;

    if (!endpoint) return;

    selectTestEndpoint(
        endpoint,
        card,
        item.qpId
    );

}

// ============================================================
// SELECT ENDPOINT
// ============================================================

function selectTestEndpoint(
    endpoint,
    card,
    preferredQPId
) {

    selectedTestEndpoint =
        endpoint;

    latestResponseMeta =
        null;

    document
        .querySelectorAll(
            ".endpoint-card"
        )
        .forEach(
            item => {

                item.classList.remove(
                    "bg-sky-500/10",
                    "border-l-2",
                    "border-sky-500"
                );

            }
        );

    if (card) {

        card.classList.add(
            "bg-sky-500/10",
            "border-l-2",
            "border-sky-500"
        );

    }

    if (testMethod) {

        testMethod.value =
            endpoint.method ||
            "GET";

    }

    if (baseUrl) {

        baseUrl.value =
            endpoint.baseUrl ||
            "";

    }

    if (endpointPath) {

        endpointPath.value =
            endpoint.endpoint ||
            "";

    }

    syncAddressFullDisplay();

    if (annotationInput) {

        annotationInput.value =
            endpoint.annotation ||
            "";

    }

    /*
     * Refresh tags from backend whenever an endpoint
     * is selected, so the UI reflects the current
     * backend state.
     */
    refreshSelectedEndpointTags();

    renderTestQP(
        endpoint,
        preferredQPId
    );

    updateSelectedEndpointHighlight();

}

// ============================================================
// REFRESH SELECTED ENDPOINT TAGS
// ============================================================

async function refreshSelectedEndpointTags() {

    const endpoint = selectedTestEndpoint;

    if (!endpoint || endpoint.id === undefined || endpoint.id === null) {
        renderSelectedEndpointTags();
        return;
    }
    const idsOf = e => getPersistedQPs(e).map(q => String(q.id)).join(",");
    const idsBefore = (endpoint.qps || []).map(q => String(q.id)).join(",");

    try {
        await loadTagsAndQPsInto(endpoint);

        // Keep the master list in sync if it holds a different object
        const listed = findEndpoint(endpoint.id);
        if (listed && listed !== endpoint) {
            listed.tags = endpoint.tags;
            listed.qps = endpoint.qps;
        }
    } catch (error) {
        console.warn("Could not refresh endpoint tags or QPs:", error);
    }

    // The user may have selected another endpoint while we were waiting
    if (selectedTestEndpoint !== endpoint) return;

    renderSelectedEndpointTags();

    // If the QP list changed (e.g. the backend now returns real QPs),
    // redraw the QP buttons so the new ones show up
    const idsAfter = idsOf(endpoint);
    if (idsBefore !== idsAfter) {
        renderTestQP(endpoint, selectedTestQP?.id);
    }

}
// ============================================================
// SELECTED ENDPOINT HIGHLIGHT
// ============================================================

function updateSelectedEndpointHighlight() {

    const selectedId =
        selectedTestEndpoint?.id;

    document
        .querySelectorAll(
            ".endpoint-card"
        )
        .forEach(
            card => {

                const cardEndpointId =
                    card.dataset.endpointId;

                const active =
                    selectedId !== undefined &&
                    selectedId !== null &&
                    String(cardEndpointId) ===
                    String(selectedId);

                card.classList.toggle(
                    "bg-sky-500/10",
                    active
                );

                card.classList.toggle(
                    "border-l-2",
                    active
                );

                card.classList.toggle(
                    "border-sky-500",
                    active
                );

            }
        );

}

// ============================================================
// RENDER QP
// ============================================================

function renderTestQP(
    endpoint,
    preferredQPId
) {

    if (!testQPPanel) return;

    testQPPanel.innerHTML = "";

    selectedTestQP =
        null;

    selectedQPIds =
        new Set();

    if (
        !Array.isArray(endpoint.qps)
    ) {

        endpoint.qps = [];

    }

    if (
        endpoint.qps.length === 0
    ) {

        endpoint.qps.push({

            id:
                "default",

            name:
                "1",

            request: {

                headers: {},

                body: {}

            },

            response: {}

        });

        saveLocalQPs();

    }

    let preferredButton =
        null;

    let preferredQP =
        null;

    endpoint.qps.forEach(
        qp => {

            const wrapper =
                document.createElement(
                    "div"
                );

            wrapper.className =
                "relative";

            const button =
                document.createElement(
                    "button"
                );

            button.type =
                "button";

            button.className = `
                qp-btn
                w-full
                min-h-10
                rounded-md
                border
                border-slate-700
                bg-slate-800
                px-1
                py-2
                text-xs
                text-slate-300
                hover:bg-cyan-500
                hover:text-white
                transition-all
                duration-150
            `;

            button.textContent =
                qp.name ||
                qp.id;

            button.dataset.qp =
                qp.id;

            button.addEventListener(
                "click",
                () => {

                    selectTestQP(
                        qp,
                        button
                    );

                }
            );

            const checkbox =
                document.createElement(
                    "input"
                );

            checkbox.type =
                "checkbox";

            checkbox.className = `
                absolute
                top-0.5
                left-0.5
                h-3
                w-3
                accent-cyan-400
                z-10
            `;

            checkbox.addEventListener(
                "click",
                event => {

                    event.stopPropagation();

                }
            );

            checkbox.addEventListener(
                "change",
                () => {

                    if (
                        checkbox.checked
                    ) {

                        selectedQPIds.add(
                            String(qp.id)
                        );

                        button.classList.add(
                            "ring-2",
                            "ring-cyan-400"
                        );

                    } else {

                        selectedQPIds.delete(
                            String(qp.id)
                        );

                        button.classList.remove(
                            "ring-2",
                            "ring-cyan-400"
                        );

                    }

                }
            );

            wrapper.appendChild(
                button
            );

            wrapper.appendChild(
                checkbox
            );

            if (
                String(qp.id) ===
                String(
                    preferredQPId ||
                    endpoint.qps[0].id
                )
            ) {

                preferredButton =
                    button;

                preferredQP =
                    qp;

            }

            testQPPanel.appendChild(
                wrapper
            );

        }
    );

    selectTestQP(
        preferredQP ||
        endpoint.qps[0],

        preferredButton ||
        testQPPanel.querySelector(
            ".qp-btn"
        )
    );

}

// ============================================================
// QP MENU
// ============================================================

function setupQPMenu() {

    if (
        !qpMenuButton ||
        !qpMenu
    ) return;

    qpMenuButton.addEventListener(
        "click",
        event => {

            event.preventDefault();
            event.stopPropagation();

            qpMenu.classList.toggle(
                "hidden"
            );

        }
    );

    qpMenu.addEventListener(
        "click",
        event => {

            const button =
                event.target.closest(
                    "[data-qp-action]"
                );

            if (!button) return;

            handleQPMenuAction(
                button.dataset.qpAction
            );

            qpMenu.classList.add(
                "hidden"
            );

        }
    );

    document.addEventListener(
        "click",
        event => {

            if (
                !qpMenu.classList.contains(
                    "hidden"
                ) &&
                !qpMenu.contains(
                    event.target
                ) &&
                event.target !==
                qpMenuButton
            ) {

                qpMenu.classList.add(
                    "hidden"
                );

            }

        }
    );

}
// ============================================================
// HISTORY MENU
// ============================================================

function setupHistoryMenu() {

    const historyMenuButton =
        document.getElementById(
            "historyMenuButton"
        );

    const historyMenu =
        document.getElementById(
            "historyMenu"
        );

    if (
        !historyMenuButton ||
        !historyMenu
    ) {
        return;
    }

    historyMenuButton.addEventListener(
        "click",
        event => {

            event.preventDefault();
            event.stopPropagation();

            historyMenu.classList.toggle(
                "hidden"
            );

        }
    );

    historyMenu.addEventListener(
        "click",
        event => {

            const button =
                event.target.closest(
                    "[data-history-action]"
                );

            if (!button) {
                return;
            }

            handleHistoryMenuAction(
                button.dataset.historyAction
            );

            historyMenu.classList.add(
                "hidden"
            );

        }
    );

    document.addEventListener(
        "click",
        event => {

            if (
                !historyMenu.classList.contains(
                    "hidden"
                ) &&
                !historyMenu.contains(
                    event.target
                ) &&
                event.target !==
                historyMenuButton
            ) {

                historyMenu.classList.add(
                    "hidden"
                );

            }

        }
    );

}
// ============================================================
// QP MENU ACTION
// ============================================================

function handleQPMenuAction(action) {
    switch (action) {
        case "select-all": doSelectAllQP(); break;
        case "invert-selection": doInvertSelectionQP(); break;
        case "clear-selection": doClearSelectionQP(); break;
        case "delete-selected": doDeleteSelectedQP(); break;
    }
}
function doInvertSelectionQP() {

    if (
        !selectedTestEndpoint ||
        !Array.isArray(
            selectedTestEndpoint.qps
        )
    ) {
        return;
    }

    testQPPanel
        .querySelectorAll(
            'input[type="checkbox"]'
        )
        .forEach(
            checkbox => {

                checkbox.checked =
                    !checkbox.checked;

                checkbox.dispatchEvent(
                    new Event("change")
                );

            }
        );

}
function doInvertSelectionHistory() {

    if (activeSidebarTab !== "history") {
        return;
    }

    document
        .querySelectorAll(".history-checkbox")
        .forEach(checkbox => {

            checkbox.checked = !checkbox.checked;

            const card =
                checkbox.closest(".endpoint-card");

            const historyId =
                checkbox.dataset.historyId;

            if (checkbox.checked) {

                selectedHistoryIds.add(
                    String(historyId)
                );

                if (card) {
                    card.classList.add(
                        "ring-2",
                        "ring-cyan-400"
                    );
                }

            } else {

                selectedHistoryIds.delete(
                    String(historyId)
                );

                if (card) {
                    card.classList.remove(
                        "ring-2",
                        "ring-cyan-400"
                    );
                }

            }

        });
}
// ============================================================
// SELECT ALL QP
// ============================================================

function doSelectAllQP() {

    if (
        !selectedTestEndpoint ||
        !Array.isArray(
            selectedTestEndpoint.qps
        )
    ) return;

    selectedQPIds =
        new Set(
            selectedTestEndpoint.qps.map(
                qp =>
                    String(qp.id)
            )
        );

    testQPPanel
        .querySelectorAll(
            ".qp-btn"
        )
        .forEach(
            button => {

                button.classList.add(
                    "ring-2",
                    "ring-cyan-400"
                );

            }
        );

    testQPPanel
        .querySelectorAll(
            'input[type="checkbox"]'
        )
        .forEach(
            checkbox => {

                checkbox.checked =
                    true;

            }
        );

}

// ============================================================
// CLEAR QP SELECTION
// ============================================================

function doClearSelectionQP() {

    selectedQPIds =
        new Set();

    testQPPanel
        .querySelectorAll(
            ".qp-btn"
        )
        .forEach(
            button => {

                button.classList.remove(
                    "ring-2",
                    "ring-cyan-400"
                );

            }
        );

    testQPPanel
        .querySelectorAll(
            'input[type="checkbox"]'
        )
        .forEach(
            checkbox => {

                checkbox.checked =
                    false;

            }
        );

}

// ============================================================
// DELETE SELECTED QP
// ============================================================

async function doDeleteSelectedQP() {

    if (
        !selectedTestEndpoint ||
        !Array.isArray(
            selectedTestEndpoint.qps
        )
    ) return;

    if (
        selectedQPIds.size === 0
    ) {

        console.warn(
            "No QPs selected."
        );

        return;
    }

    const endpointId = selectedTestEndpoint.id;

    if (endpointId !== undefined && endpointId !== null) {
        const deletePromises = [];
        for (const qpId of selectedQPIds) {
            if (String(qpId) !== "default") {
                deletePromises.push(
                    fetch(
                        `${API_BASE}/test-view/${encodeURIComponent(endpointId)}/qps/${encodeURIComponent(qpId)}/delete`,
                        { method: "POST" }
                    ).catch(error => {
                        console.error(`Error deleting QP ${qpId}:`, error);
                    })
                );
            }
        }
        if (deletePromises.length > 0) {
            await Promise.all(deletePromises);
        }
    }

    selectedTestEndpoint.qps =
        selectedTestEndpoint.qps.filter(
            qp =>
                !selectedQPIds.has(
                    String(qp.id)
                )
        );

    saveLocalQPs();

    renderTestQP(
        selectedTestEndpoint
    );

    applyTestFilters();

}

// ============================================================
// SELECT QP
// ============================================================

async function selectTestQP(
    qp,
    button
) {

    selectedTestQP =
        qp;

    latestResponseMeta =
        null;

    document
        .querySelectorAll(
            ".qp-btn"
        )
        .forEach(
            item => {

                item.classList.remove(
                    "bg-cyan-500",
                    "text-white"
                );

                item.classList.add(
                    "bg-slate-800",
                    "text-slate-300"
                );

            }
        );

    if (button) {

        button.classList.remove(
            "bg-slate-800",
            "text-slate-300"
        );

        button.classList.add(
            "bg-cyan-500",
            "text-white"
        );

    }

    renderCurrentRequest();
    renderCurrentResponse();

    if (qp.id !== "default" && selectedTestEndpoint?.id && !qp.isFullDataLoaded) {
        try {
            const endpointId = encodeURIComponent(selectedTestEndpoint.id);
            const qpId = encodeURIComponent(qp.id);

            const [reqRes, resRes, headRes] = await Promise.all([
                fetch(`${API_BASE}/test-view/${endpointId}/request/${qpId}`).catch(() => null),
                fetch(`${API_BASE}/test-view/${endpointId}/response/${qpId}`).catch(() => null),
                fetch(`${API_BASE}/test-view/${endpointId}/headers/${qpId}`).catch(() => null)
            ]);

            qp.request = qp.request || {};
            qp.response = qp.response || {};

            if (reqRes && reqRes.ok) {
                const text = await reqRes.text();
                try { qp.request.body = JSON.parse(text); }
                catch { qp.request.body = text; }
            }

            if (resRes && resRes.ok) {
                const text = await resRes.text();
                try { qp.response.body = JSON.parse(text); }
                catch { qp.response.body = text; }
            }

            if (headRes && headRes.ok) {
                const text = await headRes.text();
                try {
                    const headersData = JSON.parse(text);
                    qp.request.headers = headersData.request_headers || {};
                    qp.response.headers = headersData.response_headers || {};
                } catch { }
            }

            qp.isFullDataLoaded = true;

            if (selectedTestQP === qp) {
                renderCurrentRequest();
                renderCurrentResponse();
            }
        } catch (error) {
            console.error("Failed to load full QP data:", error);
        }
    }

}

// ============================================================
// REQUEST
// ============================================================

function renderCurrentRequest() {

    if (!requestContent) return;

    if (!selectedTestQP) {

        requestContent.value =
            "";

        return;

    }

    const request =
        selectedTestQP.request ||
        {};

    const content =
        activeRequestTab ===
            "headers"

            ? request.headers || {}

            : getRequestBodyPreview(
                request
            );

    requestContent.value =
        formatJSON(content);

}

// ============================================================
// RESPONSE
// ============================================================

function renderCurrentResponse() {

    if (!responseContent) return;

    if (!selectedTestQP) {

        responseContent.value =
            "";

        return;

    }

    const response =
        selectedTestQP.response ||
        {};

    const content =
        activeResponseTab ===
            "headers"

            ? getResponseHeadersPreview(
                response
            )

            : response.body ?? {};

    responseContent.value =
        formatJSON(content);

}

// ============================================================
// REQUEST BODY
// ============================================================

function getRequestBodyPreview(request) {

    if (
        request.body !== undefined
    ) {

        return request.body;

    }

    return {};

}

// ============================================================
// RESPONSE HEADERS
// ============================================================

function getResponseHeadersPreview(
    response
) {

    const status =
        response.status ||
        200;

    const headers = {

        status:
            `${status} ${getStatusText(status)}`.trim(),

        ...(response.headers || {})

    };

    if (
        latestResponseMeta?.elapsed !==
        undefined
    ) {

        headers.time =
            `${latestResponseMeta.elapsed} ms`;

    }

    return headers;

}

// ============================================================
// CLEAR
// ============================================================

function clearRequestResponse() {

    if (requestContent) {

        requestContent.value =
            "";

    }

    if (responseContent) {

        responseContent.value =
            "";

    }

}

// ============================================================
// FORMAT JSON
// ============================================================

function formatJSON(value) {

    try {

        return JSON.stringify(
            value ?? {},
            null,
            4
        );

    } catch {

        return String(
            value ?? ""
        );

    }

}

// ============================================================
// SEARCH
// ============================================================

function setupTestSearch() {

    if (!testSearchInput) return;

    testSearchInput.addEventListener(
        "input",
        applyTestFilters
    );

}

// ============================================================
// METHOD FILTER
// ============================================================

function setupMethodFilters() {

    [
        "GET",
        "POST",
        "PUT",
        "DELETE"
    ].forEach(
        method => {

            const button =
                document.getElementById(
                    `method${method}`
                );

            if (!button) return;

            button.addEventListener(
                "click",
                event => {

                    event.preventDefault();

                    activeTestMethod =
                        activeTestMethod ===
                            method
                            ? "ALL"
                            : method;

                    updateMethodButtons();
                    applyTestFilters();

                }
            );

        }
    );

}

// ============================================================
// METHOD BUTTONS
// ============================================================

function updateMethodButtons() {

    [
        "GET",
        "POST",
        "PUT",
        "DELETE"
    ].forEach(
        method => {

            const button =
                document.getElementById(
                    `method${method}`
                );

            if (!button) return;

            button.classList.remove(
                "ring-2",
                "ring-cyan-400"
            );

            if (
                activeTestMethod ===
                method
            ) {

                button.classList.add(
                    "ring-2",
                    "ring-cyan-400"
                );

            }

        }
    );

}

// ============================================================
// TIME FILTER
// ============================================================

function setupTimeFilter() {

    if (!timeFilter) return;

    timeFilter.addEventListener(
        "change",
        () => {

            activeTimeFilter =
                timeFilter.value;

            applyTestFilters();

        }
    );

}

// ============================================================
// URL FILTER
// ============================================================

function setupURLFilter() {

    if (!urlFilter) return;

    urlFilter.addEventListener(
        "input",
        applyTestFilters
    );

}

// ============================================================
// FILTER
// ============================================================

function applyTestFilters() {

    const search =
        testSearchInput
            ? testSearchInput.value
                .trim()
                .toLowerCase()
            : "";

    const url =
        urlFilter
            ? urlFilter.value
                .trim()
                .toLowerCase()
            : "";

    filteredTestEndpoints =
        getActiveSidebarItems()
            .filter(
                item => {

                    const endpoint =
                        item.endpointRef ||
                        item.source ||
                        item;

                    /*
                     * Collection filter — only show endpoints
                     * that are members of the active collection
                     * (when the page was opened via ?collection=).
                     */
                    if (
                        activeSidebarTab !== "history" &&
                        activeCollectionEndpointIds !== null
                    ) {

                        const itemId =
                            String(
                                item.id ??
                                item.endpointId ??
                                item.endpoint_id ??
                                ''
                            );

                        if (
                            !activeCollectionEndpointIds.has(
                                itemId
                            )
                        ) {

                            return false;

                        }

                    }

                    const haystack = [

                        item.id,
                        item.endpointId,
                        item.method,
                        item.endpoint,
                        item.status,
                        endpoint?.annotation,
                        ...(endpoint?.tags || [])

                    ]
                        .join(" ")
                        .toLowerCase();

                    return (

                        (
                            !search ||
                            haystack.includes(
                                search
                            )
                        ) &&

                        (
                            activeTestMethod ===
                            "ALL" ||
                            item.method ===
                            activeTestMethod
                        ) &&

                        (
                            !url ||
                            String(
                                item.endpoint ||
                                ""
                            )
                                .toLowerCase()
                                .includes(url)
                        ) &&

                        matchesTimeFilter(
                            item.updated
                        )

                    );

                }
            );

    renderTestEndpoints();

}


// ============================================================
// TIME MATCH
// ============================================================

function matchesTimeFilter(
    dateString
) {

    if (
        activeTimeFilter ===
        "all"
    ) {

        return true;
    }

    if (!dateString) {

        return false;
    }

    const date =
        new Date(dateString);

    if (
        Number.isNaN(
            date.getTime()
        )
    ) {

        return false;
    }

    const now =
        new Date();

    const day =
        24 *
        60 *
        60 *
        1000;

    const difference =
        now - date;

    switch (
    activeTimeFilter
    ) {

        case "today":

            return (
                date.toDateString() ===
                now.toDateString()
            );

        case "week":

        case "7days":

            return (
                difference >= 0 &&
                difference <=
                7 * day
            );

        case "30days":

            return (
                difference >= 0 &&
                difference <=
                30 * day
            );

        default:

            return true;

    }

}

// ============================================================
// RUNNER
// ============================================================

function setupRunner() {

    if (runButton) {

        runButton.type =
            "button";

        runButton.addEventListener(
            "click",
            event => {

                event.preventDefault();
                event.stopPropagation();

                if (runButton.dataset.running === "true") {
                    stopTestEndpoint();
                } else {
                    runTestEndpoint();
                }

            }
        );

    }

    const saveDropdownContainer = document.getElementById("saveDropdownContainer");
    const saveMenu = document.getElementById("saveMenu");
    const btnUpdateQP = document.getElementById("btnUpdateQP");
    const btnCreateQP = document.getElementById("btnCreateQP");
    if (saveButton && saveMenu && saveDropdownContainer) {
        saveButton.type = "button";

        saveButton.addEventListener("click", event => {
            event.preventDefault();
            event.stopPropagation();
            saveMenu.classList.toggle("hidden");
            if (!saveMenu.classList.contains("hidden")) refreshSaveMenuState();
        });

        document.addEventListener("click", event => {
            if (!saveDropdownContainer.contains(event.target)) {
                saveMenu.classList.add("hidden");
            }
        });

        document.getElementById("btnCreateQP")?.addEventListener("click", event => {
            event.preventDefault();
            event.stopPropagation();
            saveMenu.classList.add("hidden");
            runSave(saveAsNewQP);
        });

        document
            .getElementById("btnUpdateQP")
            ?.addEventListener(
                "click",
                event => {

                    event.preventDefault();
                    event.stopPropagation();

                    if (
                        event.currentTarget.disabled
                    ) {
                        return;
                    }

                    saveMenu.classList.add(
                        "hidden"
                    );

                    runSave(
                        updateExistingQP
                    );

                }
            );
    }

    const runForm =
        runButton?.closest("form");

    if (runForm) {

        runForm.addEventListener(
            "submit",
            event => {

                event.preventDefault();
                event.stopPropagation();

                console.log(
                    "TestView form submission blocked."
                );

            }
        );

    }

}
// ============================================================
// SAVE (new QP / update QP)
// ============================================================

let saveInFlight = false;

async function runSave(fn) {
    if (saveInFlight) return;
    saveInFlight = true;
    try {
        await fn();
    } catch (error) {
        console.error("Save failed:", error);
        window.alert(error?.message || "Could not save.");
    } finally {
        saveInFlight = false;
    }
}

function findEndpointByAddress(endpointStr, method) {

    if (isCurrentInputExistingEndpoint(selectedTestEndpoint, endpointStr, method)) {
        return selectedTestEndpoint;
    }

    const normalizedAddress = normalizeAddress(endpointStr);
    const normalizedMethod = String(method || "GET").toUpperCase();

    return endpoints.find(endpoint =>
        normalizeAddress(endpoint.endpoint_str) === normalizedAddress &&
        String(endpoint.method || "GET").toUpperCase() === normalizedMethod
    ) || null;
}

function getPersistedQPs(endpoint) {

    if (
        !endpoint ||
        !Array.isArray(endpoint.qps)
    ) {
        return [];
    }

    return endpoint.qps.filter(
        qp =>
            String(qp.id) !==
            "default"
    );

}
function currentAddressAndMethod() {

    return {
        endpointStr:
            getCurrentEndpointInput(),

        method:
            String(
                testMethod?.value ||
                "GET"
            ).toUpperCase()
    };

}

function refreshSaveMenuState() {

    const {
        endpointStr,
        method
    } = currentAddressAndMethod();

    const existing =
        findEndpointByAddress(
            endpointStr,
            method
        );

    const canUpdate =
        Boolean(existing) &&
        getPersistedQPs(existing).length > 0;

    const updateButton =
        document.getElementById(
            "btnUpdateQP"
        );

    if (!updateButton) return;

    updateButton.disabled =
        !canUpdate;

    updateButton.classList.toggle(
        "opacity-50",
        !canUpdate
    );

    updateButton.classList.toggle(
        "cursor-not-allowed",
        !canUpdate
    );

    updateButton.title =
        canUpdate
            ? "Update an existing QP"
            : "No saved QP exists for this endpoint";

}

// Request/response text exactly as the user sees it. If the Headers tab is
// open, the textarea holds headers, so use the stored body instead.
function getEditorPayload() {

    return {

        request_body:
            requestContent?.value || "",

        response_body:
            responseContent?.value || ""

    };

}

function parseMaybeJSON(text) {
    try { return JSON.parse(text); } catch { return text; }
}

// [B] New endpoint. TODO: wire to the backend route that creates an
// endpoint without running a test, and return the normalized endpoint.
async function createEndpointOnBackend({ method, endpointStr, annotation }) {
    throw new Error(
        "Creating a new endpoint from Save isn't connected to the backend yet."
    );
}

async function resolveEndpointForSave() {
    const { endpointStr, method } = currentAddressAndMethod();

    if (!endpointStr) {
        window.alert("Enter an endpoint URL first.");
        return null;
    }

    const existing = findEndpointByAddress(endpointStr, method);
    if (existing) return existing;

    const created = await createEndpointOnBackend({
        method,
        endpointStr,
        annotation: annotationInput?.value || ""
    });

    const endpoint = normalizeBackendEndpoint(created);
    endpoints.push(endpoint);
    return endpoint;
}
async function saveAsNewQP() {

    const endpoint =
        await resolveEndpointForSave();

    if (!endpoint) return;

    const payload =
        getEditorPayload();

    const annotation =
        annotationInput?.value || "";

    const response =
        await fetch(
            `${API_BASE}/test-view/${encodeURIComponent(endpoint.id)}/qps/create`,
            {
                method: "POST",

                headers: {
                    "Content-Type":
                        "application/json"
                },

                body: JSON.stringify({

                    method:
                        endpoint.method || "GET",

                    annotation,

                    ...payload

                })

            }
        );

    if (!response.ok) {

        throw new Error(
            `QP creation failed: ${response.status}`
        );

    }

    const data =
        await response.json();

    const requestNumber =
        data.request_number;

    if (
        requestNumber ===
        undefined ||
        requestNumber ===
        null
    ) {

        throw new Error(
            "Backend did not return request_number."
        );

    }

    const newQP = {

        id:
            requestNumber,

        name:
            String(requestNumber),

        method:
            endpoint.method,

        timestamp:
            new Date().toISOString(),

        isFullDataLoaded:
            true,

        request: {

            body:
                parseMaybeJSON(
                    payload.request_body
                ),

            headers:
                selectedTestQP?.request
                    ?.headers || {}

        },

        response: {

            body:
                parseMaybeJSON(
                    payload.response_body
                ),

            headers: {},

            status: null

        }

    };

    if (
        !Array.isArray(endpoint.qps)
    ) {

        endpoint.qps = [];

    }

    endpoint.qps = [...getPersistedQPs(endpoint), newQP];

    selectedTestEndpoint =
        endpoint;

    selectedTestQP =
        newQP;

    saveLocalQPs();

    renderTestQP(
        endpoint,
        newQP.id
    );

    applyTestFilters();

    refreshSaveMenuState();

}
async function updateExistingQP() {

    const {
        endpointStr,
        method
    } = currentAddressAndMethod();

    const endpoint =
        findEndpointByAddress(
            endpointStr,
            method
        );

    if (!endpoint) {

        window.alert(
            "No existing endpoint found."
        );

        return;

    }

    const persisted = getPersistedQPs(endpoint);

    if (persisted.length === 0) {
        window.alert(
            "No saved QP exists for this endpoint."
        );
        return;
    }

    const target =
        await pickQPDialog(
            persisted,
            selectedTestQP?.id
        );

    if (!target) return;

    const payload =
        getEditorPayload();

    const annotation =
        annotationInput?.value || "";

    const response =
        await fetch(
            `${API_BASE}/test-view/${encodeURIComponent(endpoint.id)}/qps/${encodeURIComponent(target.id)}/update`,
            {
                method: "POST",

                headers: {
                    "Content-Type":
                        "application/json"
                },

                body: JSON.stringify({

                    annotation,

                    ...payload

                })

            }
        );

    if (!response.ok) {

        throw new Error(
            `QP update failed: ${response.status}`
        );

    }

    target.request =
        target.request || {};

    target.response =
        target.response || {};

    target.request.body =
        parseMaybeJSON(
            payload.request_body
        );

    target.response.body =
        parseMaybeJSON(
            payload.response_body
        );

    target.isFullDataLoaded =
        true;

    endpoint.annotation =
        annotation;

    selectedTestEndpoint =
        endpoint;

    selectedTestQP =
        target;

    saveLocalQPs();

    renderTestQP(
        endpoint,
        target.id
    );

    applyTestFilters();

    refreshSaveMenuState();

}
function pickQPDialog(qps, preselectedId) {
    return new Promise(resolve => {
        const overlay = document.createElement("div");
        overlay.className =
            "fixed inset-0 z-[10000] flex items-center justify-center bg-black/60";

        overlay.innerHTML = `
            <div class="w-72 rounded-xl border border-slate-700 bg-slate-900 p-4 shadow-xl">
                <p class="mb-3 text-sm font-semibold text-slate-200">Update which QP?</p>
                <div data-list class="grid grid-cols-4 gap-2 max-h-64 overflow-y-auto p-1"></div>
                <button type="button" data-cancel
                    class="mt-4 w-full rounded-md border border-slate-700 py-2 text-xs text-slate-400 hover:bg-slate-800">
                    Cancel
                </button>
            </div>`;

        const onKey = e => { if (e.key === "Escape") close(null); };
        const close = value => {
            document.removeEventListener("keydown", onKey);
            overlay.remove();
            resolve(value);
        };

        const list = overlay.querySelector("[data-list]");
        qps.forEach(qp => {
            const b = document.createElement("button");
            b.type = "button";
            b.textContent = qp.name || qp.id;
            b.className =
                "rounded-md border border-slate-700 bg-slate-800 py-2 text-xs text-slate-200 hover:bg-cyan-500 hover:text-white";
            if (String(qp.id) === String(preselectedId)) {
                b.classList.add("ring-2", "ring-cyan-400");
            }
            b.addEventListener("click", () => close(qp));
            list.appendChild(b);
        });

        overlay.addEventListener("click", e => {
            if (e.target === overlay || e.target.hasAttribute("data-cancel")) close(null);
        });
        document.addEventListener("keydown", onKey);
        document.body.appendChild(overlay);
    });
}

// ============================================================
// GET CURRENT INPUT ENDPOINT
// ============================================================
function getCurrentEndpointInput() {

    if (
        addressCombinedMode &&
        urlFullInput
    ) {

        return urlFullInput.value.trim();

    }

    return joinAddressParts(
        baseUrl?.value,
        endpointPath?.value
    );

}

// ============================================================
// DETERMINE WHETHER CURRENT INPUT IS THE
// SELECTED STORED ENDPOINT
// ============================================================

function isCurrentInputExistingEndpoint(
    endpoint,
    endpointStr,
    method
) {

    if (
        !endpoint ||
        endpoint.id === undefined ||
        endpoint.id === null
    ) {

        return false;
    }

    const storedURL =
        String(
            endpoint.endpoint_str ||
            (
                endpoint.baseUrl ||
                ""
            ) +
            (
                endpoint.endpoint ||
                ""
            )
        ).trim();

    const currentURL =
        String(
            endpointStr ||
            ""
        ).trim();

    const storedMethod =
        String(
            endpoint.method ||
            "GET"
        ).toUpperCase();

    const currentMethod =
        String(
            method ||
            "GET"
        ).toUpperCase();

    return (
        normalizeAddress(storedURL) === normalizeAddress(currentURL) &&
        storedMethod === currentMethod
    );

}

// ============================================================
// RUN TEST
// ============================================================

async function runTestEndpoint() {

    /*
     * The selected endpoint is NOT automatically
     * considered the endpoint being tested.
     *
     * Same URL + same method:
     *     retest existing endpoint.
     *
     * Different URL/method:
     *     create/test a new endpoint.
     */

    const method =
        String(
            testMethod?.value ||
            selectedTestEndpoint?.method ||
            "GET"
        ).toUpperCase();

    const endpointStr =
        getCurrentEndpointInput();

    if (!endpointStr) {

        window.alert(
            "Enter an endpoint URL first."
        );

        return;
    }

    const existingEndpoint =
        isCurrentInputExistingEndpoint(
            selectedTestEndpoint,
            endpointStr,
            method
        )
            ? selectedTestEndpoint
            : null;

    const endpointId =
        existingEndpoint?.id;

    console.log(
        "Test target resolved:",
        {
            endpointId,
            endpointStr,
            method,
            existing:
                Boolean(existingEndpoint)
        }
    );

    if (!selectedTestQP) {

        selectedTestQP = {

            id:
                "default",

            name:
                "Default Request",

            request: {

                headers: {},

                body: {}

            },

            response: {}

        };

    }

    setRunButtonState(true);

    testRunStartedAt =
        performance.now();

    activeRequestNumber =
        null;

    try {

        // -------------------------------------
        // CONNECT TO BACKEND
        // -------------------------------------

        const ws =
            window.EdmsAPI
                .connectTestView();

        testWS =
            ws;

        await waitForWebSocketOpen(
            ws
        );

        // -------------------------------------
        // LISTEN FOR BACKEND EVENTS
        // -------------------------------------

        const finishedPromise =
            window.EdmsAPI
                .waitForTestFinished(
                    ws,
                    {

                        onStarted:
                            handleTestStarted,

                        onTick:
                            handleTestTick,

                        onFinished:
                            handleTestFinished,

                        onTimeout:
                            handleTestTimeout,

                        onError:
                            handleTestError

                    }
                );

        // -------------------------------------
        // REQUEST / BODY
        // -------------------------------------

        const request =
            selectedTestQP.request ||
            {};

        let requestJson =
            request.body ?? {};

        if (
            activeRequestTab ===
            "body" &&
            requestContent
        ) {

            const text =
                requestContent.value
                    .trim();

            if (text) {

                try {

                    requestJson =
                        JSON.parse(text);

                } catch {

                    throw new Error(
                        "Request body is not valid JSON."
                    );

                }

            } else {

                requestJson = {};

            }

        }

        // -------------------------------------
        // SEND TO BACKEND
        // -------------------------------------

        console.log(
            "Starting backend test:",
            {

                endpointId,

                endpointStr,

                method,

                body:
                    requestJson

            }
        );

        window.EdmsAPI.startTest(
            ws,

            endpointId,

            endpointStr,

            method,

            requestJson,

            30000,

            500,

            request.headers,

            existingEndpoint?.annotation
        );

        // -------------------------------------
        // WAIT FOR TEST
        // -------------------------------------

        const event =
            await finishedPromise;

        console.log(
            "Test finished:",
            event
        );

        // -------------------------------------
        // RESOLVE ENDPOINT ID
        // -------------------------------------

        let resolvedEndpointId =
            event.payload?.endpoint_id ??
            event.payload?.endpointId ??
            event.endpoint_id ??
            event.endpointId ??
            activeTestEndpointIdFromState();

        if (
            resolvedEndpointId ===
            undefined ||
            resolvedEndpointId ===
            null
        ) {

            console.log(
                "Endpoint ID not present in test event. Reloading endpoint snapshot..."
            );

            await loadEndpointsFromBackend();

            const createdEndpoint =
                endpoints.find(
                    endpoint => {

                        const sameURL =
                            String(
                                endpoint.endpoint_str ||
                                ""
                            ) ===
                            String(
                                endpointStr
                            );

                        const sameMethod =
                            String(
                                endpoint.method ||
                                "GET"
                            ).toUpperCase() ===
                            method;

                        return (
                            sameURL &&
                            sameMethod
                        );

                    }
                );

            if (createdEndpoint) {

                resolvedEndpointId =
                    createdEndpoint.id;

            }

        }

        if (
            resolvedEndpointId ===
            undefined ||
            resolvedEndpointId ===
            null
        ) {

            throw new Error(
                "Backend created the test, but the endpoint ID could not be resolved."
            );

        }

        console.log(
            "Resolved endpoint ID:",
            resolvedEndpointId
        );

        // -------------------------------------
        // GET REAL BACKEND ENDPOINT
        // -------------------------------------

        let backendEndpoint =
            findEndpoint(
                resolvedEndpointId
            );

        if (!backendEndpoint) {

            await loadEndpointsFromBackend();

            backendEndpoint =
                findEndpoint(
                    resolvedEndpointId
                );

        }

        if (backendEndpoint) {

            const localQP =
                selectedTestQP;

            const previousQPIds =
                Array.isArray(
                    backendEndpoint.qps
                )
                    ? backendEndpoint.qps.map(
                        qp =>
                            String(qp.id)
                    )
                    : [];

            selectedTestEndpoint =
                backendEndpoint;

            if (
                !Array.isArray(
                    selectedTestEndpoint.qps
                )
            ) {

                selectedTestEndpoint.qps =
                    [];

            }

            if (
                selectedTestEndpoint.qps.length ===
                0
            ) {

                selectedTestEndpoint.qps.push(
                    localQP
                );

            } else if (
                !previousQPIds.includes(
                    String(localQP.id)
                ) &&
                String(localQP.id) ===
                "default"
            ) {

                selectedTestEndpoint.qps.unshift(
                    localQP
                );

            }

            selectedTestQP =
                selectedTestEndpoint.qps.find(
                    qp =>
                        String(qp.id) ===
                        String(localQP.id)
                ) ||
                selectedTestEndpoint.qps[0];

            renderSelectedEndpointTags();

            renderTestQP(
                selectedTestEndpoint,
                selectedTestQP?.id
            );

            updateSelectedEndpointHighlight();

        } else {

            if (!selectedTestEndpoint) {

                selectedTestEndpoint = {

                    id:
                        resolvedEndpointId,

                    method,

                    endpoint_str:
                        endpointStr,

                    baseUrl:
                        baseUrl?.value.trim() ||
                        "",

                    endpoint:
                        endpointPath?.value.trim() ||
                        endpointStr,

                    annotation:
                        annotationInput?.value ||
                        "",

                    qps: [],

                    tags: []

                };

                endpoints.push(
                    selectedTestEndpoint
                );

            } else {

                selectedTestEndpoint.id =
                    resolvedEndpointId;

            }

        }

        // -------------------------------------
        // REQUEST NUMBER
        // -------------------------------------

        const requestNumber =
            event.payload?.request_number ??
            event.payload?.requestNumber ??
            event.request_number ??
            event.requestNumber ??
            activeRequestNumber;

        if (
            requestNumber ===
            undefined ||
            requestNumber ===
            null
        ) {

            throw new Error(
                "Backend did not return a request_number."
            );

        }

        activeRequestNumber =
            requestNumber;

        // -------------------------------------
        // DIRECT REQUEST / RESPONSE RETRIEVAL
        // -------------------------------------
        //
        // There is NO RWR popup in Test View.
        //
        // TestFinished means the operation is
        // complete, so immediately retrieve the
        // actual saved request and response.

        const [
            requestResult,
            responseResult
        ] =
            await Promise.all([

                window.EdmsAPI.fetchRequest(
                    resolvedEndpointId,
                    requestNumber
                ),

                window.EdmsAPI.fetchResponse(
                    resolvedEndpointId,
                    requestNumber
                )

            ]);

        if (
            !requestResult?.ok
        ) {

            throw new Error(
                requestResult?.data?.message ||
                `Request retrieval failed: ${requestResult?.status}`
            );

        }

        if (
            !responseResult?.ok
        ) {

            throw new Error(
                responseResult?.data?.message ||
                `Response retrieval failed: ${responseResult?.status}`
            );

        }

        // -------------------------------------
        // FIND / CREATE QP
        // -------------------------------------

        let endpoint =
            findEndpoint(
                resolvedEndpointId
            );

        if (!endpoint) {

            await loadEndpointsFromBackend();

            endpoint =
                findEndpoint(
                    resolvedEndpointId
                );

        }

        if (!endpoint) {

            endpoint = {

                id:
                    resolvedEndpointId,

                method,

                endpoint_str:
                    endpointStr,

                endpoint:
                    endpointPath?.value.trim() ||
                    endpointStr,

                baseUrl:
                    baseUrl?.value.trim() ||
                    "",

                annotation:
                    annotationInput?.value ||
                    "",

                qps: [],

                tags: []

            };

            endpoints.push(
                endpoint
            );

        }

        if (
            !Array.isArray(
                endpoint.qps
            )
        ) {

            endpoint.qps =
                [];

        }

        let qp =
            endpoint.qps.find(
                candidate =>
                    String(candidate.id) ===
                    String(selectedTestQP?.id)
            );

        if (!qp) {

            qp =
                selectedTestQP
                    ? JSON.parse(
                        JSON.stringify(
                            selectedTestQP
                        )
                    )
                    : {

                        id:
                            "default",

                        name:
                            "Default Request",

                        request: {

                            headers: {},

                            body: {}

                        },

                        response: {}

                    };

            endpoint.qps.push(
                qp
            );

        }

        // -------------------------------------
        // APPLY REQUEST DATA DIRECTLY
        // -------------------------------------

        qp.request =
            qp.request ||
            {};

        const savedRequest =
            requestResult.data;

        if (
            savedRequest?.body !==
            undefined
        ) {

            qp.request.body =
                savedRequest.body;

        } else if (
            savedRequest !==
            undefined
        ) {

            qp.request.body =
                savedRequest;

        }

        if (
            savedRequest?.headers
        ) {

            qp.request.headers =
                savedRequest.headers;

        }

        // -------------------------------------
        // RESPONSE META
        // -------------------------------------

        const statusCode =
            event.payload?.status_code ??
            event.payload?.status ??
            event.status_code ??
            event.status ??
            200;

        const elapsed =
            event.payload?.response_time_ms ??
            event.payload?.elapsed_ms ??
            event.payload?.elapsed ??
            event.response_time_ms ??
            event.elapsed_ms ??
            event.elapsed ??
            null;

        // -------------------------------------
        // APPLY RESPONSE DATA DIRECTLY
        // -------------------------------------

        qp.response =
            qp.response ||
            {};

        const savedResponse =
            responseResult.data;

        if (
            savedResponse?.body !==
            undefined
        ) {

            qp.response.body =
                savedResponse.body;

        } else if (
            savedResponse !==
            undefined
        ) {

            qp.response.body =
                savedResponse;

        }

        qp.response.status =
            statusCode;

        if (
            savedResponse?.headers
        ) {

            qp.response.headers =
                savedResponse.headers;

        }

        latestResponseMeta = {

            elapsed,

            testedAt:
                new Date().toISOString()

        };

        // -------------------------------------
        // SAVE LOCAL QP
        // -------------------------------------

        saveLocalQPs();

        // -------------------------------------
        // SELECT / DISPLAY
        // -------------------------------------

        selectedTestEndpoint =
            endpoint;

        selectedTestQP =
            qp;

        activeRequestNumber =
            requestNumber;

        renderSelectedEndpointTags();

        renderTestQP(
            endpoint,
            qp.id
        );

        updateSelectedEndpointHighlight();

        renderCurrentRequest();

        renderCurrentResponse();

        applyTestFilters();

        // -------------------------------------
        // REFRESH TAGS
        // -------------------------------------

        await refreshSelectedEndpointTags();

        // -------------------------------------
        // UPDATE SIDEBAR
        // -------------------------------------

        if (
            activeSidebarTab ===
            "endpoints"
        ) {

            applyTestFilters();

        }

        // -------------------------------------
        // RELOAD HISTORY
        // -------------------------------------

        try {

            await loadHistoryFromBackend();

            applyTestFilters();

        } catch (
        historyReloadError
        ) {

            console.warn(
                "Could not reload backend history:",
                historyReloadError
            );

        }

    } catch (error) {

        console.error(
            "Test View run failed:",
            error
        );

        if (responseContent) {

            responseContent.value =
                formatJSON({

                    status:
                        "Error",

                    message:
                        error?.message ||
                        String(error)

                });

        }

    } finally {

        if (testWS) {

            try {

                testWS.close();

            } catch { }

            testWS =
                null;

        }

        if (runTimer) {

            clearTimeout(
                runTimer
            );

            runTimer =
                null;

        }

        testRunStartedAt =
            null;

        activeRequestNumber =
            null;

        setRunButtonState(
            false
        );

    }

}

// ============================================================
// WAIT FOR WS OPEN
// ============================================================

function waitForWebSocketOpen(ws) {

    return new Promise(
        (resolve, reject) => {

            if (
                ws.readyState ===
                WebSocket.OPEN
            ) {

                resolve();

                return;

            }

            const handleOpen =
                () => {

                    cleanup();

                    resolve();

                };

            const handleError =
                () => {

                    cleanup();

                    reject(
                        new Error(
                            "Could not connect to Test View WebSocket."
                        )
                    );

                };

            const cleanup =
                () => {

                    ws.removeEventListener(
                        "open",
                        handleOpen
                    );

                    ws.removeEventListener(
                        "error",
                        handleError
                    );

                };

            ws.addEventListener(
                "open",
                handleOpen
            );

            ws.addEventListener(
                "error",
                handleError
            );

        }
    );

}

// ============================================================
// TEST STARTED
// ============================================================

function handleTestStarted(event) {

    console.log(
        "TestStarted:",
        event
    );

    const requestNumber =
        event.payload?.request_number ??
        event.payload?.requestNumber ??
        event.request_number ??
        event.requestNumber;

    if (
        requestNumber !==
        undefined &&
        requestNumber !==
        null
    ) {

        activeRequestNumber =
            requestNumber;

    }

    /*
     * Backend may provide the endpoint ID
     * when an endpoint is auto-created.
     *
     * Capture it immediately so Stop works.
     */

    const endpointId =
        event.payload?.endpoint_id ??
        event.payload?.endpointId ??
        event.endpoint_id ??
        event.endpointId;

    if (
        endpointId !==
        undefined &&
        endpointId !==
        null
    ) {

        if (!selectedTestEndpoint) {

            selectedTestEndpoint = {

                id:
                    endpointId,

                method:
                    testMethod?.value ||
                    "GET",

                endpoint_str:
                    getCurrentEndpointInput(),

                endpoint:
                    endpointPath?.value ||
                    "",

                baseUrl:
                    baseUrl?.value ||
                    "",

                qps:
                    [],

                tags:
                    []

            };

            endpoints.push(
                selectedTestEndpoint
            );

        } else {

            selectedTestEndpoint.id =
                endpointId;

        }

        updateSelectedEndpointHighlight();

    }

    if (responseContent) {

        responseContent.value =
            formatJSON({

                status:
                    "Running",

                message:
                    "Request is being executed...",

                request_number:
                    requestNumber ??
                    "pending"

            });

    }

}

// ============================================================
// TIMER TICK
// ============================================================

function handleTestTick(event) {

    const payload =
        event.payload ||
        {};

    const elapsed =
        payload.elapsed_ms ??
        payload.elapsed ??
        payload.elapsedMs;

    const remaining =
        payload.remaining_ms ??
        payload.remaining ??
        payload.remainingMs;

    if (!responseContent) return;

    const info = {

        status:
            "Running"

    };

    if (
        elapsed !==
        undefined
    ) {

        info.elapsed =
            `${elapsed} ms`;

    }

    if (
        remaining !==
        undefined
    ) {

        info.remaining =
            `${remaining} ms`;

    }

    responseContent.value =
        formatJSON(info);

}

// ============================================================
// TEST FINISHED
// ============================================================

function handleTestFinished(event) {

    console.log(
        "TestFinished:",
        event
    );

    const requestNumber =
        event.payload?.request_number ??
        event.payload?.requestNumber ??
        event.request_number ??
        event.requestNumber;

    if (
        requestNumber !==
        undefined &&
        requestNumber !==
        null
    ) {

        activeRequestNumber =
            requestNumber;

    }

}

// ============================================================
// TEST TIMEOUT
// ============================================================

function handleTestTimeout(event) {

    console.warn(
        "TestTimeout:",
        event
    );

    if (responseContent) {

        responseContent.value =
            formatJSON({

                status:
                    "Timeout",

                message:
                    "Backend reported that the request timed out."

            });

    }

}

// ============================================================
// TEST ERROR
// ============================================================

function handleTestError(event) {

    console.error(
        "Backend test error:",
        event
    );

    const message =
        event?.payload?.message ??
        event?.message ??
        "Backend returned an error.";

    if (responseContent) {

        responseContent.value =
            formatJSON({

                status:
                    "Error",

                message

            });

    }

}

// ============================================================
// STOP
// ============================================================

async function stopTestEndpoint() {

    if (runTimer) {

        clearTimeout(
            runTimer
        );

        runTimer =
            null;

    }

    if (
        !selectedTestEndpoint
    ) {

        console.warn(
            "No endpoint selected for stop."
        );

        return;

    }

    if (
        activeRequestNumber ===
        null ||
        activeRequestNumber ===
        undefined
    ) {

        console.warn(
            "No active request number available to stop."
        );

        return;

    }

    if (
        selectedTestEndpoint.id ===
        undefined ||
        selectedTestEndpoint.id ===
        null
    ) {

        console.warn(
            "No backend endpoint ID available to stop."
        );

        return;

    }

    const requestNumberToStop =
        activeRequestNumber;

    try {

        const result =
            await window.EdmsAPI.stopTest(
                selectedTestEndpoint.id,
                requestNumberToStop
            );

        console.log(
            "Stop request result:",
            result
        );

        if (!result.ok) {

            console.warn(
                "Backend rejected stop request:",
                result
            );

        }

    } catch (error) {

        console.error(
            "Stop request failed:",
            error
        );

    }

    if (testWS) {

        try {

            testWS.close();

        } catch { }

        testWS =
            null;

    }

    setRunButtonState(
        false
    );

    if (responseContent) {

        responseContent.value =
            formatJSON({

                status:
                    "Stopped",

                message:
                    "Test stopped.",

                request_number:
                    requestNumberToStop

            });

    }

}

// ============================================================
// RUN BUTTON STATE
// ============================================================

function setRunButtonState(
    isRunning
) {

    if (!runButton) return;

    runButton.dataset.running = isRunning ? "true" : "false";

    const runIconContainer = document.getElementById("runIconContainer");
    const runText = document.getElementById("runText");

    if (isRunning) {
        runButton.className = "h-8 shrink-0 px-4 flex items-center gap-1.5 rounded-lg border border-red-500/40 bg-transparent text-red-400 font-semibold text-sm transition hover:bg-red-500/10 active:scale-95";

        if (runIconContainer) {
            runIconContainer.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="w-3.5 h-3.5"><rect x="4" y="4" width="16" height="16" rx="2"></rect></svg>`;
        }

        if (runText) {
            runText.textContent = "Stop";
        }
    } else {
        runButton.className = "h-8 shrink-0 px-4 flex items-center gap-1.5 rounded-lg bg-cyan-500 text-slate-950 font-semibold text-sm transition hover:bg-cyan-400 active:scale-95 shadow-md shadow-cyan-500/20";

        if (runIconContainer) {
            runIconContainer.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4"><polygon points="6 3 20 12 6 21 6 3"></polygon></svg>`;
        }

        if (runText) {
            runText.textContent = "Run";
        }
    }

}

// ============================================================
// STATUS
// ============================================================

function getStatusText(status) {

    const statusTexts = {

        200: "OK",
        201: "Created",
        202: "Accepted",
        204: "No Content",
        301: "Moved Permanently",
        302: "Found",
        400: "Bad Request",
        401: "Unauthorized",
        403: "Forbidden",
        404: "Not Found",
        409: "Conflict",
        422: "Validation Error",
        429: "Too Many Requests",
        500: "Server Error",
        502: "Bad Gateway",
        503: "Service Unavailable"

    };

    return (
        statusTexts[status] ||
        ""
    );

}

// ============================================================
// TAGS
// ============================================================

function setupTags() {

    if (addTagButton) {

        addTagButton.addEventListener(
            "click",
            event => {

                event.preventDefault();

                addTestTag();

            }
        );

    }

    if (tagInput) {

        tagInput.addEventListener(
            "keydown",
            event => {

                if (
                    event.key ===
                    "Enter"
                ) {

                    event.preventDefault();

                    addTestTag();

                }

            }
        );

    }

    if (annotationInput) {

        annotationInput.addEventListener(
            "input",
            () => {

                if (
                    selectedTestEndpoint
                ) {

                    selectedTestEndpoint.annotation =
                        annotationInput.value;

                }

            }
        );

    }

}

// ============================================================
// ADD TAG
// ============================================================
async function loadTagsAndQPsInto(endpoint) {

    const [tagsResult, qpsResult] = await Promise.allSettled([
        fetchEndpointTags(endpoint.id),
        fetchEndpointQPs(endpoint.id)
    ]);

    // Tags and QPs succeed or fail independently
    if (tagsResult.status === "fulfilled") {
        endpoint.tags = tagsResult.value;
    } else if (!Array.isArray(endpoint.tags)) {
        endpoint.tags = [];
    }

    if (qpsResult.status === "fulfilled" && qpsResult.value.length > 0) {
        const localQPs = Array.isArray(endpoint.qps) ? endpoint.qps : [];
        endpoint.qps = qpsResult.value.map(backendQp => {
            const localQp = localQPs.find(q => String(q.id) === String(backendQp.id));
            return {
                ...backendQp,
                request: localQp?.request || {},
                response: localQp?.response || {}
            };
        });
    } else if (!Array.isArray(endpoint.qps)) {
        endpoint.qps = [];
    }

    if (qpsResult.status === "rejected") {
        console.warn(`QPs unavailable for ${endpoint.id}:`, qpsResult.reason?.message);
    }
}
async function addTestTag() {

    if (!selectedTestEndpoint) {

        return;

    }

    const tag =
        tagInput
            ? tagInput.value.trim()
            : "";

    if (!tag) return;

    if (
        !Array.isArray(
            selectedTestEndpoint.tags
        )
    ) {

        selectedTestEndpoint.tags =
            [];

    }

    if (
        selectedTestEndpoint.tags.includes(
            tag
        )
    ) {

        if (tagInput) {

            tagInput.value =
                "";

        }

        return;

    }

    try {

        /*
         * Persist through the actual backend
         * endpoint-tags API first.
         */

        await addEndpointTag(
            selectedTestEndpoint.id,
            tag
        );

        /*
         * Update local state only after the
         * backend operation succeeds.
         */

        selectedTestEndpoint.tags.push(
            tag
        );

        const endpoint =
            findEndpoint(
                selectedTestEndpoint.id
            );

        if (endpoint) {

            endpoint.tags =
                selectedTestEndpoint.tags;

        }

        if (tagInput) {

            tagInput.value =
                "";

        }

        renderSelectedEndpointTags();

        applyTestFilters();

        console.log(
            "Endpoint tag added:",
            {
                endpointId:
                    selectedTestEndpoint.id,

                tag
            }
        );

    } catch (error) {

        console.error(
            "Failed to add endpoint tag:",
            error
        );

        window.alert(
            error?.message ||
            "Could not add tag."
        );

    }

}

// ============================================================
// REMOVE TAG
// ============================================================

async function removeTestTag(tag) {

    if (
        !selectedTestEndpoint ||
        selectedTestEndpoint.id === undefined ||
        selectedTestEndpoint.id === null
    ) {

        return;

    }

    try {

        await removeEndpointTag(
            selectedTestEndpoint.id,
            tag
        );

        selectedTestEndpoint.tags =
            (
                selectedTestEndpoint.tags ||
                []
            ).filter(
                existingTag =>
                    existingTag !== tag
            );

        const endpoint =
            findEndpoint(
                selectedTestEndpoint.id
            );

        if (endpoint) {

            endpoint.tags =
                selectedTestEndpoint.tags;

        }

        renderSelectedEndpointTags();

        applyTestFilters();

        console.log(
            "Endpoint tag removed:",
            {
                endpointId:
                    selectedTestEndpoint.id,

                tag
            }
        );

    } catch (error) {

        console.error(
            "Failed to remove endpoint tag:",
            error
        );

        window.alert(
            error?.message ||
            "Could not remove tag."
        );

    }

}

// ============================================================
// RENDER TAGS
// ============================================================

function renderSelectedEndpointTags() {

    if (!endpointTags) return;

    endpointTags.innerHTML =
        "";

    if (
        !selectedTestEndpoint ||
        !Array.isArray(
            selectedTestEndpoint.tags
        ) ||
        selectedTestEndpoint.tags.length === 0
    ) {

        endpointTags.innerHTML = `
            <span class="text-xs text-slate-500">
                No tags selected.
            </span>
        `;

        return;

    }

    selectedTestEndpoint.tags.forEach(
        tag => {

            const element =
                document.createElement(
                    "span"
                );

            element.className = `
                inline-flex
                items-center
                gap-1
                px-3
                py-1
                rounded-full
                bg-cyan-500/15
                text-cyan-300
                text-xs
                border
                border-cyan-500/20
            `;

            const text =
                document.createElement(
                    "span"
                );

            text.textContent =
                tag;

            const removeButton =
                document.createElement(
                    "button"
                );

            removeButton.type =
                "button";

            removeButton.className = `
                ml-1
                text-cyan-400
                hover:text-white
                text-sm
                leading-none
            `;

            removeButton.textContent =
                "×";

            removeButton.title =
                "Remove tag";

            removeButton.addEventListener(
                "click",
                event => {

                    event.preventDefault();
                    event.stopPropagation();

                    removeTestTag(
                        tag
                    );

                }
            );

            element.appendChild(
                text
            );

            element.appendChild(
                removeButton
            );

            endpointTags.appendChild(
                element
            );

        }
    );

}

// ============================================================
// TABS
// ============================================================

function setupTabs() {

    setupContentTab(
        "requestHeadersTab",
        "request",
        "headers"
    );

    setupContentTab(
        "requestBodyTab",
        "request",
        "body"
    );

    setupContentTab(
        "responseHeadersTab",
        "response",
        "headers"
    );

    setupContentTab(
        "responseBodyTab",
        "response",
        "body"
    );

    updateContentTabButtons(
        "request",
        "body"
    );

    updateContentTabButtons(
        "response",
        "body"
    );
}

// ============================================================
// CONTENT TAB
// ============================================================

function setupContentTab(
    buttonId,
    panel,
    tab
) {

    const button =
        document.getElementById(
            buttonId
        );

    if (!button) return;

    button.addEventListener(
        "click",
        event => {

            event.preventDefault();

            if (
                panel ===
                "request"
            ) {

                activeRequestTab =
                    tab;

                renderCurrentRequest();

            } else {

                activeResponseTab =
                    tab;

                renderCurrentResponse();

            }

            updateContentTabButtons(
                panel,
                tab
            );

        }
    );

}

// ============================================================
// CONTENT TAB BUTTONS
// ============================================================

function updateContentTabButtons(
    panel,
    activeTab
) {

    const ids =
        panel ===
            "request"

            ? {
                headers:
                    "requestHeadersTab",

                body:
                    "requestBodyTab"
            }

            : {
                headers:
                    "responseHeadersTab",

                body:
                    "responseBodyTab"
            };

    Object.entries(ids).forEach(
        ([tab, id]) => {

            const button =
                document.getElementById(
                    id
                );

            if (!button) return;

            const active =
                tab ===
                activeTab;

            button.classList.toggle(
                "bg-cyan-500",
                active
            );

            button.classList.toggle(
                "text-slate-950",
                active
            );

            button.classList.toggle(
                "font-semibold",
                active
            );

            button.classList.toggle(
                "bg-slate-800",
                !active
            );

            button.classList.toggle(
                "text-slate-300",
                !active
            );

        }
    );

}

// ============================================================
// PANEL CONTROLS
// ============================================================

function setupPanelControls() {

    setupPanelControlSet(
        "request",
        requestBox,
        responseBox
    );

    setupPanelControlSet(
        "response",
        responseBox,
        requestBox
    );

}

function setupPanelControlSet(
    panelName,
    panel,
    siblingPanel
) {

    if (!panel) return;

    const closeButton =
        document.getElementById(
            `${panelName}Close`
        );

    const fullscreenButton =
        document.getElementById(
            `${panelName}Fullscreen`
        );

    const resetButton =
        document.getElementById(
            `${panelName}Reset`
        );

    if (closeButton) {

        closeButton.addEventListener(
            "click",
            event => {

                event.preventDefault();

                collapsePanel(
                    panel
                );

            }
        );

    }

    if (fullscreenButton) {

        fullscreenButton.addEventListener(
            "click",
            event => {

                event.preventDefault();

                togglePanelFullscreen(
                    panel,
                    siblingPanel
                );

            }
        );

    }

    if (resetButton) {

        resetButton.addEventListener(
            "click",
            event => {

                event.preventDefault();

                resetPanels();

            }
        );

    }

}

// ============================================================
// PANEL COLLAPSE
// ============================================================

function collapsePanel(panel) {

    panel.dataset.closed =
        "true";

    Array.from(
        panel.children
    ).forEach(
        child => {

            if (
                !child.classList.contains(
                    "panelheader"
                )
            ) {

                child.style.display =
                    "none";

            }

        }
    );

}

// ============================================================
// FULLSCREEN
// ============================================================

function togglePanelFullscreen(
    panel,
    siblingPanel
) {

    const isFullscreen =
        panel.dataset.fullscreen ===
        "true";

    resetPanels();

    if (isFullscreen) return;

    panel.dataset.fullscreen =
        "true";

    panel.style.position =
        "fixed";

    panel.style.inset =
        "16px";

    panel.style.zIndex =
        "50";

    panel.style.height =
        "auto";

    panel.style.maxHeight =
        "calc(100vh - 32px)";

    panel.style.display =
        "flex";

    if (siblingPanel) {

        siblingPanel.style.display =
            "none";

    }

}

// ============================================================
// RESET PANELS
// ============================================================

function resetPanels() {

    [
        requestBox,
        responseBox
    ].forEach(
        panel => {

            if (!panel) return;

            panel.dataset.fullscreen =
                "false";

            panel.dataset.closed =
                "false";

            panel.removeAttribute(
                "style"
            );

            Array.from(
                panel.children
            ).forEach(
                child => {

                    child.style.display =
                        "";

                }
            );

        }
    );

}

// ============================================================
// SIDEBAR TABS
// ============================================================

function setupSidebarTabs() {

    setupSidebarTab(
        "historyTab",
        "history"
    );



    setupSidebarTab(
        "endpointsTab",
        "endpoints"
    );

    updateSidebarTabButtons();

}

function setupSidebarTab(
    buttonId,
    tab
) {

    const button =
        document.getElementById(
            buttonId
        );

    if (!button) return;

    button.addEventListener(
        "click",
        event => {

            event.preventDefault();

            activeSidebarTab =
                tab;

            selectedTestEndpoint =
                null;

            selectedTestQP =
                null;

            clearRequestResponse();

            updateSidebarTabButtons();

            applyTestFilters();

        }
    );

}

// ============================================================
// SIDEBAR TAB BUTTONS
// ============================================================
function updateSidebarTabButtons() {

    const tabs = {

        historyTab:
            "history",

        endpointsTab:
            "endpoints"

    };

    Object.entries(tabs).forEach(
        ([id, tab]) => {

            const button =
                document.getElementById(
                    id
                );

            if (!button) return;

            const active =
                activeSidebarTab ===
                tab;

            button.classList.toggle(
                "text-sky-400",
                active
            );

            button.classList.toggle(
                "border-sky-500",
                active
            );

            button.classList.toggle(
                "text-slate-400",
                !active
            );

        }
    );

    const historyMenuButton =
        document.getElementById(
            "historyMenuButton"
        );

    if (historyMenuButton) {

        historyMenuButton.classList.toggle(
            "hidden",
            activeSidebarTab !== "history"
        );

    }

    const historyMenu =
        document.getElementById(
            "historyMenu"
        );

    if (
        historyMenu &&
        activeSidebarTab !== "history"
    ) {

        historyMenu.classList.add(
            "hidden"
        );

    }

    requestAnimationFrame(
        updateSidebarTabUnderline
    );

}
// ============================================================
// SIDEBAR TAB UNDERLINE
// ============================================================

function updateSidebarTabUnderline() {

    const tabs = [

        document.getElementById(
            "historyTab"
        ),



        document.getElementById(
            "endpointsTab"
        )

    ].filter(Boolean);

    if (
        tabs.length === 0
    ) {
        return;
    }

    const activeTab =
        tabs.find(
            tab => {

                if (
                    tab.id ===
                    "historyTab"
                ) {

                    return (
                        activeSidebarTab ===
                        "history"
                    );

                }

                return (
                    activeSidebarTab ===
                    "endpoints"
                );

            }
        );

    if (!activeTab) {
        return;
    }

    const parent =
        activeTab.parentElement;

    if (!parent) {
        return;
    }

    if (
        getComputedStyle(parent)
            .position ===
        "static"
    ) {

        parent.style.position =
            "relative";

    }

    let underline =
        document.getElementById(
            "testSidebarTabUnderline"
        );

    if (!underline) {

        underline =
            document.createElement(
                "div"
            );

        underline.id =
            "testSidebarTabUnderline";

        underline.className = `
            absolute
            bottom-0
            h-0.5
            rounded-full
            bg-sky-400
            transition-all
            duration-200
            ease-out
            pointer-events-none
        `;

        parent.appendChild(
            underline
        );

    }

    const parentRect =
        parent.getBoundingClientRect();

    const activeRect =
        activeTab.getBoundingClientRect();

    underline.style.width =
        `${activeRect.width}px`;

    underline.style.transform =
        `translateX(${activeRect.left -
        parentRect.left
        }px)`;

}

// ============================================================
// SIDEBAR UNDERLINE ON RESIZE
// ============================================================

window.addEventListener(
    "resize",
    () => {

        requestAnimationFrame(
            updateSidebarTabUnderline
        );

    }
);

// ============================================================
// SIDEBAR COLLAPSE
// ============================================================

function setupSidebarCollapse() {

    if (
        !sidebarCollapseToggle ||
        !testSidebar
    ) return;

    const sidebarContent =
        document.getElementById(
            "sidebarContent"
        );

    sidebarCollapseToggle.addEventListener(
        "click",
        event => {

            event.preventDefault();

            const collapsed =
                testSidebar.classList.toggle(
                    "sidebar-collapsed"
                );

            if (sidebarContent) {

                sidebarContent.classList.toggle(
                    "hidden",
                    collapsed
                );

            }

        }
    );

}

// ============================================================
// ADDRESS MODE
// ============================================================

let addressCombinedMode =
    false;

function setupAddressMode() {

    if (
        !addressModeToggle ||
        !addressSplit ||
        !urlFullInput
    ) return;

    addressModeToggle.addEventListener(
        "click",
        event => {

            event.preventDefault();

            addressCombinedMode =
                !addressCombinedMode;

            if (
                addressCombinedMode
            ) {

                syncAddressFullDisplay();

                addressSplit.classList.add(
                    "hidden"
                );

                urlFullInput.classList.remove(
                    "hidden"
                );

            } else {

                applyFullAddressToSplit();

                addressSplit.classList.remove(
                    "hidden"
                );

                urlFullInput.classList.add(
                    "hidden"
                );

            }

        }
    );

    urlFullInput.addEventListener(
        "input",
        applyFullAddressToSplit
    );

}

// ============================================================
// ADDRESS HELPERS
// ============================================================
function normalizeAddress(value) {
    return String(value || "")
        .trim()
        .replace(/^["']+|["']+$/g, "")   // NEW: strip wrapping quotes
        .replace(/\/+$/, "")
        .replace(/^([a-z][a-z0-9+.-]*:\/\/[^\/?#]*)/i, m => m.toLowerCase());
}
function splitAddress(value) {
    value = String(value || "").trim();
    if (!value) return { prefix: "", path: "" };

    // optional scheme (any case, any scheme) + host[:port], up to the first / ? or #
    const match = value.match(/^((?:[a-z][a-z0-9+.-]*:\/\/)?[^\/?#]*)(.*)$/i);

    return {
        prefix: match[1],
        path: match[2]
    };
}

function joinAddressParts(prefix, path) {
    prefix = String(prefix || "").trim().replace(/\/+$/, "");
    path = String(path || "").trim();

    if (!prefix) return path;
    if (!path) return prefix;

    // Path already contains the full prefix -> don't repeat it.
    // The character after the prefix must be a boundary, so
    // "localhost:80" doesn't wrongly match "localhost:8080/x".
    if (
        path.toLowerCase().startsWith(prefix.toLowerCase()) &&
        /^([\/?#]|$)/.test(path.slice(prefix.length))
    ) {
        return path;
    }

    // Query or fragment attaches directly, no "/" inserted
    if (/^[?#]/.test(path)) return prefix + path;

    return prefix + "/" + path.replace(/^\/+/, "");
}
function syncAddressFullDisplay() {

    if (!urlFullInput) return;

    const prefix =
        baseUrl
            ? baseUrl.value.trim()
            : "";

    const path =
        endpointPath
            ? endpointPath.value.trim()
            : "";

    urlFullInput.value =
        joinAddressParts(
            prefix,
            path
        );

}

function applyFullAddressToSplit() {

    if (!urlFullInput) return;

    const value =
        urlFullInput.value.trim();

    const {
        prefix,
        path
    } = splitAddress(value);

    if (baseUrl) {
        baseUrl.value = prefix;
    }

    if (endpointPath) {
        endpointPath.value = path;
    }

}
// ============================================================
// HELPERS
// ============================================================

function findEndpoint(
    endpointId
) {

    return endpoints.find(
        endpoint =>
            String(endpoint.id) ===
            String(endpointId)
    );

}

function getBookmarkEndpointId(
    bookmark
) {

    if (
        bookmark === undefined ||
        bookmark === null
    ) {

        return null;

    }

    if (
        typeof bookmark !==
        "object"
    ) {

        return bookmark;

    }

    return (
        bookmark.endpointId ??
        bookmark.endpoint_id ??
        bookmark.id ??
        null
    );

}

function activeTestEndpointIdFromState() {

    if (
        selectedTestEndpoint?.id !==
        undefined &&
        selectedTestEndpoint?.id !==
        null
    ) {

        return selectedTestEndpoint.id;

    }

    return null;

}

function isEndpointBookmarked(
    endpointId
) {

    if (
        endpointId ===
        undefined ||
        endpointId ===
        null
    ) {

        return false;
    }

    return bookmarks.some(
        bookmark =>
            String(
                getBookmarkEndpointId(
                    bookmark
                )
            ) ===
            String(endpointId)
    );

}

function formatEndpointDate(
    dateString
) {

    if (!dateString) return "";

    const date =
        new Date(dateString);

    if (
        Number.isNaN(
            date.getTime()
        )
    ) {

        return dateString;

    }

    const now =
        new Date();

    if (
        date.toDateString() ===
        now.toDateString()
    ) {

        return date.toLocaleTimeString(
            "en-US",
            {
                hour:
                    "numeric",

                minute:
                    "2-digit"
            }
        );

    }

    return date.toLocaleDateString(
        "en-GB",
        {
            day:
                "2-digit",

            month:
                "2-digit"
        }
    );

}

function escapeHTML(
    value
) {

    return String(
        value ?? ""
    )
        .replace(
            /&/g,
            "&amp;"
        )
        .replace(
            /</g,
            "&lt;"
        )
        .replace(
            />/g,
            "&gt;"
        )
        .replace(
            /"/g,
            "&quot;"
        )
        .replace(
            /'/g,
            "&#039;"
        );

}