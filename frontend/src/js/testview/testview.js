// =========================================
// EDMS TEST VIEW
// =========================================

let endpoints = [];
let bookmarks = [];
let historyRecords = [];
let filteredTestEndpoints = [];

let selectedTestEndpoint = null;
let selectedTestQP = null;

let selectedQPIds = new Set();

let activeSidebarTab = "endpoints";
let activeTestMethod = "ALL";
let activeTimeFilter = "all";

let activeRequestTab = "headers";
let activeResponseTab = "headers";

let latestResponseMeta = null;

let runTimer = null;

let testWS = null;
let endpointWS = null;
let bookmarkWS = null;
let historyWS = null;

let testRunStartedAt = null;
let activeRequestNumber = null;

const LOCAL_QP_KEY = "edmsTestViewQPs";

const API_BASE = "http://localhost:3000";

// =========================================
// DOM
// =========================================

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

const stopButton =
document.getElementById("stopRequest");

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

// =========================================
// INIT
// =========================================

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

    updateMethodButtons();
    applyTestFilters();

}

// =========================================
// LOAD DATA
// =========================================

async function loadTestData() {

    // Backend is the source of truth for:
    // - Endpoints
    // - Active bookmarks
    // - History

    await loadEndpointsFromBackend();

    await loadBookmarksFromBackend();

    await loadHistoryFromBackend();

    // QPs are still local because the backend
    // currently has no QP API.
    restoreLocalQPs();

}

// =========================================
// BACKEND ENDPOINT SNAPSHOT
// =========================================

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

                            finished = true;

                            console.log(
                                `Loaded ${endpoints.length} endpoints from backend.`
                            );

                            resolve();

                            try {
                                ws.close();
                            } catch {}

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

// =========================================
// NORMALIZE ENDPOINT
// =========================================

function normalizeBackendEndpoint(endpoint) {

    const id =
        endpoint.id ??
        endpoint.endpoint_id;

    const fullURL =
        endpoint.endpoint ??
        endpoint.endpoint_str ??
        endpoint.url ??
        "";

    let parsedURL = null;

    try {

        parsedURL =
            new URL(fullURL);

    } catch {
        // Keep original URL.
    }

    return {

        ...endpoint,

        id,

        method:
            String(
                endpoint.method ||
                "GET"
            ).toUpperCase(),

        endpoint:
            parsedURL
                ? parsedURL.pathname +
                  parsedURL.search
                : fullURL,

        baseUrl:
            endpoint.baseUrl ||
            endpoint.base_url ||
            (
                parsedURL
                    ? parsedURL.origin
                    : ""
            ),

        endpoint_str:
            endpoint.endpoint_str ||
            fullURL,

        qps:
            Array.isArray(endpoint.qps)
                ? endpoint.qps
                : [],

        tags:
            Array.isArray(endpoint.tags)
                ? endpoint.tags
                : []

    };

}

// =========================================
// BACKEND BOOKMARK SNAPSHOT
// =========================================

function loadBookmarksFromBackend() {

    return new Promise(
        (resolve, reject) => {

            const ws =
                window.EdmsAPI
                    .connectBookmarkLoader();

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

                            finished = true;

                            console.log(
                                `Loaded ${bookmarks.length} active bookmarks from backend.`
                            );

                            resolve();

                            try {
                                ws.close();
                            } catch {}

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

// =========================================
// BACKEND HISTORY SNAPSHOT
// =========================================

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

                            finished = true;

                            console.log(
                                `Loaded ${historyRecords.length} history records from backend.`
                            );

                            resolve();

                            try {
                                ws.close();
                            } catch {}

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

// =========================================
// NORMALIZE HISTORY
// =========================================

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

// =========================================
// LOCAL QP STORAGE
// =========================================

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

// =========================================
// SIDEBAR ITEMS
// =========================================

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

// =========================================
// RENDER SIDEBAR
// =========================================

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

    filteredTestEndpoints.forEach(
        item => {

            testEndpointList.appendChild(
                createTestEndpointCard(item)
            );

        }
    );

}

// =========================================
// ENDPOINT CARD
// =========================================

function createTestEndpointCard(item) {

    const card =
        document.createElement("div");

    const endpoint =
        item.endpointRef ||
        item.source ||
        item;

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
        item.id ||
        endpoint.id;

    card.dataset.endpointId =
        item.endpointId ||
        endpoint.id;

    card.innerHTML = `

        <div class="flex items-center justify-between">

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
    `;

    card.addEventListener(
        "click",
        () => {

            selectSidebarItem(
                item,
                card
            );

        }
    );

    if (
        item.type ===
        "history"
    ) {

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

// =========================================
// HISTORY CONTEXT MENU
// =========================================

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

// =========================================
// HISTORY → ACTIVE COLLECTION
// =========================================

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

// =========================================
// SAVE SELECTED ENDPOINT → ACTIVE COLLECTION
// =========================================

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

// =========================================
// SAVE ENDPOINT → ACTIVE COLLECTION
// =========================================

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

        // First add the endpoint to the active
        // Bookmark View workspace.
        await window.EdmsAPI.addActiveBookmark(
            endpointId
        );

        // Then persist the active bookmark into
        // the currently loaded collection.
        const result =
            await window.EdmsAPI.saveActiveBookmark(
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

// =========================================
// CLEAR ALL HISTORY
// =========================================

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

// =========================================
// METHOD COLOR
// =========================================

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

// =========================================
// BADGE
// =========================================

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

    return item.saved
        ? "Added - Bookmarked"
        : "Added";

}

// =========================================
// QP COUNT
// =========================================

function getQPCountLabel(endpoint) {

    const count =
        Array.isArray(endpoint?.qps)
            ? endpoint.qps.length
            : 0;

    return `${count} QP${count === 1 ? "" : "s"}`;

}

// =========================================
// SELECT SIDEBAR ITEM
// =========================================

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

// =========================================
// SELECT ENDPOINT
// =========================================

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

    renderSelectedEndpointTags();

    renderTestQP(
        endpoint,
        preferredQPId
    );

}

// =========================================
// RENDER QP
// =========================================

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

// =========================================
// QP MENU
// =========================================

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

// =========================================
// QP MENU ACTION
// =========================================

function handleQPMenuAction(action) {

    switch (action) {

        case "create":
            doCreateQP();
            break;

        case "select-all":
            doSelectAllQP();
            break;

        case "clear-selection":
            doClearSelectionQP();
            break;

        case "delete-selected":
            doDeleteSelectedQP();
            break;

    }

}

// =========================================
// CREATE QP
// =========================================

function doCreateQP() {

    if (!selectedTestEndpoint) {

        console.warn(
            "Select an endpoint first."
        );

        return;
    }

    const name =
        window.prompt(
            "QP name:"
        );

    if (
        !name ||
        !name.trim()
    ) return;

    if (
        !Array.isArray(
            selectedTestEndpoint.qps
        )
    ) {

        selectedTestEndpoint.qps =
            [];

    }

    const nextId =
        selectedTestEndpoint.qps.reduce(
            (max, qp) =>
                Math.max(
                    max,
                    Number(qp.id) || 0
                ),
            0
        ) + 1;

    selectedTestEndpoint.qps.push({

        id:
            nextId,

        name:
            name.trim(),

        request: {

            headers: {},

            body: {}

        },

        response: {}

    });

    saveLocalQPs();

    renderTestQP(
        selectedTestEndpoint,
        nextId
    );

    applyTestFilters();

}

// =========================================
// SELECT ALL QP
// =========================================

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

// =========================================
// CLEAR QP SELECTION
// =========================================

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

// =========================================
// DELETE SELECTED QP
// =========================================

function doDeleteSelectedQP() {

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

// =========================================
// SELECT QP
// =========================================

function selectTestQP(
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

}

// =========================================
// REQUEST
// =========================================

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

// =========================================
// RESPONSE
// =========================================

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

// =========================================
// REQUEST BODY
// =========================================

function getRequestBodyPreview(request) {

    if (
        request.body !== undefined
    ) {

        return request.body;
    }

    return {};

}

// =========================================
// RESPONSE HEADERS
// =========================================

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

// =========================================
// CLEAR
// =========================================

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

// =========================================
// FORMAT JSON
// =========================================

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

// =========================================
// SEARCH
// =========================================

function setupTestSearch() {

    if (!testSearchInput) return;

    testSearchInput.addEventListener(
        "input",
        applyTestFilters
    );

}

// =========================================
// METHOD FILTER
// =========================================

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

// =========================================
// METHOD BUTTONS
// =========================================

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

// =========================================
// TIME FILTER
// =========================================

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

// =========================================
// URL FILTER
// =========================================

function setupURLFilter() {

    if (!urlFilter) return;

    urlFilter.addEventListener(
        "input",
        applyTestFilters
    );

}

// =========================================
// FILTER
// =========================================

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

// =========================================
// TIME MATCH
// =========================================

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

// =========================================
// RUNNER
// =========================================

function setupRunner() {

    if (runButton) {

        runButton.type =
            "button";

        runButton.addEventListener(
            "click",
            event => {

                event.preventDefault();
                event.stopPropagation();

                runTestEndpoint();

            }
        );

    }

    if (stopButton) {

        stopButton.type =
            "button";

        stopButton.addEventListener(
            "click",
            event => {

                event.preventDefault();
                event.stopPropagation();

                stopTestEndpoint();

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

// =========================================
// RUN TEST
// =========================================

// =========================================
// RUN TEST
// =========================================

async function runTestEndpoint() {

    /*
     * New endpoint flow:
     *
     * 1. If an endpoint is selected, retest that endpoint.
     * 2. If nothing is selected, use the URL/method fields directly.
     * 3. Send endpoint_id as undefined.
     * 4. Backend automatically creates the endpoint.
     * 5. Resolve the newly created endpoint_id.
     * 6. Fetch request/response using that ID.
     */

    // -------------------------------------
    // METHOD
    // -------------------------------------

    const method =
        String(
            testMethod?.value ||
            selectedTestEndpoint?.method ||
            "GET"
        ).toUpperCase();

    // -------------------------------------
    // ENDPOINT URL
    // -------------------------------------

    let endpointStr = "";

    if (selectedTestEndpoint) {

        endpointStr =
            selectedTestEndpoint.endpoint_str ||
            (
                selectedTestEndpoint.baseUrl ||
                ""
            ) +
            (
                selectedTestEndpoint.endpoint ||
                ""
            );

    } else {

        /*
         * No endpoint selected.
         * Build the endpoint directly from
         * the URL fields entered by the user.
         */

        if (addressCombinedMode && urlFullInput) {

            endpointStr =
                urlFullInput.value.trim();

        } else {

            endpointStr =
                (
                    baseUrl?.value.trim() ||
                    ""
                ) +
                (
                    endpointPath?.value.trim() ||
                    ""
                );

        }

    }

    if (!endpointStr) {

        window.alert(
            "Enter an endpoint URL first."
        );

        return;
    }

    // -------------------------------------
    // CREATE TEMPORARY ENDPOINT STATE
    // -------------------------------------

    /*
     * When no endpoint exists yet, create a
     * temporary frontend representation.
     *
     * IMPORTANT:
     * id stays undefined.
     *
     * The backend will create the real endpoint.
     */

    if (!selectedTestEndpoint) {

        selectedTestEndpoint = {

            id:
                undefined,

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

    }

    // -------------------------------------
    // CREATE DEFAULT QP IF NEEDED
    // -------------------------------------

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
        // START LISTENER BEFORE SENDING
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
        // REQUEST
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
        // ENDPOINT ID
        // -------------------------------------

        /*
         * Existing endpoint:
         *     send its ID
         *
         * New endpoint:
         *     send undefined
         *
         * The backend will automatically create
         * the endpoint in the second case.
         */

        const endpointId =
            selectedTestEndpoint.id;

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

        // -------------------------------------
        // SEND TO BACKEND
        // -------------------------------------

        window.EdmsAPI.startTest(
            ws,

            endpointId,

            endpointStr,

            method,

            requestJson,

            30000,

            500,

            request.headers,

            selectedTestEndpoint.annotation
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

        /*
         * If the backend did not include endpoint_id
         * in the finished event, reload the endpoint
         * snapshot and find the endpoint by URL.
         */

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
                    endpoint =>
                        String(
                            endpoint.endpoint_str ||
                            ""
                        ) ===
                        String(
                            endpointStr
                        )
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
        // USE REAL BACKEND ENDPOINT
        // -------------------------------------

        let backendEndpoint =
            findEndpoint(
                resolvedEndpointId
            );

        /*
         * If the endpoint wasn't already present in
         * the current snapshot, reload once more.
         */

        if (!backendEndpoint) {

            await loadEndpointsFromBackend();

            backendEndpoint =
                findEndpoint(
                    resolvedEndpointId
                );

        }

        if (backendEndpoint) {

            /*
             * Preserve the local QP because QPs are
             * currently frontend-local.
             */

            const localQP =
                selectedTestQP;

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

            /*
             * Reuse the temporary/default QP.
             */

            if (
                selectedTestEndpoint.qps.length ===
                0
            ) {

                selectedTestEndpoint.qps.push(
                    localQP
                );

            }

            selectedTestQP =
                localQP;

        } else {

            /*
             * Backend endpoint snapshot could not be
             * resolved, but we do have its ID.
             */

            selectedTestEndpoint.id =
                resolvedEndpointId;

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
        // LOAD SAVED REQUEST / RESPONSE
        // -------------------------------------

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

        console.log(
            "Saved request:",
            requestResult
        );

        console.log(
            "Saved response:",
            responseResult
        );

        // -------------------------------------
        // REQUEST
        // -------------------------------------

        if (selectedTestQP) {

            selectedTestQP.request =
                selectedTestQP.request ||
                {};

            const savedRequest =
                requestResult?.data;

            if (
                savedRequest?.body !==
                undefined
            ) {

                selectedTestQP.request.body =
                    savedRequest.body;

            } else if (
                savedRequest !==
                undefined
            ) {

                selectedTestQP.request.body =
                    savedRequest;

            }

            if (
                savedRequest?.headers
            ) {

                selectedTestQP.request.headers =
                    savedRequest.headers;

            }

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
            event.response_time_ms ??
            event.elapsed_ms ??
            Math.round(
                performance.now() -
                testRunStartedAt
            );

        // -------------------------------------
        // RESPONSE
        // -------------------------------------

        if (selectedTestQP) {

            selectedTestQP.response =
                selectedTestQP.response ||
                {};

            const savedResponse =
                responseResult?.data;

            if (
                savedResponse?.body !==
                undefined
            ) {

                selectedTestQP.response.body =
                    savedResponse.body;

            } else if (
                savedResponse !==
                undefined
            ) {

                selectedTestQP.response.body =
                    savedResponse;

            }

            selectedTestQP.response.status =
                statusCode;

            if (
                savedResponse?.headers
            ) {

                selectedTestQP.response.headers =
                    savedResponse.headers;

            }

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
        // DISPLAY RESULT
        // -------------------------------------

        renderCurrentRequest();

        renderCurrentResponse();

        /*
         * Make sure the newly created endpoint
         * appears in the sidebar.
         */

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

            } catch {}

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

// =========================================
// WAIT FOR WS OPEN
// =========================================

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

// =========================================
// TEST STARTED
// =========================================

function handleTestStarted(event) {

    console.log(
        "TestStarted:",
        event
    );

    const requestNumber =
        event.payload?.request_number ??
        event.request_number;

    if (
        requestNumber !==
            undefined &&
        requestNumber !==
            null
    ) {

        activeRequestNumber =
            requestNumber;

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

// =========================================
// TIMER TICK
// =========================================

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

// =========================================
// TEST FINISHED
// =========================================

function handleTestFinished(event) {

    console.log(
        "TestFinished:",
        event
    );

    const requestNumber =
        event.payload?.request_number ??
        event.request_number;

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

// =========================================
// TEST TIMEOUT
// =========================================

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

// =========================================
// TEST ERROR
// =========================================

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

// =========================================
// STOP
// =========================================

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

        } catch {}

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

// =========================================
// RUN BUTTON STATE
// =========================================

function setRunButtonState(
    isRunning
) {

    if (!runButton) return;

    runButton.disabled =
        isRunning;

    runButton.classList.toggle(
        "opacity-50",
        isRunning
    );

    runButton.classList.toggle(
        "cursor-not-allowed",
        isRunning
    );

}

// =========================================
// STATUS
// =========================================

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

// =========================================
// TAGS
// =========================================

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

// =========================================
// ADD TAG
// =========================================

function addTestTag() {

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
        !selectedTestEndpoint.tags.includes(
            tag
        )
    ) {

        selectedTestEndpoint.tags.push(
            tag
        );

    }

    if (tagInput) {

        tagInput.value =
            "";

    }

    renderSelectedEndpointTags();

}

// =========================================
// RENDER TAGS
// =========================================

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
                px-3 py-1 rounded-full
                bg-cyan-500/15
                text-cyan-300
                text-xs
                border
                border-cyan-500/20
            `;

            element.textContent =
                tag;

            endpointTags.appendChild(
                element
            );

        }
    );

}

// =========================================
// TABS
// =========================================

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
        "headers"
    );

    updateContentTabButtons(
        "response",
        "headers"
    );

}

// =========================================
// CONTENT TAB
// =========================================

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

// =========================================
// CONTENT TAB BUTTONS
// =========================================

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

// =========================================
// PANEL CONTROLS
// =========================================

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

// =========================================
// PANEL COLLAPSE
// =========================================

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

// =========================================
// FULLSCREEN
// =========================================

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

// =========================================
// RESET PANELS
// =========================================

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

// =========================================
// SIDEBAR TABS
// =========================================

function setupSidebarTabs() {

    setupSidebarTab(
        "historyTab",
        "history"
    );

    setupSidebarTab(
        "bookmarksTab",
        "bookmarks"
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

function updateSidebarTabButtons() {

    const tabs = {

        historyTab:
            "history",

        bookmarksTab:
            "bookmarks",

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

}

// =========================================
// SIDEBAR COLLAPSE
// =========================================

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

// =========================================
// ADDRESS MODE
// =========================================

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

// =========================================
// ADDRESS HELPERS
// =========================================

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
        `${prefix}${path}`;

}

function applyFullAddressToSplit() {

    if (!urlFullInput) return;

    const value =
        urlFullInput.value.trim();

    const match =
        value.match(
            /^(https?:\/\/[^/]+)(\/.*)?$/i
        );

    if (match) {

        if (baseUrl) {

            baseUrl.value =
                match[1];

        }

        if (endpointPath) {

            endpointPath.value =
                match[2] ||
                "";

        }

    } else if (endpointPath) {

        endpointPath.value =
            value;

    }

}

// =========================================
// HELPERS
// =========================================

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

    return (
        bookmark.endpointId ??
        bookmark.endpoint_id
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