(() => {

    'use strict';


    // ============================================================
    // CONFIG
    // ============================================================

    const DATA_URL =
        '../data/collections.json';


    const PAGE_SIZE =
        10;


    // ============================================================
    // STATE
    // ============================================================

    const state = {

        collection: null,

        endpoints: [],

        filtered: [],

        page: 1,

        search: '',

        method: 'all',

        tag: '',

        selected: new Set(),

        selectedEndpoint: null

    };


    // ============================================================
    // INIT
    // ============================================================

    document.addEventListener(
        'DOMContentLoaded',
        init
    );


    async function init() {

        wireNavigation();

        wireToolbar();

        wireTable();

        wireWindows();

        await loadCollection();

    }


    // ============================================================
    // LOAD COLLECTION
    // ============================================================

    async function loadCollection() {

        const params =
            new URLSearchParams(
                window.location.search
            );


        const folderId =
            params.get(
                'folder'
            );


        if (!folderId) {

            renderError(
                'No collection was selected.'
            );

            return;

        }


        try {

            const response =
                await fetch(
                    DATA_URL
                );


            if (!response.ok) {

                throw new Error(
                    `HTTP ${response.status}`
                );

            }


            const data =
                await response.json();


            const folders =
                Array.isArray(
                    data.folders
                )
                    ? data.folders
                    : [];


            state.collection =
                folders.find(
                    folder =>
                        Number(folder.id) ===
                        Number(folderId)
                );


            if (
                !state.collection
            ) {

                renderError(
                    'The requested collection could not be found.'
                );

                return;

            }


            state.endpoints =
                Array.isArray(
                    state.collection.endpoints
                )
                    ? state.collection.endpoints
                    : [];


            state.filtered =
                [
                    ...state.endpoints
                ];


            renderCollectionInfo();

            renderTagFilter();

            renderStats();

            render();

        } catch (error) {

            console.error(
                'Failed to load collection:',
                error
            );


            renderError(
                'Unable to load collection data.'
            );

        }

    }


    // ============================================================
    // FILTERING
    // ============================================================

    function applyFilters() {

        const term =
            state.search
                .trim()
                .toLowerCase();


        state.filtered =
            state.endpoints.filter(
                endpoint => {

                    const path =
                        String(
                            endpoint.endpoint ||
                            ''
                        );


                    const method =
                        String(
                            endpoint.method ||
                            ''
                        ).toUpperCase();


                    const annotation =
                        String(
                            endpoint.annotation ||
                            ''
                        );


                    const tags =
                        Array.isArray(
                            endpoint.tags
                        )
                            ? endpoint.tags
                            : [];


                    const matchesSearch =
                        !term ||

                        path
                            .toLowerCase()
                            .includes(term) ||

                        method
                            .toLowerCase()
                            .includes(term) ||

                        annotation
                            .toLowerCase()
                            .includes(term) ||

                        tags.some(
                            tag =>
                                String(tag)
                                    .toLowerCase()
                                    .includes(term)
                        );


                    const matchesMethod =
                        state.method ===
                            'all' ||

                        method ===
                            state.method;


                    const matchesTag =
                        !state.tag ||

                        tags.includes(
                            state.tag
                        );


                    return (
                        matchesSearch &&
                        matchesMethod &&
                        matchesTag
                    );

                }
            );


        state.page =
            Math.min(
                state.page,
                getTotalPages()
            );


        render();

    }


    function getTotalPages() {

        return Math.max(
            1,
            Math.ceil(
                state.filtered.length /
                PAGE_SIZE
            )
        );

    }


    function getPageItems() {

        const start =
            (state.page - 1) *
            PAGE_SIZE;


        return state.filtered.slice(
            start,
            start + PAGE_SIZE
        );

    }


    // ============================================================
    // RENDER
    // ============================================================

    function render() {

        renderTable();

        renderPagination();

        updateSelectionUI();

    }


    // ============================================================
    // TABLE
    // ============================================================

    function renderTable() {

        const tbody =
            document.getElementById(
                'endpointTableBody'
            );


        tbody.innerHTML =
            '';


        const items =
            getPageItems();


        if (!items.length) {

            tbody.innerHTML = `

                <tr>

                    <td
                        colspan="6"
                        class="px-4 py-16
                               text-center"
                    >

                        <p
                            class="text-sm
                                   text-slate-400"
                        >
                            No endpoints found
                        </p>

                        <p
                            class="mt-1
                                   text-[11px]
                                   text-slate-600"
                        >
                            Try changing the
                            current filters.
                        </p>

                    </td>

                </tr>

            `;


            return;

        }


        items.forEach(
            endpoint => {

                const row =
                    createEndpointRow(
                        endpoint
                    );


                tbody.appendChild(
                    row
                );

            }
        );


        updateSelectAllState();

    }


    function createEndpointRow(
        endpoint
    ) {

        const row =
            document.createElement(
                'tr'
            );


        const endpointId =
            getEndpointId(
                endpoint
            );


        const selected =
            state.selected.has(
                endpointId
            );


        const method =
            String(
                endpoint.method ||
                ''
            ).toUpperCase();


        const tags =
            Array.isArray(
                endpoint.tags
            )
                ? endpoint.tags
                : [];


        const annotation =
            endpoint.annotation ||
            '—';


        row.dataset.id =
            endpointId;


        row.className =
            [
                'border-b',
                'border-slate-800',
                'cursor-pointer',
                'transition-colors',
                'hover:bg-slate-800/60',

                selected
                    ? 'bg-cyan-500/[0.055]'
                    : ''
            ]
                .filter(Boolean)
                .join(' ');


        row.innerHTML = `

            <td class="px-3 py-2">

                <input
                    type="checkbox"
                    class="endpoint-select
                           h-3.5 w-3.5
                           accent-cyan-400"
                    ${selected ? 'checked' : ''}
                >

            </td>


            <td
                class="px-3 py-2
                       font-semibold
                       ${methodClass(method)}"
            >
                ${escapeHtml(method || '—')}
            </td>


            <td class="px-3 py-2">

                <div
                    class="flex items-center
                           gap-2"
                >

                    <span
                        class="font-mono
                               text-[11px]
                               text-slate-300"
                    >
                        ${renderEndpointPath(
                            endpoint.endpoint
                        )}
                    </span>

                </div>

            </td>


            <td class="px-3 py-2">

                <div
                    class="flex flex-wrap
                           gap-1"
                >

                    ${
                        tags.length
                            ? tags
                                .map(
                                    tag => `

                                        <button
                                            type="button"
                                            class="endpoint-tag
                                                   rounded
                                                   bg-sky-900/40
                                                   px-1.5 py-0.5
                                                   text-[10px]
                                                   text-sky-300
                                                   hover:bg-sky-500/20"
                                            data-tag="${escapeAttr(
                                                tag
                                            )}"
                                        >
                                            ${escapeHtml(
                                                tag
                                            )}
                                        </button>

                                    `
                                )
                                .join('')

                            : `
                                <span
                                    class="text-[10px]
                                           text-slate-600"
                                >
                                    —
                                </span>
                              `
                    }

                </div>

            </td>


            <td
                class="px-3 py-2
                       text-center"
            >

                <span
                    class="inline-flex
                           min-w-6
                           justify-center
                           rounded-md
                           bg-slate-800
                           px-1.5 py-0.5
                           text-[10px]
                           text-slate-400"
                >
                    ${getQPCount(endpoint)}
                </span>

            </td>


            <td
                class="px-3 py-2
                       text-[11px]
                       text-slate-500"
            >

                <span
                    class="line-clamp-2"
                    title="${escapeAttr(
                        annotation
                    )}"
                >
                    ${escapeHtml(
                        annotation
                    )}
                </span>

            </td>

        `;


        // --------------------------------------------------------
        // CHECKBOX
        // --------------------------------------------------------

        const checkbox =
            row.querySelector(
                '.endpoint-select'
            );


        checkbox.addEventListener(
            'click',
            event =>
                event.stopPropagation()
        );


        checkbox.addEventListener(
            'change',
            () => {

                if (
                    checkbox.checked
                ) {

                    state.selected.add(
                        endpointId
                    );

                } else {

                    state.selected.delete(
                        endpointId
                    );

                }


                updateSelectionUI();

                updateSelectAllState();

                renderTable();

            }
        );


        // --------------------------------------------------------
        // TAG
        // --------------------------------------------------------

        row.querySelectorAll(
            '.endpoint-tag'
        ).forEach(
            button => {

                button.addEventListener(
                    'click',
                    event => {

                        event.stopPropagation();


                        state.tag =
                            button.dataset.tag;


                        const select =
                            document.getElementById(
                                'tagFilter'
                            );


                        select.value =
                            state.tag;


                        state.page =
                            1;


                        applyFilters();

                    }
                );

            }
        );


        // --------------------------------------------------------
        // ROW CLICK
        // --------------------------------------------------------

        row.addEventListener(
            'click',
            event => {

                if (
                    event.target.closest(
                        'button,input'
                    )
                ) {

                    return;

                }


                selectEndpoint(
                    endpoint
                );

            }
        );


        return row;

    }


    // ============================================================
    // ENDPOINT SELECTION
    // ============================================================

    function selectEndpoint(
        endpoint
    ) {

        state.selectedEndpoint =
            endpoint;


        renderEndpointDetails(
            endpoint
        );


        openWindow(
            'requestWindow'
        );


        openWindow(
            'responseWindow'
        );


        renderTable();

    }


    function renderEndpointDetails(
        endpoint
    ) {

        const request =
            endpoint.request ||
            {};


        const response =
            endpoint.response ||
            {};


        document.getElementById(
            'requestHeaders'
        ).textContent =
            formatData(
                request.headers ||
                {}
            );


        document.getElementById(
            'requestQuery'
        ).textContent =
            formatData(
                request.query ||
                request.queryParams ||
                {}
            );


        document.getElementById(
            'requestBody'
        ).textContent =
            formatData(
                request.body ??
                'Not available'
            );


        document.getElementById(
            'responseStatus'
        ).textContent =
            response.status
                ? String(
                    response.status
                )
                : 'Prototype';


        document.getElementById(
            'responseBody'
        ).textContent =
            formatData(
                response.body ??
                response ??
                {
                    endpoint:
                        endpoint.endpoint,

                    message:
                        'No response payload defined.'
                }
            );

    }


    // ============================================================
    // STATS
    // ============================================================

    function renderStats() {

        const endpoints =
            state.endpoints;


        const tags =
            new Set();


        let get =
            0;


        let write =
            0;


        endpoints.forEach(
            endpoint => {

                const method =
                    String(
                        endpoint.method ||
                        ''
                    ).toUpperCase();


                if (
                    method ===
                    'GET'
                ) {

                    get++;

                }


                if (
                    [
                        'POST',
                        'PUT',
                        'PATCH',
                        'DELETE'
                    ].includes(
                        method
                    )
                ) {

                    write++;

                }


                const endpointTags =
                    Array.isArray(
                        endpoint.tags
                    )
                        ? endpoint.tags
                        : [];


                endpointTags.forEach(
                    tag =>
                        tags.add(tag)
                );

            }
        );


        document.getElementById(
            'endpointCount'
        ).textContent =
            endpoints.length;


        document.getElementById(
            'tagCount'
        ).textContent =
            tags.size;


        document.getElementById(
            'getCount'
        ).textContent =
            get;


        document.getElementById(
            'writeCount'
        ).textContent =
            write;

    }


    function renderCollectionInfo() {

        const folder =
            state.collection;


        document.getElementById(
            'collectionTitle'
        ).textContent =
            folder.name;


        document.getElementById(
            'collectionSubtitle'
        ).textContent =
            [
                capitalize(
                    folder.purpose
                ),
                folder.datatype || ''
            ]
                .filter(Boolean)
                .join(' · ');

    }


    // ============================================================
    // TAG FILTER
    // ============================================================

    function renderTagFilter() {

        const select =
            document.getElementById(
                'tagFilter'
            );


        const tags =
            new Set();


        state.endpoints.forEach(
            endpoint => {

                const endpointTags =
                    Array.isArray(
                        endpoint.tags
                    )
                        ? endpoint.tags
                        : [];


                endpointTags.forEach(
                    tag =>
                        tags.add(tag)
                );

            }
        );


        select.innerHTML =
            '<option value="">Tags</option>';


        [...tags]
            .sort()
            .forEach(
                tag => {

                    const option =
                        document.createElement(
                            'option'
                        );


                    option.value =
                        tag;


                    option.textContent =
                        tag;


                    select.appendChild(
                        option
                    );

                }
            );

    }


    // ============================================================
    // PAGINATION
    // ============================================================

    function renderPagination() {

        const total =
            state.filtered.length;


        const start =
            total
                ? (
                    (state.page - 1) *
                    PAGE_SIZE
                ) + 1
                : 0;


        const end =
            Math.min(
                state.page *
                    PAGE_SIZE,
                total
            );


        document.getElementById(
            'rangeStart'
        ).textContent =
            start;


        document.getElementById(
            'rangeEnd'
        ).textContent =
            end;


        document.getElementById(
            'totalEndpoints'
        ).textContent =
            total;


        const totalPages =
            getTotalPages();


        const container =
            document.getElementById(
                'pageNumbers'
            );


        container.innerHTML =
            '';


        getPageRange(
            state.page,
            totalPages
        ).forEach(
            page => {

                if (
                    page === '...'
                ) {

                    const span =
                        document.createElement(
                            'span'
                        );


                    span.className =
                        'px-1 text-slate-600';


                    span.textContent =
                        '…';


                    container.appendChild(
                        span
                    );


                    return;

                }


                const button =
                    document.createElement(
                        'button'
                    );


                button.type =
                    'button';


                button.textContent =
                    page;


                button.className =
                    page === state.page

                        ? `
                            min-w-7 rounded-md
                            bg-cyan-500
                            px-2 py-1
                            text-[11px] text-white
                          `

                        : `
                            min-w-7 rounded-md
                            border border-slate-700
                            px-2 py-1
                            text-[11px]
                            text-slate-400
                            hover:bg-slate-800
                            hover:text-white
                          `;


                button.addEventListener(
                    'click',
                    () => {

                        state.page =
                            page;

                        render();

                    }
                );


                container.appendChild(
                    button
                );

            }
        );


        document.getElementById(
            'prevPage'
        ).disabled =
            state.page <= 1;


        document.getElementById(
            'nextPage'
        ).disabled =
            state.page >= totalPages;

    }


    function getPageRange(
        current,
        total
    ) {

        if (
            total <= 7
        ) {

            return Array.from(
                {
                    length: total
                },
                (_, i) =>
                    i + 1
            );

        }


        const result = [
            1
        ];


        if (
            current > 4
        ) {

            result.push(
                '...'
            );

        }


        for (
            let page =
                Math.max(
                    2,
                    current - 1
                );

            page <=
                Math.min(
                    total - 1,
                    current + 1
                );

            page++
        ) {

            result.push(
                page
            );

        }


        if (
            current <
            total - 3
        ) {

            result.push(
                '...'
            );

        }


        result.push(
            total
        );


        return result;

    }


    // ============================================================
    // SELECTION
    // ============================================================

    function updateSelectionUI() {

        const count =
            state.selected.size;


        const label =
            document.getElementById(
                'selectionLabel'
            );


        const clear =
            document.getElementById(
                'clearEndpointSelection'
            );


        if (
            count
        ) {

            label.textContent =
                `${count} selected`;


            label.classList.remove(
                'hidden'
            );


            clear.classList.remove(
                'hidden'
            );

        } else {

            label.classList.add(
                'hidden'
            );


            clear.classList.add(
                'hidden'
            );

        }


        updateSelectAllState();

    }


    function updateSelectAllState() {

        const selectAll =
            document.getElementById(
                'selectAllEndpoints'
            );


        const items =
            getPageItems();


        const selectedCount =
            items.filter(
                endpoint =>
                    state.selected.has(
                        getEndpointId(
                            endpoint
                        )
                    )
            ).length;


        selectAll.checked =
            items.length > 0 &&
            selectedCount ===
                items.length;


        selectAll.indeterminate =
            selectedCount > 0 &&
            selectedCount <
                items.length;

    }


    // ============================================================
    // TOOLBAR
    // ============================================================

    function wireToolbar() {

        document.getElementById(
            'endpointSearch'
        ).addEventListener(
            'input',
            event => {

                state.search =
                    event.target.value;


                state.page =
                    1;


                applyFilters();

            }
        );


        document.getElementById(
            'methodFilter'
        ).addEventListener(
            'change',
            event => {

                state.method =
                    event.target.value;


                state.page =
                    1;


                applyFilters();

            }
        );


        document.getElementById(
            'tagFilter'
        ).addEventListener(
            'change',
            event => {

                state.tag =
                    event.target.value;


                state.page =
                    1;


                applyFilters();

            }
        );


        document.getElementById(
            'resetEndpointFilters'
        ).addEventListener(
            'click',
            resetFilters
        );


        document.getElementById(
            'clearEndpointSelection'
        ).addEventListener(
            'click',
            () => {

                state.selected.clear();

                render();

            }
        );


        document.getElementById(
            'selectAllEndpoints'
        ).addEventListener(
            'change',
            event => {

                getPageItems()
                    .forEach(
                        endpoint => {

                            const id =
                                getEndpointId(
                                    endpoint
                                );


                            if (
                                event.target.checked
                            ) {

                                state.selected.add(
                                    id
                                );

                            } else {

                                state.selected.delete(
                                    id
                                );

                            }

                        }
                    );


                render();

            }
        );


        document.getElementById(
            'prevPage'
        ).addEventListener(
            'click',
            () => {

                if (
                    state.page > 1
                ) {

                    state.page--;

                    render();

                }

            }
        );


        document.getElementById(
            'nextPage'
        ).addEventListener(
            'click',
            () => {

                if (
                    state.page <
                    getTotalPages()
                ) {

                    state.page++;

                    render();

                }

            }
        );

    }


    function resetFilters() {

        state.search =
            '';

        state.method =
            'all';

        state.tag =
            '';

        state.page =
            1;


        document.getElementById(
            'endpointSearch'
        ).value =
            '';


        document.getElementById(
            'methodFilter'
        ).value =
            'all';


        document.getElementById(
            'tagFilter'
        ).value =
            '';


        applyFilters();

    }


    // ============================================================
    // TABLE
    // ============================================================

    function wireTable() {

        /*
         * The table itself is rendered dynamically.
         * Row-level events are attached when rows are created.
         */

    }


    // ============================================================
    // REQUEST / RESPONSE WINDOWS
    // ============================================================

    function wireWindows() {

        document
            .querySelectorAll(
                '[data-window-close]'
            )
            .forEach(
                button => {

                    button.addEventListener(
                        'click',
                        () => {

                            const type =
                                button.dataset
                                    .windowClose;


                            closeWindow(
                                type ===
                                    'request'
                                    ? 'requestWindow'
                                    : 'responseWindow'
                            );

                        }
                    );

                }
            );

    }


    function openWindow(
        id
    ) {

        document.getElementById(
            id
        )?.classList.remove(
            'hidden'
        );

    }


    function closeWindow(
        id
    ) {

        document.getElementById(
            id
        )?.classList.add(
            'hidden'
        );

    }


    // ============================================================
    // NAVIGATION
    // ============================================================

    function wireNavigation() {

        document.getElementById(
            'backToCollections'
        )?.addEventListener(
            'click',
            () => {

                window.location.href =
                    './collection_view.html';

            }
        );


        document
            .querySelectorAll(
                '[data-target]'
            )
            .forEach(
                button => {

                    button.addEventListener(
                        'click',
                        () => {

                            window.location.href =
                                button.dataset.target;

                        }
                    );

                }
            );

    }


    // ============================================================
    // ERROR
    // ============================================================

    function renderError(
        message
    ) {

        document.getElementById(
            'collectionTitle'
        ).textContent =
            'Collection Data View';


        document.getElementById(
            'collectionSubtitle'
        ).textContent =
            message;


        document.getElementById(
            'endpointTableBody'
        ).innerHTML = `

            <tr>

                <td
                    colspan="6"
                    class="px-4 py-20
                           text-center"
                >

                    <p
                        class="text-sm
                               text-rose-400"
                    >
                        ${escapeHtml(
                            message
                        )}
                    </p>

                    <button
                        type="button"
                        class="mt-4 rounded-md
                               bg-slate-800
                               px-3 py-1.5
                               text-xs
                               text-slate-300
                               hover:bg-slate-700"
                        onclick="
                            window.location.href =
                            './collection_view.html'
                        "
                    >
                        Back to Collections
                    </button>

                </td>

            </tr>

        `;

    }


    // ============================================================
    // HELPERS
    // ============================================================

    function getEndpointId(
        endpoint
    ) {

        /*
         * Prefer the endpoint's own ID.
         * Fall back to the endpoint path so
         * older prototype data still works.
         */

        return String(
            endpoint.id ??
            endpoint.endpoint ??
            Math.random()
        );

    }


    function getQPCount(
        endpoint
    ) {

        if (
            Array.isArray(
                endpoint.queryParams
            )
        ) {

            return endpoint.queryParams.length;

        }


        if (
            Array.isArray(
                endpoint.qp
            )
        ) {

            return endpoint.qp.length;

        }


        if (
            endpoint.queryParams &&
            typeof endpoint.queryParams ===
                'object'
        ) {

            return Object.keys(
                endpoint.queryParams
            ).length;

        }


        if (
            endpoint.qp &&
            typeof endpoint.qp ===
                'object'
        ) {

            return Object.keys(
                endpoint.qp
            ).length;

        }


        return Number(
            endpoint.qpCount ||
            0
        );

    }


    function renderEndpointPath(
        path
    ) {

        const segments =
            String(
                path || ''
            )
                .split('/')
                .filter(Boolean);


        if (!segments.length) {

            return '—';

        }


        return segments
            .map(
                (segment, index) => {

                    return `

                        <span
                            class="endpoint-segment
                                   rounded px-0.5
                                   transition
                                   hover:bg-cyan-500/10
                                   hover:text-cyan-300"
                            data-segment="${escapeAttr(
                                segment
                            )}"
                        >
                            ${
                                index === 0
                                    ? '/' +
                                      escapeHtml(
                                          segment
                                      )
                                    : '/' +
                                      escapeHtml(
                                          segment
                                      )
                            }
                        </span>

                    `;

                }
            )
            .join('');

    }


    function methodClass(
        method
    ) {

        return {

            GET:
                'text-emerald-400',

            POST:
                'text-sky-400',

            PUT:
                'text-amber-400',

            PATCH:
                'text-violet-400',

            DELETE:
                'text-rose-400'

        }[
            method
        ] || 'text-slate-300';

    }


    function capitalize(
        value
    ) {

        const text =
            String(
                value || ''
            );


        return text
            ? text.charAt(0).toUpperCase() +
              text.slice(1)
            : '';

    }


    function formatData(
        value
    ) {

        if (
            typeof value ===
            'string'
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


    function escapeHtml(
        value
    ) {

        const div =
            document.createElement(
                'div'
            );


        div.textContent =
            String(
                value ?? ''
            );


        return div.innerHTML;

    }


    function escapeAttr(
        value
    ) {

        return escapeHtml(
            value
        )
            .replace(
                /"/g,
                '&quot;'
            );

    }

})();