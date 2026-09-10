// ============================================================
// EDMS BOOKMARK VIEW
// ============================================================


// ============================================================
// STATE
// ============================================================

let endpoints = [];
let filteredEndpoints = [];

let selectedEndpoint = null;
let selectedQP = null;

const selectedEndpointIds = new Set();

let currentPage = 1;

// 50 endpoints per page
const PAGE_SIZE = 50;

let sidebarCollapsed = false;

let activeContextEndpoint = null;
let activeContextQP = null;


// ============================================================
// FILTER STATE
// ============================================================

const activeFilters = {

    search: "",

    crud: "All CRUD",

    tags: new Set(),

    segments: new Set()

};


// ============================================================
// DOM
// ============================================================

const tableBody =
    document.getElementById("endpointTable");

const qpPanel =
    document.getElementById("qpPanel");

const deleteQPButton =
    document.getElementById("deleteQP");

const addQPButton =
    document.getElementById("addQP");

const requestEndpoint =
    document.getElementById("requestEndpoint");

const requestHeaders =
    document.getElementById("requestHeaders");

const requestQuery =
    document.getElementById("requestQuery");

const requestPath =
    document.getElementById("requestPath");

const requestBody =
    document.getElementById("requestBody");

const responseStatus =
    document.getElementById("responseStatus");

const responseTime =
    document.getElementById("responseTime");

const responsePayload =
    document.getElementById("responsePayload");

const responseServer =
    document.getElementById("responseServer");

const responseHeaders =
    document.getElementById("responseHeaders");

const responseBody =
    document.getElementById("responseBody");

const detailsOverlay =
    document.getElementById("detailsOverlay");

const detailsPanel =
    document.getElementById("detailsPanel");

const detailsClose =
    document.getElementById("detailsClose");

const detailsFullscreen =
    document.getElementById("detailsFullscreen");

const detailsFullscreenIcon =
    document.getElementById("detailsFullscreenIcon");

const contextMenu =
    document.getElementById("contextMenu");

const modalOverlay =
    document.getElementById("modalOverlay");

const modalTitle =
    document.getElementById("modalTitle");

const modalContent =
    document.getElementById("modalContent");

let detailsFullscreenActive = false;


// ============================================================
// INIT
// ============================================================

document.addEventListener(
    "DOMContentLoaded",
    init
);


async function init() {

    await loadData();

    TagManager.init({

        view: "list",

        getItems: () => endpoints,

        getTags: endpoint => {
            return Array.isArray(endpoint.tags)
                ? endpoint.tags
                : [];
        },

        setTags: (endpoint, tags) => {
            endpoint.tags = tags;
        },

        onChange: () => {
            renderTags();
            updateStats();
            applyFilters();
        }

    });

    

    setupFilters();

    setupSidebar();

    setupSelectionControls();

    setupModal();

    setupContextMenu();

    setupGlobalActions();

    setupDetailsViewer();

    makeColumnsResizable();

    renderTags();

    renderSegments();

    updateStats();

    renderTable();

    updateFooter();

    updateSelectionUI();

    

}


// ============================================================
// LOAD DATA
// ============================================================

async function loadData() {

    try {

        // ----------------------------------------------------
        // FIRST: load the ACTIVE BOOKMARK workspace
        // ----------------------------------------------------

        const bookmarkSnapshot =
            await loadBookmarksFromBackend();


        console.log(
            "BOOKMARK SNAPSHOT RECEIVED:",
            bookmarkSnapshot
        );


        const activeBookmarks =
            Array.isArray(
                bookmarkSnapshot?.bookmarks
            )
                ? bookmarkSnapshot.bookmarks
                : [];


        console.log(
            "ACTIVE COLLECTION:",
            bookmarkSnapshot?.active_collection
        );


        console.log(
            "ACTIVE BOOKMARKS:",
            activeBookmarks
        );


        // ----------------------------------------------------
        // If backend gave full endpoint data inside bookmarks,
        // use it directly.
        // ----------------------------------------------------

        endpoints =
            activeBookmarks
                .map(bookmark => {

                    const endpointId =
                        getBookmarkEndpointId(
                            bookmark
                        );


                    if (
                        endpointId === null ||
                        endpointId === undefined
                    ) {

                        console.warn(
                            "Bookmark has no endpoint ID:",
                            bookmark
                        );

                        return null;

                    }


                    /*
                     * Some backend responses may contain
                     * endpoint data directly in the bookmark.
                     *
                     * Keep whatever the backend actually gave us.
                     */

                    return {

                        ...(typeof bookmark === "object"
                            ? bookmark
                            : {}),

                        id:
                            bookmark?.id ??
                            bookmark?.endpoint_id ??
                            bookmark?.endpointId ??
                            endpointId,

                        endpoint_id:
                            endpointId,

                        endpoint:
                            bookmark?.endpoint_str ??
                            bookmark?.endpoint ??
                            "",

                        method:
                            bookmark?.method ??
                            "",

                        annotation:
                            bookmark?.annotation ??
                            "",

                        tags:
                            Array.isArray(
                                bookmark?.tags
                            )
                                ? bookmark.tags
                                : [],

                        qps:
                            Array.isArray(
                                bookmark?.qps
                            )
                                ? bookmark.qps
                                : [],

                        updated:
                            bookmark?.updated ??
                            null

                    };

                })
                .filter(Boolean);


        // ----------------------------------------------------
        // SECOND: only if bookmarks contain IDs but no endpoint
        // information, fetch the endpoint snapshot and merge it.
        // ----------------------------------------------------

        const needsEndpointData =
            endpoints.some(
                endpoint =>
                    !endpoint.endpoint ||
                    !endpoint.method
            );


        if (needsEndpointData) {

            console.log(
                "Bookmark data contains IDs only. Loading endpoint snapshot..."
            );


            const backendEndpoints =
                await loadEndpointsFromBackend();


            console.log(
                "ENDPOINT SNAPSHOT:",
                backendEndpoints
            );


            const endpointMap =
                new Map();


            backendEndpoints.forEach(
                backendEndpoint => {

                    const endpointId =
                        backendEndpoint?.id ??
                        backendEndpoint?.endpoint_id ??
                        backendEndpoint?.endpointId;


                    if (
                        endpointId !== null &&
                        endpointId !== undefined
                    ) {

                        endpointMap.set(
                            String(endpointId),
                            backendEndpoint
                        );

                    }

                }
            );


            endpoints =
                endpoints.map(
                    bookmark => {

                        const backendEndpoint =
                            endpointMap.get(
                                String(
                                    bookmark.endpoint_id
                                )
                            );


                        if (!backendEndpoint) {

                            console.warn(
                                "No endpoint data found for bookmark:",
                                bookmark
                            );

                            return bookmark;

                        }


                        return {

                            ...backendEndpoint,

                            ...bookmark,

                            id:
                                backendEndpoint.id ??
                                backendEndpoint.endpoint_id ??
                                bookmark.id,

                            endpoint_id:
                                bookmark.endpoint_id,

                            endpoint:
                                backendEndpoint.endpoint_str ??
                                backendEndpoint.endpoint ??
                                bookmark.endpoint ??
                                "",

                            method:
                                backendEndpoint.method ??
                                bookmark.method ??
                                "",

                            annotation:
                                backendEndpoint.annotation ??
                                bookmark.annotation ??
                                "",

                            tags:
                                Array.isArray(bookmark.tags) &&
                                bookmark.tags.length
                                    ? bookmark.tags
                                    : Array.isArray(
                                        backendEndpoint.tags
                                    )
                                        ? backendEndpoint.tags
                                        : [],

                            qps:
                                Array.isArray(bookmark.qps)
                                    ? bookmark.qps
                                    : Array.isArray(
                                        backendEndpoint.qps
                                    )
                                        ? backendEndpoint.qps
                                        : [],

                            updated:
                                backendEndpoint.updated ??
                                bookmark.updated ??
                                null

                        };

                    }
                );

        }


        filteredEndpoints =
            [...endpoints];


        console.log(
            "FINAL BOOKMARK VIEW ENDPOINTS:",
            endpoints
        );


    } catch (error) {

        console.error(
            "FAILED TO LOAD BOOKMARK VIEW:",
            error
        );


        endpoints = [];

        filteredEndpoints = [];


        showToast(
            "Unable to load bookmark data.",
            "error"
        );

    }

}
// ============================================================
// LOAD ENDPOINTS FROM BACKEND
// ============================================================

function loadBookmarksFromBackend() {

    return new Promise(
        (resolve, reject) => {

            if (
                !window.EdmsAPI ||
                typeof window.EdmsAPI.connectBookmarkLoader !==
                    "function"
            ) {

                reject(
                    new Error(
                        "Bookmark loader API is unavailable."
                    )
                );

                return;

            }


            const ws =
                window.EdmsAPI.connectBookmarkLoader();


            let settled = false;


            const finish = (
                callback,
                value
            ) => {

                if (settled) return;

                settled = true;


                try {

                    ws.close();

                } catch {}


                callback(value);

            };


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
                            "BOOKMARK WS MESSAGE:",
                            message
                        );


                        if (
                            message.type !==
                            "snapshot"
                        ) {

                            return;

                        }


                        if (
                            !Array.isArray(
                                message.bookmarks
                            )
                        ) {

                            finish(
                                reject,
                                new Error(
                                    "Bookmark snapshot contains no bookmarks array."
                                )
                            );

                            return;

                        }


                        finish(
                            resolve,
                            message
                        );

                    } catch (error) {

                        finish(
                            reject,
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


                    finish(
                        reject,
                        new Error(
                            "Bookmark WebSocket failed."
                        )
                    );

                }
            );


            ws.addEventListener(
                "close",
                () => {

                    if (!settled) {

                        finish(
                            reject,
                            new Error(
                                "Bookmark WebSocket closed before snapshot was received."
                            )
                        );

                    }

                }
            );

        }
    );

}




// ============================================================
// LOAD ACTIVE BOOKMARKS FROM BACKEND
// ============================================================

function loadBookmarksFromBackend() {

    return new Promise((resolve, reject) => {

        if (
            !window.EdmsAPI ||
            typeof window.EdmsAPI.connectBookmarkLoader !==
                "function"
        ) {

            reject(
                new Error(
                    "Bookmark loader API is unavailable."
                )
            );

            return;

        }


        const ws =
            window.EdmsAPI.connectBookmarkLoader();


        let settled = false;


        const finish = (
            callback,
            value
        ) => {

            if (settled) return;

            settled = true;


            try {

                ws.close();

            } catch {}


            callback(value);

        };


        ws.addEventListener(
            "message",
            event => {

                try {

                    const message =
                        JSON.parse(
                            event.data
                        );


                    console.log(
                        "Bookmark WebSocket message:",
                        message
                    );


                    if (
                        message.type ===
                            "snapshot" &&
                        Array.isArray(
                            message.bookmarks
                        )
                    ) {

                        finish(
                            resolve,
                            message
                        );

                    }


                } catch (error) {

                    finish(
                        reject,
                        error
                    );

                }

            }
        );


        ws.addEventListener(
            "error",
            () => {

                finish(
                    reject,
                    new Error(
                        "Bookmark WebSocket failed."
                    )
                );

            }
        );

    });

}


// ============================================================
// BOOKMARK ENDPOINT ID
// ============================================================

function getBookmarkEndpointId(bookmark) {

    if (
        bookmark === undefined ||
        bookmark === null
    ) {
        return null;
    }

    if (
        typeof bookmark !== "object"
    ) {
        return bookmark;
    }

    return (
        bookmark.endpoint_id ??
        bookmark.endpointId ??
        bookmark.id ??
        null
    );
}

// ============================================================
// FILTER SETUP
// ============================================================

function setupFilters() {

    const searchInput =
        document.getElementById("searchInput");

    if (searchInput) {

        searchInput.addEventListener(
            "input",
            event => {

                activeFilters.search =
                    event.target.value
                        .trim()
                        .toLowerCase();

                currentPage = 1;

                applyFilters();

            }
        );

    }


    const crudFilter =
        document.getElementById("crudFilter");

    if (crudFilter) {

        crudFilter.addEventListener(
            "change",
            event => {

                activeFilters.crud =
                    event.target.value;

                currentPage = 1;

                applyFilters();

            }
        );

    }


    const resetButton =
        document.getElementById("resetFilters");

    if (resetButton) {

        resetButton.addEventListener(
            "click",
            resetFilters
        );

    }

}


// ============================================================
// APPLY FILTERS
// ============================================================

function applyFilters() {

    filteredEndpoints =
        endpoints.filter(endpoint => {

            const search =
                activeFilters.search;


            // --------------------------------------------------
            // Search
            // --------------------------------------------------

            const matchesSearch =
                !search ||

                String(endpoint.id || "")
                    .toLowerCase()
                    .includes(search) ||

                String(endpoint.method || "")
                    .toLowerCase()
                    .includes(search) ||

                String(endpoint.endpoint || "")
                    .toLowerCase()
                    .includes(search) ||

                String(endpoint.annotation || "")
                    .toLowerCase()
                    .includes(search) ||

                (
                    Array.isArray(endpoint.tags) &&
                    endpoint.tags.some(
                        tag =>
                            String(tag)
                                .toLowerCase()
                                .includes(search)
                    )
                );


            // --------------------------------------------------
            // CRUD
            // --------------------------------------------------

            const matchesCRUD =
                activeFilters.crud === "All CRUD" ||
                endpoint.method ===
                    activeFilters.crud;


            // --------------------------------------------------
            // Tags
            // --------------------------------------------------

            const matchesTags =
                activeFilters.tags.size === 0 ||

                (
                    Array.isArray(endpoint.tags) &&
                    [...activeFilters.tags]
                        .every(
                            tag =>
                                endpoint.tags.includes(tag)
                        )
                );


            // --------------------------------------------------
            // Segments
            // --------------------------------------------------

            const endpointSegments =
                getEndpointSegments(
                    endpoint.endpoint
                );

            const matchesSegments =
                activeFilters.segments.size === 0 ||

                [...activeFilters.segments]
                    .every(
                        segment =>
                            endpointSegments
                                .includes(segment)
                    );


            return (
                matchesSearch &&
                matchesCRUD &&
                matchesTags &&
                matchesSegments
            );

        });


    currentPage = Math.min(
        currentPage,
        getTotalPages()
    );


    if (currentPage < 1) {

        currentPage = 1;

    }


    renderTable();

    updateFooter();

}


// ============================================================
// RESET FILTERS
// ============================================================

function resetFilters() {

    activeFilters.search = "";

    activeFilters.crud = "All CRUD";

    activeFilters.tags.clear();

    activeFilters.segments.clear();


    const searchInput =
        document.getElementById("searchInput");

    if (searchInput) {

        searchInput.value = "";

    }


    const crudFilter =
        document.getElementById("crudFilter");

    if (crudFilter) {

        crudFilter.value = "All CRUD";

    }


    document
        .querySelectorAll(
            ".tag-checkbox, .segment-checkbox"
        )
        .forEach(
            checkbox =>
                checkbox.checked = false
        );


    currentPage = 1;

    applyFilters();

}


// ============================================================
// SEGMENTS
// ============================================================

function getEndpointSegments(endpointPath) {

    return String(endpointPath || "")
        .split("/")
        .filter(Boolean);

}


// ============================================================
// TAGS
// ============================================================

function renderTags() {

    const container =
        document.getElementById("tagsContainer");

    if (!container) return;

    container.innerHTML = "";

    const tagCounts = {};

    endpoints.forEach(endpoint => {

        if (!Array.isArray(endpoint.tags)) {
            return;
        }

        endpoint.tags.forEach(tag => {

            if (!tag) return;

            tagCounts[tag] =
                (tagCounts[tag] || 0) + 1;

        });

    });


    Object.entries(tagCounts)
        .sort((a, b) =>
            a[0].localeCompare(b[0])
        )
        .forEach(([tag, count]) => {

            const wrapper =
                document.createElement("label");

            wrapper.className =
                "group flex cursor-pointer items-center gap-2";


            wrapper.innerHTML = `

                <input
                    type="checkbox"
                    class="tag-checkbox h-3.5 w-3.5
                           accent-cyan-400"
                    data-tag="${escapeAttribute(tag)}"
                >

                <button
                    type="button"
                    class="tag-filter-button min-w-0 flex-1
                           truncate rounded px-1.5 py-1 text-left
                           text-[11px] text-slate-400
                           transition hover:bg-sky-500/10
                           hover:text-sky-300"
                    data-tag="${escapeAttribute(tag)}"
                >
                    ${escapeHTML(tag)}
                    <span class="text-slate-600">
                        (${count})
                    </span>
                </button>

            `;


            const checkbox =
                wrapper.querySelector(
                    ".tag-checkbox"
                );

            const tagButton =
                wrapper.querySelector(
                    ".tag-filter-button"
                );


            checkbox.addEventListener(
                "change",
                () => {

                    setTagFilter(
                        tag,
                        checkbox.checked
                    );

                }
            );


            tagButton.addEventListener(
                "click",
                event => {

                    event.preventDefault();

                    const checked =
                        !activeFilters.tags.has(tag);

                    checkbox.checked =
                        checked;

                    setTagFilter(
                        tag,
                        checked
                    );

                }
            );


            container.appendChild(wrapper);

        });

}


// ============================================================
// TAG FILTER
// ============================================================

function setTagFilter(tag, enabled) {

    if (enabled) {

        activeFilters.tags.add(tag);

    } else {

        activeFilters.tags.delete(tag);

    }

    currentPage = 1;

    applyFilters();

}


// ============================================================
// SEGMENTS
// ============================================================

function renderSegments() {

    const container =
        document.getElementById(
            "segmentsContainer"
        );

    if (!container) return;

    container.innerHTML = "";

    const segmentCounts = {};


    endpoints.forEach(endpoint => {

        getEndpointSegments(
            endpoint.endpoint
        ).forEach(segment => {

            segmentCounts[segment] =
                (segmentCounts[segment] || 0) + 1;

        });

    });


    Object.entries(segmentCounts)
        .sort((a, b) =>
            a[0].localeCompare(b[0])
        )
        .forEach(([segment, count]) => {

            const wrapper =
                document.createElement("label");

            wrapper.className =
                "group flex cursor-pointer items-center gap-2";


            wrapper.innerHTML = `

                <input
                    type="checkbox"
                    class="segment-checkbox h-3.5 w-3.5
                           accent-cyan-400"
                    data-segment="${escapeAttribute(segment)}"
                >

                <button
                    type="button"
                    class="segment-filter-button min-w-0
                           flex-1 truncate rounded px-1.5 py-1
                           text-left text-[11px]
                           text-slate-400 transition
                           hover:bg-cyan-500/10
                           hover:text-cyan-300"
                    data-segment="${escapeAttribute(segment)}"
                >
                    ${escapeHTML(segment)}
                    <span class="text-slate-600">
                        (${count})
                    </span>
                </button>

            `;


            const checkbox =
                wrapper.querySelector(
                    ".segment-checkbox"
                );

            const segmentButton =
                wrapper.querySelector(
                    ".segment-filter-button"
                );


            checkbox.addEventListener(
                "change",
                () => {

                    setSegmentFilter(
                        segment,
                        checkbox.checked
                    );

                }
            );


            segmentButton.addEventListener(
                "click",
                event => {

                    event.preventDefault();

                    const checked =
                        !activeFilters.segments
                            .has(segment);

                    checkbox.checked =
                        checked;

                    setSegmentFilter(
                        segment,
                        checked
                    );

                }
            );


            container.appendChild(wrapper);

        });

}





// ============================================================
// SEGMENT FILTER
// ============================================================

function setSegmentFilter(
    segment,
    enabled
) {

    if (enabled) {

        activeFilters.segments.add(
            segment
        );

    } else {

        activeFilters.segments.delete(
            segment
        );

    }

    currentPage = 1;

    applyFilters();

}


// ============================================================
// TABLE
// ============================================================

function renderTable() {

    if (!tableBody) return;

    tableBody.innerHTML = "";


    const pageItems =
        getCurrentPageItems();


    if (pageItems.length === 0) {

        tableBody.innerHTML = `

            <tr>

                <td
                    colspan="8"
                    class="px-4 py-16 text-center"
                >

                    <div
                        class="mx-auto max-w-xs"
                    >

                        <p
                            class="text-sm font-medium
                                   text-slate-400"
                        >
                            No endpoints found
                        </p>

                        <p
                            class="mt-1 text-xs
                                   text-slate-600"
                        >
                            Try changing the current
                            search or filters.
                        </p>

                    </div>

                </td>

            </tr>

        `;

        return;

    }


    pageItems.forEach(endpoint => {

        tableBody.appendChild(
            createRow(endpoint)
        );

    });


    updateSelectAllState();

}


// ============================================================
// CREATE ROW
// ============================================================

function createRow(endpoint) {

    const row =
        document.createElement("tr");


    const isSelected =
        selectedEndpointIds.has(
            getEndpointKey(endpoint)
        );


    const isActive =
        selectedEndpoint === endpoint;


    row.className =
        [
            "endpoint-row",
            "border-b",
            "border-slate-800",
            "transition-colors",
            "duration-100",
            "hover:bg-slate-800/60",
            "cursor-pointer",
            "group",
            isSelected
                ? "bg-cyan-500/[0.045]"
                : "",
            isActive
                ? "bg-sky-500/[0.08]"
                : ""
        ]
            .filter(Boolean)
            .join(" ");


    row.dataset.id =
        getEndpointKey(endpoint);

    row.endpoint = endpoint;


    // ----------------------------------------------------------
    // Method
    // ----------------------------------------------------------

    let methodColor =
        "text-white";

    switch (endpoint.method) {

        case "GET":
            methodColor =
                "text-emerald-400";
            break;

        case "POST":
            methodColor =
                "text-sky-400";
            break;

        case "PUT":
            methodColor =
                "text-amber-400";
            break;

        case "PATCH":
            methodColor =
                "text-violet-400";
            break;

        case "DELETE":
            methodColor =
                "text-rose-400";
            break;

    }


    // ----------------------------------------------------------
    // Tags
    // ----------------------------------------------------------

    const tagsHTML =
        Array.isArray(endpoint.tags)

            ? endpoint.tags
                .map(tag => `

                    <button
                        type="button"
                        class="row-tag mr-1 inline-flex
                               rounded bg-sky-900/40
                               px-1.5 py-0.5 text-[10px]
                               text-sky-300
                               transition
                               hover:bg-sky-500/20
                               hover:text-sky-200"
                        data-tag="${escapeAttribute(tag)}"
                    >
                        ${escapeHTML(tag)}
                    </button>

                `)
                .join("")

            : "";


    row.innerHTML = `

        <td class="px-2 py-2 align-middle">

            <input
                type="checkbox"
                class="row-checkbox h-3.5 w-3.5
                       accent-cyan-400"
                ${isSelected ? "checked" : ""}
            >

        </td>


        <td class="px-2 py-2 font-mono text-slate-500">
            ${escapeHTML(endpoint.id)}
        </td>


        <td
            class="px-2 py-2 font-semibold ${methodColor}"
        >
            ${escapeHTML(endpoint.method)}
        </td>


        <td
            class="endpoint-cell px-2 py-2 font-mono
                   text-[11px] text-slate-300"
        >
            <button
                type="button"
                class="row-segment-trigger max-w-full
                       truncate text-left
                       hover:text-cyan-300"
                title="Click segments to filter"
            >
                ${escapeHTML(endpoint.endpoint)}
            </button>
        </td>


        <td class="px-2 py-2">
            <div class="flex flex-wrap gap-y-1">
                ${tagsHTML}
            </div>
        </td>


        <td class="px-2 py-2 text-center">

            <span
                class="inline-flex min-w-6 items-center
                       justify-center rounded-md
                       bg-slate-800 px-1.5 py-0.5
                       text-[10px] text-slate-400"
            >
                ${
                    Array.isArray(endpoint.qps)
                        ? endpoint.qps.length
                        : 0
                }
            </span>

        </td>


        <td
            class="px-2 py-2 text-[11px]
                   text-slate-500"
        >
            <span
                class="line-clamp-2"
                title="${escapeAttribute(
                    endpoint.annotation || ""
                )}"
            >
                ${escapeHTML(
                    endpoint.annotation || "—"
                )}
            </span>
        </td>


        <td
            class="px-2 py-2 text-[10px]
                   text-slate-600"
        >
            ${formatDate(endpoint.updated)}
        </td>

    `;


    // ----------------------------------------------------------
    // Checkbox
    // ----------------------------------------------------------

    const checkbox =
        row.querySelector(
            ".row-checkbox"
        );


    checkbox.addEventListener(
        "click",
        event => {

            event.stopPropagation();

        }
    );


    checkbox.addEventListener(
        "change",
        () => {

            toggleEndpointSelection(
                endpoint
            );

        }
    );


    // ----------------------------------------------------------
    // Tags
    // ----------------------------------------------------------

    row.querySelectorAll(
        ".row-tag"
    ).forEach(button => {

        button.addEventListener(
            "click",
            event => {

                event.stopPropagation();

                const tag =
                    button.dataset.tag;

                activeFilters.tags.clear();

                activeFilters.tags.add(tag);

                syncFilterCheckboxes();

                currentPage = 1;

                applyFilters();

            }
        );

    });


    // ----------------------------------------------------------
    // Endpoint segment click
    // ----------------------------------------------------------

    const endpointButton =
        row.querySelector(
            ".row-segment-trigger"
        );


    endpointButton.addEventListener(
        "click",
        event => {

            event.stopPropagation();

            openSegmentChooser(
                endpoint
            );

        }
    );


    // ----------------------------------------------------------
    // Normal row click
    // ----------------------------------------------------------

    row.addEventListener(
        "click",
        () => {

            selectEndpoint(
                endpoint,
                row
            );

        }
    );


    // ----------------------------------------------------------
    // RMB
    // ----------------------------------------------------------

    row.addEventListener(
        "contextmenu",
        event => {

            event.preventDefault();

            activeContextEndpoint =
                endpoint;

            activeContextQP = null;

            openEndpointContextMenu(
                event.clientX,
                event.clientY,
                endpoint
            );

        }
    );


    return row;

}


// ============================================================
// ENDPOINT SELECTION
// ============================================================

function toggleEndpointSelection(
    endpoint
) {

    const key =
        getEndpointKey(endpoint);


    if (selectedEndpointIds.has(key)) {

        selectedEndpointIds.delete(key);

    } else {

        selectedEndpointIds.add(key);

    }


    updateSelectionUI();

    updateSelectAllState();

    renderTable();

}


// ============================================================
// SELECT ENDPOINT
// ============================================================

function selectEndpoint(
    endpoint,
    row
) {

    selectedEndpoint =
        endpoint;

    selectedQP = null;


    if (deleteQPButton) {

        deleteQPButton.disabled =
            true;

    }


    if (addQPButton) {

        addQPButton.disabled =
            false;

    }


    document
        .querySelectorAll(
            ".endpoint-row"
        )
        .forEach(
            currentRow => {

                currentRow.classList.remove(
                    "bg-sky-500/[0.08]"
                );

            }
        );


    row.classList.add(
        "bg-sky-500/[0.08]"
    );


    renderQP(endpoint);

}


// ============================================================
// QP PANEL
// ============================================================

function renderQP(endpoint) {

    if (!qpPanel) return;

    qpPanel.innerHTML = "";


    if (
        !endpoint ||
        !Array.isArray(endpoint.qps) ||
        endpoint.qps.length === 0
    ) {

        qpPanel.innerHTML = `

            <div
                class="py-6 text-center
                       text-[10px] text-slate-600"
            >
                No QP
            </div>

        `;

        return;

    }


    endpoint.qps.forEach(
        (qp, index) => {

            const button =
                document.createElement("button");


            const qpId =
                qp.id ?? index + 1;


            button.type =
                "button";

            button.dataset.qp =
                qpId;


            button.className =
                [
                    "qp-btn",
                    "flex",
                    "h-8",
                    "w-full",
                    "items-center",
                    "justify-center",
                    "rounded-md",
                    "border",
                    "border-slate-700",
                    "bg-slate-800",
                    "text-[10px]",
                    "text-slate-400",
                    "transition",
                    "hover:border-cyan-500/40",
                    "hover:bg-cyan-500/15",
                    "hover:text-cyan-300"
                ].join(" ");


            button.textContent =
                qpId;


            if (
                selectedQP &&
                selectedQP === qp
            ) {

                button.classList.remove(
                    "bg-slate-800",
                    "text-slate-400"
                );

                button.classList.add(
                    "bg-cyan-500",
                    "text-white"
                );

            }


            button.addEventListener(
                "click",
                event => {

                    event.stopPropagation();

                    selectQP(
                        qp,
                        button
                    );

                }
            );


            button.addEventListener(
                "contextmenu",
                event => {

                    event.preventDefault();

                    event.stopPropagation();

                    activeContextEndpoint =
                        endpoint;

                    activeContextQP =
                        qp;

                    openQPContextMenu(
                        event.clientX,
                        event.clientY,
                        endpoint,
                        qp
                    );

                }
            );


            qpPanel.appendChild(button);

        }
    );

}


// ============================================================
// SELECT QP
// ============================================================

function selectQP(
    qp,
    button
) {

    selectedQP =
        qp;


    if (deleteQPButton) {

        deleteQPButton.disabled =
            false;

    }


    document
        .querySelectorAll(
            ".qp-btn"
        )
        .forEach(
            btn => {

                btn.classList.remove(
                    "bg-cyan-500",
                    "text-white"
                );

                btn.classList.add(
                    "bg-slate-800",
                    "text-slate-400"
                );

            }
        );


    button.classList.remove(
        "bg-slate-800",
        "text-slate-400"
    );

    button.classList.add(
        "bg-cyan-500",
        "text-white"
    );


    showRequestResponse(qp);

}


// ============================================================
// REQUEST / RESPONSE
// ============================================================

function showRequestResponse(qp) {

    if (!qp || !selectedEndpoint) {
        return;
    }


    const request =
        qp.request || {};

    const response =
        qp.response || {};


    // ----------------------------------------------------------
    // Request
    // ----------------------------------------------------------

    if (requestEndpoint) {

        requestEndpoint.textContent =
            `${selectedEndpoint.method} ${selectedEndpoint.endpoint}`;

    }


    if (requestHeaders) {

        requestHeaders.textContent =
            formatJSON(
                request.headers
            );

    }


    if (requestQuery) {

        requestQuery.textContent =
            formatJSON(
                request.query
            );

    }


    if (requestPath) {

        requestPath.textContent =
            formatJSON(
                request.path
            );

    }


    if (requestBody) {

        requestBody.textContent =
            formatJSON(
                request.body
            );

    }


    // ----------------------------------------------------------
    // Response
    // ----------------------------------------------------------

    if (responseStatus) {

        responseStatus.textContent =
            response.status
                ? String(response.status)
                : "—";


        responseStatus.className =
            [
                "mt-0.5",
                "text-[11px]",
                "font-semibold",
                getStatusColor(
                    response.status
                )
            ].join(" ");

    }


    if (responseHeaders) {

        responseHeaders.textContent =
            formatJSON(
                response.headers
            );

    }


    if (responseBody) {

        responseBody.textContent =
            formatJSON(
                response.body
            );

    }


    if (responseTime) {

        responseTime.textContent =
            response.time
                ? `${response.time} ms`
                : "—";

    }


    if (responsePayload) {

        responsePayload.textContent =
            calculatePayload(
                response.body
            );

    }


    if (responseServer) {

        responseServer.textContent =
            response.server || "—";

    }


    if (detailsOverlay) {

        detailsOverlay.classList.remove(
            "hidden"
        );

    }

}


// ============================================================
// REQUEST / RESPONSE VIEWER CONTROLS
// ============================================================

function setupDetailsViewer() {

    detailsClose?.addEventListener(
        "click",
        hideRequestResponse
    );


    detailsFullscreen?.addEventListener(
        "click",
        toggleDetailsFullscreen
    );


    detailsOverlay?.addEventListener(
        "click",
        event => {

            if (
                event.target === detailsOverlay ||
                event.target === detailsOverlay.firstElementChild
            ) {

                hideRequestResponse();

            }

        }
    );

}


// ============================================================
// TOGGLE FULLSCREEN
// ============================================================

function toggleDetailsFullscreen() {

    if (!detailsPanel) {
        return;
    }


    detailsFullscreenActive =
        !detailsFullscreenActive;


    if (detailsFullscreenActive) {

        detailsPanel.classList.remove(
            "bottom-4",
            "right-4",
            "h-[min(720px,calc(100vh-32px))]",
            "w-[min(1100px,calc(100vw-32px))]",
            "rounded-xl"
        );


        detailsPanel.classList.add(
            "inset-0",
            "h-full",
            "w-full",
            "rounded-none"
        );


        if (detailsFullscreenIcon) {

            detailsFullscreenIcon.innerHTML = `
                <path d="M9 3H5a2 2 0 0 0-2 2v4"/>
                <path d="M15 3h4a2 2 0 0 1 2 2v4"/>
                <path d="M21 15v4a2 2 0 0 1-2 2h-4"/>
                <path d="M3 15v4a2 2 0 0 0 2 2h4"/>
            `;

        }


        detailsFullscreen?.setAttribute(
            "title",
            "Exit full screen"
        );


    } else {

        detailsPanel.classList.remove(
            "inset-0",
            "h-full",
            "w-full",
            "rounded-none"
        );


        detailsPanel.classList.add(
            "bottom-4",
            "right-4",
            "h-[min(720px,calc(100vh-32px))]",
            "w-[min(1100px,calc(100vw-32px))]",
            "rounded-xl"
        );


        if (detailsFullscreenIcon) {

            detailsFullscreenIcon.innerHTML = `
                <path d="M8 3H5a2 2 0 0 0-2 2v3"/>
                <path d="M16 3h3a2 2 0 0 1 2 2v3"/>
                <path d="M21 16v3a2 2 0 0 1-2 2h-3"/>
                <path d="M3 16v3a2 2 0 0 0 2 2h3"/>
            `;

        }


        detailsFullscreen?.setAttribute(
            "title",
            "Full screen"
        );

    }

}


// ============================================================
// CLOSE DETAILS
// ============================================================

function hideRequestResponse() {

    if (!detailsOverlay) {
        return;
    }


    detailsOverlay.classList.add(
        "hidden"
    );


    if (
        detailsFullscreenActive &&
        detailsPanel
    ) {

        detailsFullscreenActive =
            false;


        detailsPanel.classList.remove(
            "inset-0",
            "h-full",
            "w-full",
            "rounded-none"
        );


        detailsPanel.classList.add(
            "bottom-4",
            "right-4",
            "h-[min(720px,calc(100vh-32px))]",
            "w-[min(1100px,calc(100vw-32px))]",
            "rounded-xl"
        );


        detailsFullscreen?.setAttribute(
            "title",
            "Full screen"
        );


        if (detailsFullscreenIcon) {

            detailsFullscreenIcon.innerHTML = `
                <path d="M8 3H5a2 2 0 0 0-2 2v3"/>
                <path d="M16 3h3a2 2 0 0 1 2 2v3"/>
                <path d="M21 16v3a2 2 0 0 1-2 2h-3"/>
                <path d="M3 16v3a2 2 0 0 0 2 2h3"/>
            `;

        }

    }

}


// ============================================================
// RESET DETAILS
// ============================================================

function resetDetails() {

    selectedQP = null;

    hideRequestResponse();

    if (deleteQPButton) {

        deleteQPButton.disabled =
            true;

    }


    if (addQPButton) {

        addQPButton.disabled =
            !selectedEndpoint;

    }


    renderQP(
        selectedEndpoint
    );

}


// ============================================================
// DELETE SELECTED QP
// ============================================================

if (deleteQPButton) {

    deleteQPButton.addEventListener(
        "click",
        () => {

            if (
                !selectedEndpoint ||
                !selectedQP
            ) {
                return;
            }


            const qps =
                selectedEndpoint.qps;


            if (!Array.isArray(qps)) {
                return;
            }


            const index =
                qps.indexOf(
                    selectedQP
                );


            if (index === -1) {
                return;
            }


            qps.splice(
                index,
                1
            );


            showToast(
                "QP removed.",
                "success"
            );


            resetDetails();

            renderTable();

        }
    );

}


// ============================================================
// ADD QP
// ============================================================

if (addQPButton) {

    addQPButton.addEventListener(
        "click",
        () => {

            if (!selectedEndpoint) {
                return;
            }

            openAddQPModal();

        }
    );

}


// ============================================================
// ADD QP MODAL
// ============================================================

function openAddQPModal() {

    openModal(
        "Add Query Parameter",
        `
            <div class="space-y-4">

                <div>

                    <label
                        class="mb-1 block text-xs
                               text-slate-500"
                    >
                        QP Name
                    </label>

                    <input
                        id="newQPName"
                        class="h-9 w-full rounded-md
                               border border-slate-700
                               bg-slate-950 px-3 text-xs
                               outline-none
                               focus:border-cyan-500"
                        placeholder="Example: Valid Request"
                    >

                </div>


                <div
                    class="rounded-lg border
                           border-slate-800
                           bg-slate-950/50 p-3"
                >

                    <p
                        class="text-xs
                               text-slate-500"
                    >
                        A new QP will be added to the
                        currently selected endpoint.
                    </p>

                </div>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="confirmAddQP"
                        type="button"
                        class="rounded-md bg-cyan-600
                               px-3 py-1.5 text-xs
                               font-medium text-white
                               hover:bg-cyan-500"
                    >
                        Add QP
                    </button>

                </div>

            </div>
        `
    );


    document
        .getElementById(
            "confirmAddQP"
        )
        ?.addEventListener(
            "click",
            () => {

                const input =
                    document.getElementById(
                        "newQPName"
                    );


                const name =
                    input?.value.trim();


                if (!name) {

                    showToast(
                        "Enter a QP name.",
                        "error"
                    );

                    return;

                }


                if (
                    !Array.isArray(
                        selectedEndpoint.qps
                    )
                ) {

                    selectedEndpoint.qps = [];

                }


                const nextId =
                    selectedEndpoint.qps
                        .reduce(
                            (
                                max,
                                qp
                            ) =>
                                Math.max(
                                    max,
                                    Number(
                                        qp.id
                                    ) || 0
                                ),
                            0
                        ) + 1;


                selectedEndpoint.qps.push({

                    id: nextId,

                    name,

                    request: {},

                    response: {}

                });


                closeModal();

                renderQP(
                    selectedEndpoint
                );

                renderTable();

                showToast(
                    "QP added.",
                    "success"
                );

            }
        );

}


// ============================================================
// PAGINATION
// ============================================================

function getTotalPages() {

    return Math.max(
        1,
        Math.ceil(
            filteredEndpoints.length /
            PAGE_SIZE
        )
    );

}


function getCurrentPageItems() {

    const start =
        (currentPage - 1) *
        PAGE_SIZE;


    return filteredEndpoints.slice(
        start,
        start + PAGE_SIZE
    );

}


function renderPagination() {

    const container =
        document.getElementById(
            "pagination"
        );

    if (!container) return;

    container.innerHTML = "";


    const totalPages =
        getTotalPages();


    const createButton =
        (
            label,
            page,
            disabled = false,
            active = false
        ) => {

            const button =
                document.createElement(
                    "button"
                );


            button.type =
                "button";

            button.textContent =
                label;


            button.className =
                [
                    "min-w-7",
                    "rounded-md",
                    "px-2",
                    "py-1",
                    "text-[11px]",
                    "transition",
                    active
                        ? "bg-cyan-500 text-white"
                        : "border border-slate-700 text-slate-400 hover:bg-slate-800 hover:text-white",
                    disabled
                        ? "cursor-not-allowed opacity-30"
                        : ""
                ]
                    .filter(Boolean)
                    .join(" ");


            button.disabled =
                disabled;


            button.addEventListener(
                "click",
                () => {

                    currentPage =
                        page;

                    renderTable();

                    updateFooter();

                }
            );


            return button;

        };


    container.appendChild(
        createButton(
            "‹",
            currentPage - 1,
            currentPage === 1
        )
    );


    const pages =
        getPaginationPages(
            currentPage,
            totalPages
        );


    pages.forEach(
        page => {

            if (page === "...") {

                const span =
                    document.createElement(
                        "span"
                    );

                span.className =
                    "px-1 text-slate-600";

                span.textContent =
                    "…";

                container.appendChild(
                    span
                );

                return;

            }


            container.appendChild(
                createButton(
                    String(page),
                    page,
                    false,
                    page === currentPage
                )
            );

        }
    );


    container.appendChild(
        createButton(
            "›",
            currentPage + 1,
            currentPage === totalPages
        )
    );

}


function getPaginationPages(
    current,
    total
) {

    if (total <= 7) {

        return Array.from(
            {
                length: total
            },
            (_, index) =>
                index + 1
        );

    }


    const pages = [
        1
    ];


    if (current > 4) {

        pages.push("...");

    }


    const start =
        Math.max(
            2,
            current - 1
        );


    const end =
        Math.min(
            total - 1,
            current + 1
        );


    for (
        let page = start;
        page <= end;
        page++
    ) {

        pages.push(page);

    }


    if (current < total - 3) {

        pages.push("...");

    }


    pages.push(total);


    return pages;

}


// ============================================================
// FOOTER
// ============================================================

function updateFooter() {

    const showing =
        document.getElementById(
            "showingEndpoints"
        );

    const total =
        document.getElementById(
            "totalEndpoints"
        );


    const pageItems =
        getCurrentPageItems();


    if (showing) {

        showing.textContent =
            pageItems.length;

    }


    if (total) {

        total.textContent =
            filteredEndpoints.length;

    }


    renderPagination();

}


// ============================================================
// STATS
// ============================================================

function updateStats() {

    const endpointStat =
        document.getElementById(
            "endpointStat"
        );

    const tagStat =
        document.getElementById(
            "tagStat"
        );

    const segmentStat =
        document.getElementById(
            "segmentStat"
        );


    if (endpointStat) {

        endpointStat.textContent =
            endpoints.length;

    }


    const uniqueTags =
        new Set();


    const uniqueSegments =
        new Set();


    endpoints.forEach(
        endpoint => {

            if (
                Array.isArray(
                    endpoint.tags
                )
            ) {

                endpoint.tags.forEach(
                    tag =>
                        uniqueTags.add(tag)
                );

            }


            getEndpointSegments(
                endpoint.endpoint
            ).forEach(
                segment =>
                    uniqueSegments.add(
                        segment
                    )
            );

        }
    );


    if (tagStat) {

        tagStat.textContent =
            uniqueTags.size;

    }


    if (segmentStat) {

        segmentStat.textContent =
            uniqueSegments.size;

    }

}


// ============================================================
// SIDEBAR
// ============================================================

function setupSidebar() {

    const toggle =
        document.getElementById(
            "sidebarToggle"
        );


    if (!toggle) return;


    toggle.addEventListener(
        "click",
        toggleSidebar
    );

}


function toggleSidebar() {

    const sidebar =
        document.getElementById(
            "filterSidebar"
        );

    const content =
        document.getElementById(
            "sidebarContent"
        );

    const title =
        document.getElementById(
            "sidebarTitle"
        );

    const icon =
        document.getElementById(
            "sidebarToggleIcon"
        );


    sidebarCollapsed =
        !sidebarCollapsed;


    if (sidebarCollapsed) {

        sidebar.classList.remove(
            "w-48"
        );

        sidebar.classList.add(
            "w-10"
        );


        content.classList.add(
            "hidden"
        );

        title.classList.add(
            "hidden"
        );


        icon.innerHTML = `
            <path d="m9 18 6-6-6-6"/>
        `;


    } else {

        sidebar.classList.remove(
            "w-10"
        );

        sidebar.classList.add(
            "w-48"
        );


        content.classList.remove(
            "hidden"
        );

        title.classList.remove(
            "hidden"
        );


        icon.innerHTML = `
            <path d="m15 18-6-6 6-6"/>
        `;

    }

}


// ============================================================
// MULTI SELECTION
// ============================================================

function setupSelectionControls() {

    const selectAll =
        document.getElementById(
            "selectAll"
        );


    if (selectAll) {

        selectAll.addEventListener(
            "change",
            () => {

                const pageItems =
                    getCurrentPageItems();


                pageItems.forEach(
                    endpoint => {

                        const key =
                            getEndpointKey(
                                endpoint
                            );


                        if (
                            selectAll.checked
                        ) {

                            selectedEndpointIds
                                .add(key);

                        } else {

                            selectedEndpointIds
                                .delete(key);

                        }

                    }
                );


                updateSelectionUI();

                renderTable();

            }
        );

    }


    document
        .getElementById(
            "clearSelection"
        )
        ?.addEventListener(
            "click",
            clearSelection
        );

}


function clearSelection() {

    selectedEndpointIds.clear();

    updateSelectionUI();

    renderTable();

}


function updateSelectionUI() {

    const bar =
        document.getElementById(
            "selectionBar"
        );

    const count =
        document.getElementById(
            "selectedCount"
        );

    const list =
        document.getElementById(
            "selectedList"
        );

    const deleteButton =
        document.getElementById(
            "deleteSelected"
        );

    const downloadButton =
        document.getElementById(
            "downloadSelected"
        );


    const selected =
        getSelectedEndpoints();


    if (selected.length === 0) {

        bar?.classList.add(
            "hidden"
        );

        bar?.classList.remove(
            "flex"
        );

    } else {

        bar?.classList.remove(
            "hidden"
        );

        bar?.classList.add(
            "flex"
        );

    }


    if (count) {

        count.textContent =
            `${selected.length} selected`;

    }


    if (list) {

        list.innerHTML =
            selected
                .map(
                    endpoint => `

                        <span
                            class="shrink-0 rounded-md
                                   border border-slate-700
                                   bg-slate-800 px-2 py-1
                                   font-mono text-[10px]
                                   text-slate-400"
                        >
                            ${escapeHTML(
                                endpoint.id
                            )}
                        </span>

                    `
                )
                .join("");

    }


    if (deleteButton) {

        deleteButton.disabled =
            selected.length === 0;

    }


    if (downloadButton) {

        downloadButton.disabled =
            selected.length === 0;

    }


    updateSelectAllState();

}


function updateSelectAllState() {

    const checkbox =
        document.getElementById(
            "selectAll"
        );


    if (!checkbox) return;


    const pageItems =
        getCurrentPageItems();


    const selectedCount =
        pageItems.filter(
            endpoint =>
                selectedEndpointIds.has(
                    getEndpointKey(endpoint)
                )
        ).length;


    checkbox.checked =
        pageItems.length > 0 &&
        selectedCount === pageItems.length;


    checkbox.indeterminate =
        selectedCount > 0 &&
        selectedCount < pageItems.length;

}


function getSelectedEndpoints() {

    return endpoints.filter(
        endpoint =>
            selectedEndpointIds.has(
                getEndpointKey(endpoint)
            )
    );

}


// ============================================================
// GLOBAL ACTIONS
// ============================================================

function setupGlobalActions() {

    document
        .getElementById(
            "deleteSelected"
        )
        ?.addEventListener(
            "click",
            openBulkDeleteModal
        );


    document
        .getElementById(
            "downloadSelected"
        )
        ?.addEventListener(
            "click",
            downloadSelectedEndpoints
        );

}


// ============================================================
// BULK DELETE
// ============================================================

function openBulkDeleteModal() {

    const selected =
        getSelectedEndpoints();


    if (selected.length === 0) {
        return;
    }


    openModal(
        "Delete Endpoints",
        `

            <div class="space-y-4">

                <div
                    class="rounded-lg border
                           border-rose-500/20
                           bg-rose-500/5 p-3"
                >

                    <p
                        class="text-xs text-rose-300"
                    >
                        ${selected.length}
                        endpoint(s) will be removed
                        from the current view.
                    </p>

                </div>


                <div
                    class="max-h-48 overflow-y-auto
                           rounded-lg border
                           border-slate-800"
                >

                    ${selected
                        .map(
                            endpoint => `

                                <div
                                    class="border-b
                                           border-slate-800
                                           px-3 py-2
                                           last:border-0"
                                >

                                    <p
                                        class="font-mono
                                               text-[11px]
                                               text-slate-300"
                                    >
                                        ${escapeHTML(
                                            endpoint.id
                                        )}
                                    </p>

                                    <p
                                        class="truncate
                                               text-[10px]
                                               text-slate-600"
                                    >
                                        ${escapeHTML(
                                            endpoint.endpoint
                                        )}
                                    </p>

                                </div>

                            `
                        )
                        .join("")}

                </div>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="confirmBulkDelete"
                        type="button"
                        class="rounded-md
                               bg-rose-600
                               px-3 py-1.5
                               text-xs font-medium
                               text-white
                               hover:bg-rose-500"
                    >
                        Delete
                    </button>

                </div>

            </div>

        `
    );


    document
        .getElementById(
            "confirmBulkDelete"
        )
        ?.addEventListener(
            "click",
            () => {

                const ids =
                    new Set(
                        selected.map(
                            endpoint =>
                                getEndpointKey(
                                    endpoint
                                )
                        )
                    );


                endpoints =
                    endpoints.filter(
                        endpoint =>
                            !ids.has(
                                getEndpointKey(
                                    endpoint
                                )
                            )
                    );


                selectedEndpointIds.clear();


                if (
                    selectedEndpoint &&
                    ids.has(
                        getEndpointKey(
                            selectedEndpoint
                        )
                    )
                ) {

                    selectedEndpoint =
                        null;

                    selectedQP =
                        null;

                    resetDetails();

                    renderQP(null);

                }


                closeModal();

                renderTags();

                renderSegments();

                updateStats();

                applyFilters();

                updateSelectionUI();

                showToast(
                    "Selected endpoints removed.",
                    "success"
                );

            }
        );

}


// ============================================================
// DOWNLOAD
// ============================================================

function downloadSelectedEndpoints() {

    const selected =
        getSelectedEndpoints();


    if (selected.length === 0) {
        return;
    }


    const blob =
        new Blob(
            [
                JSON.stringify(
                    {
                        endpoints:
                            selected
                    },
                    null,
                    2
                )
            ],
            {
                type:
                    "application/json"
            }
        );


    const url =
        URL.createObjectURL(
            blob
        );


    const anchor =
        document.createElement(
            "a"
        );


    anchor.href =
        url;

    anchor.download =
        "edms-selected-endpoints.json";


    document.body.appendChild(
        anchor
    );

    anchor.click();

    anchor.remove();


    URL.revokeObjectURL(
        url
    );


    showToast(
        "Selected endpoints downloaded.",
        "success"
    );

}


// ============================================================
// CONTEXT MENU
// ============================================================

function setupContextMenu() {

    document.addEventListener(
        "click",
        closeContextMenu
    );


    window.addEventListener(
        "scroll",
        closeContextMenu,
        true
    );


    window.addEventListener(
        "resize",
        closeContextMenu
    );

}


function openEndpointContextMenu(
    x,
    y,
    endpoint
) {

    if (!contextMenu) return;


    contextMenu.innerHTML = `

        ${contextMenuItem(
            "select",
            "Select",
            iconCheck()
        )}

        ${contextMenuItem(
            "tags",
            "Edit Tags",
            iconTag()
        )}

        ${contextMenuItem(
            "annotation",
            "Edit Annotation",
            iconEdit()
        )}

        ${contextMenuItem(
            "duplicate",
            "Duplicate",
            iconCopy()
        )}

        <div class="my-1 border-t border-slate-800"></div>

        ${contextMenuItem(
            "delete",
            "Delete",
            iconTrash(),
            "text-rose-400 hover:bg-rose-500/10"
        )}

    `;


    showContextMenu(
        x,
        y
    );


    bindContextAction(
        "select",
        () => {

            toggleEndpointSelection(
                endpoint
            );

        }
    );


    bindContextAction(
        "tags",
        () => {

            openTagEditor(
                endpoint
            );

        }
    );


    bindContextAction(
        "annotation",
        () => {

            openAnnotationEditor(
                endpoint
            );

        }
    );


    bindContextAction(
        "duplicate",
        () => {

            duplicateEndpoint(
                endpoint
            );

        }
    );


    bindContextAction(
        "delete",
        () => {

            openDeleteSingleModal(
                endpoint
            );

        }
    );

}


function openQPContextMenu(
    x,
    y,
    endpoint,
    qp
) {

    if (!contextMenu) return;


    contextMenu.innerHTML = `

        ${contextMenuItem(
            "open",
            "Open Request / Response",
            iconExternal()
        )}

        ${contextMenuItem(
            "edit",
            "Edit QP",
            iconEdit()
        )}

        ${contextMenuItem(
            "duplicate",
            "Duplicate QP",
            iconCopy()
        )}

        <div class="my-1 border-t border-slate-800"></div>

        ${contextMenuItem(
            "delete",
            "Delete QP",
            iconTrash(),
            "text-rose-400 hover:bg-rose-500/10"
        )}

    `;


    showContextMenu(
        x,
        y
    );


    bindContextAction(
        "open",
        () => {

            const button =
                [...document.querySelectorAll(
                    ".qp-btn"
                )]
                    .find(
                        item =>
                            String(
                                item.dataset.qp
                            ) ===
                            String(
                                qp.id
                            )
                    );


            if (button) {

                selectQP(
                    qp,
                    button
                );

            }

        }
    );


    bindContextAction(
        "edit",
        () => {

            openQPEditor(
                endpoint,
                qp
            );

        }
    );


    bindContextAction(
        "duplicate",
        () => {

            duplicateQP(
                endpoint,
                qp
            );

        }
    );


    bindContextAction(
        "delete",
        () => {

            deleteQPItem(
                endpoint,
                qp
            );

        }
    );

}


function contextMenuItem(
    id,
    label,
    icon,
    extraClass = ""
) {

    return `

        <button
            type="button"
            data-context-action="${id}"
            class="flex w-full items-center gap-2
                   px-3 py-2 text-left text-xs
                   text-slate-300 transition
                   hover:bg-slate-800
                   ${extraClass}"
        >
            ${icon}

            <span>
                ${label}
            </span>

        </button>

    `;

}


function bindContextAction(
    id,
    callback
) {

    contextMenu
        ?.querySelector(
            `[data-context-action="${id}"]`
        )
        ?.addEventListener(
            "click",
            event => {

                event.stopPropagation();

                closeContextMenu();

                callback();

            }
        );

}


function showContextMenu(
    x,
    y
) {

    contextMenu.classList.remove(
        "hidden"
    );


    const menuWidth =
        192;

    const menuHeight =
        contextMenu.offsetHeight;


    const left =
        Math.min(
            x,
            window.innerWidth -
                menuWidth -
                8
        );


    const top =
        Math.min(
            y,
            window.innerHeight -
                menuHeight -
                8
        );


    contextMenu.style.left =
        `${Math.max(8, left)}px`;

    contextMenu.style.top =
        `${Math.max(8, top)}px`;

}


function closeContextMenu() {

    contextMenu?.classList.add(
        "hidden"
    );

}


// ============================================================
// TAG EDITOR
// ============================================================

function openTagEditor(
    endpoint
) {

    const currentTags =
        Array.isArray(
            endpoint.tags
        )
            ? endpoint.tags.join(", ")
            : "";


    openModal(
        `Edit Tags — ${endpoint.id}`,
        `

            <div class="space-y-4">

                <div>

                    <label
                        class="mb-1 block text-xs
                               text-slate-500"
                    >
                        Tags
                    </label>

                    <input
                        id="tagEditorInput"
                        value="${escapeAttribute(
                            currentTags
                        )}"
                        class="h-9 w-full rounded-md
                               border border-slate-700
                               bg-slate-950 px-3
                               text-xs outline-none
                               focus:border-cyan-500"
                    >

                    <p
                        class="mt-1 text-[10px]
                               text-slate-600"
                    >
                        Separate tags with commas.
                    </p>

                </div>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="saveTags"
                        type="button"
                        class="rounded-md bg-cyan-600
                               px-3 py-1.5 text-xs
                               font-medium text-white
                               hover:bg-cyan-500"
                    >
                        Save
                    </button>

                </div>

            </div>

        `
    );


    document
        .getElementById(
            "saveTags"
        )
        ?.addEventListener(
            "click",
            () => {

                const input =
                    document.getElementById(
                        "tagEditorInput"
                    );


                endpoint.tags =
                    input.value
                        .split(",")
                        .map(
                            tag =>
                                tag.trim()
                        )
                        .filter(Boolean);


                closeModal();

                renderTags();

                updateStats();

                applyFilters();

                showToast(
                    "Tags updated.",
                    "success"
                );

            }
        );

}


// ============================================================
// ANNOTATION EDITOR
// ============================================================

function openAnnotationEditor(
    endpoint
) {

    openModal(
        `Edit Annotation — ${endpoint.id}`,
        `

            <div class="space-y-4">

                <textarea
                    id="annotationInput"
                    rows="5"
                    class="w-full resize-y rounded-md
                           border border-slate-700
                           bg-slate-950 p-3 text-xs
                           leading-5 text-slate-300
                           outline-none
                           focus:border-cyan-500"
                >${escapeHTML(
                    endpoint.annotation || ""
                )}</textarea>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="saveAnnotation"
                        type="button"
                        class="rounded-md bg-cyan-600
                               px-3 py-1.5 text-xs
                               font-medium text-white
                               hover:bg-cyan-500"
                    >
                        Save
                    </button>

                </div>

            </div>

        `
    );


    document
        .getElementById(
            "saveAnnotation"
        )
        ?.addEventListener(
            "click",
            () => {

                endpoint.annotation =
                    document
                        .getElementById(
                            "annotationInput"
                        )
                        .value
                        .trim();


                closeModal();

                renderTable();

                showToast(
                    "Annotation updated.",
                    "success"
                );

            }
        );

}


// ============================================================
// DUPLICATE ENDPOINT
// ============================================================

function duplicateEndpoint(
    endpoint
) {

    const copy =
        JSON.parse(
            JSON.stringify(
                endpoint
            )
        );


    copy.id =
        generateEndpointId();


    copy.addedAt =
        new Date().toISOString();


    copy.updated =
        new Date().toISOString();


    endpoints.push(
        copy
    );


    renderTags();

    renderSegments();

    updateStats();

    applyFilters();


    showToast(
        `Endpoint duplicated as ${copy.id}.`,
        "success"
    );

}


// ============================================================
// SINGLE DELETE
// ============================================================

function openDeleteSingleModal(
    endpoint
) {

    openModal(
        `Delete ${endpoint.id}`,
        `

            <div class="space-y-4">

                <div
                    class="rounded-lg border
                           border-rose-500/20
                           bg-rose-500/5 p-3"
                >

                    <p
                        class="text-xs text-rose-300"
                    >
                        This endpoint will be removed
                        from the current session.
                    </p>

                </div>


                <div
                    class="rounded-lg border
                           border-slate-800
                           bg-slate-950/40 p-3"
                >

                    <p
                        class="font-mono text-xs
                               text-slate-300"
                    >
                        ${escapeHTML(
                            endpoint.id
                        )}
                    </p>

                    <p
                        class="mt-1 truncate
                               font-mono text-[10px]
                               text-slate-600"
                    >
                        ${escapeHTML(
                            endpoint.endpoint
                        )}
                    </p>

                </div>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="confirmSingleDelete"
                        type="button"
                        class="rounded-md bg-rose-600
                               px-3 py-1.5 text-xs
                               font-medium text-white
                               hover:bg-rose-500"
                    >
                        Delete
                    </button>

                </div>

            </div>

        `
    );


    document
        .getElementById(
            "confirmSingleDelete"
        )
        ?.addEventListener(
            "click",
            () => {

                const key =
                    getEndpointKey(
                        endpoint
                    );


                endpoints =
                    endpoints.filter(
                        item =>
                            getEndpointKey(
                                item
                            ) !== key
                    );


                selectedEndpointIds.delete(
                    key
                );


                if (
                    selectedEndpoint ===
                    endpoint
                ) {

                    selectedEndpoint =
                        null;

                    selectedQP =
                        null;

                    resetDetails();

                    renderQP(null);

                }


                closeModal();

                renderTags();

                renderSegments();

                updateStats();

                applyFilters();

                updateSelectionUI();


                showToast(
                    "Endpoint deleted.",
                    "success"
                );

            }
        );

}


// ============================================================
// QP EDITOR
// ============================================================

function openQPEditor(
    endpoint,
    qp
) {

    openModal(
        `Edit QP — ${qp.name || qp.id}`,
        `

            <div class="space-y-4">

                <div>

                    <label
                        class="mb-1 block text-xs
                               text-slate-500"
                    >
                        QP Name
                    </label>

                    <input
                        id="qpNameInput"
                        value="${escapeAttribute(
                            qp.name || ""
                        )}"
                        class="h-9 w-full rounded-md
                               border border-slate-700
                               bg-slate-950 px-3
                               text-xs outline-none
                               focus:border-cyan-500"
                    >

                </div>


                <div class="flex justify-end gap-2">

                    <button
                        type="button"
                        data-modal-close
                        class="rounded-md border
                               border-slate-700
                               px-3 py-1.5 text-xs
                               text-slate-400
                               hover:bg-slate-800"
                    >
                        Cancel
                    </button>

                    <button
                        id="saveQP"
                        type="button"
                        class="rounded-md bg-cyan-600
                               px-3 py-1.5 text-xs
                               font-medium text-white
                               hover:bg-cyan-500"
                    >
                        Save
                    </button>

                </div>

            </div>

        `
    );


    document
        .getElementById(
            "saveQP"
        )
        ?.addEventListener(
            "click",
            () => {

                qp.name =
                    document
                        .getElementById(
                            "qpNameInput"
                        )
                        .value
                        .trim();


                closeModal();

                renderQP(
                    endpoint
                );

                showToast(
                    "QP updated.",
                    "success"
                );

            }
        );

}


// ============================================================
// DUPLICATE QP
// ============================================================

function duplicateQP(
    endpoint,
    qp
) {

    if (
        !Array.isArray(
            endpoint.qps
        )
    ) {

        endpoint.qps = [];

    }


    const copy =
        JSON.parse(
            JSON.stringify(
                qp
            )
        );


    copy.id =
        endpoint.qps.reduce(
            (
                max,
                item
            ) =>
                Math.max(
                    max,
                    Number(
                        item.id
                    ) || 0
                ),
            0
        ) + 1;


    copy.name =
        `${qp.name || "QP"} Copy`;


    endpoint.qps.push(
        copy
    );


    renderQP(
        endpoint
    );

    renderTable();


    showToast(
        "QP duplicated.",
        "success"
    );

}


// ============================================================
// DELETE QP ITEM
// ============================================================

function deleteQPItem(
    endpoint,
    qp
) {

    if (
        !Array.isArray(
            endpoint.qps
        )
    ) {
        return;
    }


    const index =
        endpoint.qps.indexOf(
            qp
        );


    if (index === -1) {
        return;
    }


    endpoint.qps.splice(
        index,
        1
    );


    if (
        selectedQP === qp
    ) {

        selectedQP =
            null;

        hideRequestResponse();

    }


    renderQP(
        endpoint
    );

    renderTable();


    showToast(
        "QP deleted.",
        "success"
    );

}


// ============================================================
// SEGMENT CHOOSER
// ============================================================

function openSegmentChooser(
    endpoint
) {

    const segments =
        getEndpointSegments(
            endpoint.endpoint
        );


    openModal(
        `Endpoint Segments — ${endpoint.id}`,
        `

            <div class="space-y-2">

                <p
                    class="mb-3 text-xs
                           text-slate-500"
                >
                    Select a segment to filter
                    the endpoint list.
                </p>

                ${segments
                    .map(
                        segment => `

                            <button
                                type="button"
                                class="segment-modal-option
                                       flex w-full items-center
                                       justify-between
                                       rounded-lg border
                                       border-slate-800
                                       bg-slate-950/40
                                       px-3 py-2 text-left
                                       transition
                                       hover:border-cyan-500/30
                                       hover:bg-cyan-500/5"
                                data-segment="${escapeAttribute(
                                    segment
                                )}"
                            >

                                <span
                                    class="font-mono
                                           text-xs
                                           text-slate-300"
                                >
                                    ${escapeHTML(
                                        segment
                                    )}
                                </span>

                                <span
                                    class="text-[10px]
                                           text-slate-600"
                                >
                                    Filter
                                </span>

                            </button>

                        `
                    )
                    .join("")}

            </div>

        `
    );


    document
        .querySelectorAll(
            ".segment-modal-option"
        )
        .forEach(
            button => {

                button.addEventListener(
                    "click",
                    () => {

                        const segment =
                            button.dataset.segment;


                        activeFilters.segments
                            .clear();

                        activeFilters.segments
                            .add(segment);


                        syncFilterCheckboxes();

                        currentPage = 1;

                        closeModal();

                        applyFilters();

                    }
                );

            }
        );

}


// ============================================================
// MODAL
// ============================================================

function setupModal() {

    document
        .getElementById(
            "modalClose"
        )
        ?.addEventListener(
            "click",
            closeModal
        );


    modalOverlay?.addEventListener(
        "click",
        event => {

            if (
                event.target ===
                modalOverlay
            ) {

                closeModal();

            }

        }
    );


    document.addEventListener(
        "click",
        event => {

            if (
                event.target.closest(
                    "[data-modal-close]"
                )
            ) {

                closeModal();

            }

        }
    );


    document.addEventListener(
        "keydown",
        event => {

            if (
                event.key === "Escape"
            ) {

                if (detailsFullscreenActive) {

                    toggleDetailsFullscreen();

                    return;

                }


                hideRequestResponse();

                closeModal();

                closeContextMenu();

            }

        }
    );

}


function openModal(
    title,
    content
) {

    if (!modalOverlay) return;


    modalTitle.textContent =
        title;

    modalContent.innerHTML =
        content;


    modalOverlay.classList.remove(
        "hidden"
    );

    modalOverlay.classList.add(
        "flex"
    );

}


function closeModal() {

    modalOverlay?.classList.add(
        "hidden"
    );

    modalOverlay?.classList.remove(
        "flex"
    );

}


// ============================================================
// TOAST
// ============================================================

function showToast(
    message,
    type = "info"
) {

    const toast =
        document.createElement(
            "div"
        );


    const icon =
        type === "error"
            ? iconAlert()
            : iconCheck();


    const color =
        type === "error"
            ? "border-rose-500/20 text-rose-300"
            : "border-emerald-500/20 text-emerald-300";


    toast.className =
        `
            fixed bottom-5 left-1/2 z-[100]
            flex -translate-x-1/2
            items-center gap-2
            rounded-lg border
            bg-slate-900
            px-3 py-2
            text-xs shadow-2xl
            ${color}
        `;


    toast.innerHTML =
        `${icon}<span>${escapeHTML(message)}</span>`;


    document.body.appendChild(
        toast
    );


    setTimeout(
        () => {

            toast.remove();

        },
        2400
    );

}


// ============================================================
// COLUMN RESIZING
// ============================================================

function makeColumnsResizable() {

    const headers =
        document.querySelectorAll(
            "th"
        );


    headers.forEach(
        header => {

            const handle =
                header.querySelector(
                    ".resize-handle"
                );


            if (!handle) {
                return;
            }


            let startX = 0;

            let startWidth = 0;


            handle.addEventListener(
                "mousedown",
                event => {

                    event.preventDefault();

                    event.stopPropagation();


                    startX =
                        event.pageX;

                    startWidth =
                        header.offsetWidth;


                    const resize =
                        moveEvent => {

                            const newWidth =
                                Math.max(
                                    60,
                                    startWidth +
                                    (
                                        moveEvent.pageX -
                                        startX
                                    )
                                );


                            header.style.width =
                                `${newWidth}px`;

                        };


                    const stopResize =
                        () => {

                            document
                                .removeEventListener(
                                    "mousemove",
                                    resize
                                );

                            document
                                .removeEventListener(
                                    "mouseup",
                                    stopResize
                                );

                        };


                    document
                        .addEventListener(
                            "mousemove",
                            resize
                        );

                    document
                        .addEventListener(
                            "mouseup",
                            stopResize
                        );

                }
            );

        }
    );

}


// ============================================================
// FILTER CHECKBOX SYNC
// ============================================================

function syncFilterCheckboxes() {

    document
        .querySelectorAll(
            ".tag-checkbox"
        )
        .forEach(
            checkbox => {

                checkbox.checked =
                    activeFilters.tags.has(
                        checkbox.dataset.tag
                    );

            }
        );


    document
        .querySelectorAll(
            ".segment-checkbox"
        )
        .forEach(
            checkbox => {

                checkbox.checked =
                    activeFilters.segments.has(
                        checkbox.dataset.segment
                    );

            }
        );

}


// ============================================================
// HELPERS
// ============================================================

function getEndpointKey(
    endpoint
) {

    return String(
        endpoint.id
    );

}


function generateEndpointId() {

    let number = 1;


    while (
        endpoints.some(
            endpoint =>
                String(
                    endpoint.id
                ) ===
                `E${String(number)
                    .padStart(3, "0")}`
        )
    ) {

        number++;

    }


    return `E${String(number)
        .padStart(3, "0")}`;

}


function formatJSON(
    value
) {

    if (
        value === undefined ||
        value === null ||
        (
            typeof value === "object" &&
            Object.keys(value).length === 0
        )
    ) {

        return "—";

    }


    if (
        typeof value === "string"
    ) {

        return value;

    }


    try {

        return JSON.stringify(
            value,
            null,
            2
        );

    } catch {

        return String(
            value
        );

    }

}


function calculatePayload(
    body
) {

    if (
        body === undefined ||
        body === null
    ) {

        return "—";

    }


    try {

        const text =
            JSON.stringify(
                body
            );


        return `${text.length} B`;

    } catch {

        return "—";

    }

}


function getStatusColor(
    status
) {

    const value =
        Number(status);


    if (
        value >= 200 &&
        value < 300
    ) {

        return "text-emerald-400";

    }


    if (
        value >= 400 &&
        value < 500
    ) {

        return "text-amber-400";

    }


    if (
        value >= 500
    ) {

        return "text-rose-400";

    }


    return "text-slate-400";

}


function formatDate(
    value
) {

    if (!value) {
        return "—";
    }


    const date =
        new Date(value);


    if (
        Number.isNaN(
            date.getTime()
        )
    ) {

        return String(
            value
        );

    }


    return date.toLocaleDateString(
        undefined,
        {
            month: "short",
            day: "numeric",
            year: "numeric"
        }
    );

}


function escapeHTML(
    value
) {

    return String(
        value ?? ""
    )
        .replaceAll(
            "&",
            "&amp;"
        )
        .replaceAll(
            "<",
            "&lt;"
        )
        .replaceAll(
            ">",
            "&gt;"
        )
        .replaceAll(
            '"',
            "&quot;"
        )
        .replaceAll(
            "'",
            "&#039;"
        );

}


function escapeAttribute(
    value
) {

    return escapeHTML(
        value
    );

}


// ============================================================
// SVG ICONS
// ============================================================

function iconCheck() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path d="m5 12 4 4L19 6"/>
        </svg>
    `;

}


function iconTrash() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path d="M4 7h16"/>
            <path d="M10 11v6M14 11v6"/>
            <path d="M6 7l1 13h10l1-13"/>
            <path d="M9 7V4h6v3"/>
        </svg>
    `;

}


function iconTag() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path
                d="M20.5 13.5 13.5 20.5
                   a2 2 0 0 1-2.8 0L4 13.8V4h9.8l6.7 6.7
                   a2 2 0 0 1 0 2.8Z"
            />
            <circle
                cx="8.5"
                cy="8.5"
                r="1"
            />
        </svg>
    `;

}


function iconEdit() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path
                d="M12 20h9"
            />
            <path
                d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4Z"
            />
        </svg>
    `;

}


function iconCopy() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <rect
                x="9"
                y="9"
                width="11"
                height="11"
                rx="2"
            />
            <path
                d="M5 15H4a2 2 0 0 1-2-2V4
                   a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
            />
        </svg>
    `;

}


function iconExternal() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path
                d="M14 3h7v7"
            />
            <path
                d="M10 14 21 3"
            />
            <path
                d="M21 14v5a2 2 0 0 1-2 2H5
                   a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5"
            />
        </svg>
    `;

}


function iconAlert() {

    return `
        <svg
            class="h-3.5 w-3.5 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path
                d="m12 3 9 17H3L12 3Z"
            />
            <path
                d="M12 9v4"
            />
            <path
                d="M12 16h.01"
            />
        </svg>
    `;

}