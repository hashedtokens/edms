(() => {
    'use strict';

    // ============================================================
    // CONFIG
    // ============================================================

    const DATA_URL = '../data/repoview.json';

    const PAGE_SIZE = 10;

    const CRUD_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];

    let nextId = 10000;


    // ============================================================
    // STATE
    // ============================================================

    const state = {

        repos: [],

        filtered: [],

        page: 1,

        search: '',

        crud: 'all',

        tags: new Set(),

        segments: new Set(),

        selected: new Set(),

        activeRowId: null,

        sidebarCollapsed: false

    };

    let sizeUnits = {};

    const API_BASE = 'http://localhost:3001';


    // ============================================================
    // INIT
    // ============================================================

    document.addEventListener('DOMContentLoaded', init);


    async function init() {

        await loadData();

        wireToolbar();

        wireSidebar();

        wireSelection();

        wireSidebarTabs();

        wireModal();

        renderSidebar();

        applyFilters();

    }


    // ============================================================
    // DATA
    // ============================================================

    async function loadData() {

        try {
            if (!window.EdmsAPI) {
                console.warn('EdmsAPI not loaded yet');
                return;
            }

            const response = await window.EdmsAPI.listRepoviews();

            if (!response.ok) throw new Error(`HTTP ${response.status}`);

            const data = response.data;
            let list = [];
            if (data && Array.isArray(data.items)) {
                list = data.items;
            } else if (Array.isArray(data)) {
                list = data;
            }

            // Map the backend items to the structure expected by the frontend table
            state.repos = list.map((item, index) => {
                return {
                    id: nextId + index, // Generate temporary ID since backend doesn't provide one
                    name: item.name,
                    annotation: item.annotation || '',
                    dateCreated: item.created_at ? item.created_at.split(' ')[0] : '',
                    dataSizeBytes: 0,
                    eidCount: item.endpoint_count || 0,
                    segments: 0,
                    dataTags: [],
                    tags: [], // Tags aren't attached to the list view in the backend yet
                    qpCount: 0,
                    indexLists: 0
                };
            });


            nextId += state.repos.length;

            state.repos.forEach(item => {
                sizeUnits[item.id] = 'MB';
            });

        } catch (error) {

            console.error('Failed to load Repo View data:', error);

            state.repos = [];

        }

    }


    // ============================================================
    // FILTERING
    // ============================================================

    function applyFilters() {

        const term = state.search.trim().toLowerCase();

        state.filtered = state.repos.filter(repo => {

            const repoTags = Array.isArray(repo.tags) ? repo.tags : [];
            const dataTags = Array.isArray(repo.dataTags) ? repo.dataTags : [];

            const matchesSearch =
                !term ||
                String(repo.name || '').toLowerCase().includes(term) ||
                String(repo.annotation || '').toLowerCase().includes(term) ||
                repoTags.some(tag => String(tag).toLowerCase().includes(term)) ||
                dataTags.some(tag => String(tag).toLowerCase().includes(term));

            const matchesCrud =
                state.crud === 'all' ||
                Number(repo.crud?.[state.crud] || 0) > 0;

            const matchesTags =
                state.tags.size === 0 ||
                [...state.tags].every(tag => repoTags.includes(tag));

            const repoSegments = getRepoSegments(repo);

            const matchesSegments =
                state.segments.size === 0 ||
                [...state.segments].every(segment => repoSegments.includes(segment));

            return matchesSearch && matchesCrud && matchesTags && matchesSegments;

        });

        const totalPages = getTotalPages();

        if (state.page > totalPages) state.page = totalPages;

        render();

    }


    function getTotalPages() {

        return Math.max(1, Math.ceil(state.filtered.length / PAGE_SIZE));

    }


    function currentPageItems() {

        const start = (state.page - 1) * PAGE_SIZE;

        return state.filtered.slice(start, start + PAGE_SIZE);

    }


    // ============================================================
    // SEGMENTS
    //
    // Falls back gracefully depending on what a given repo entry
    // actually provides:
    //   1) repo.endpoints (full endpoint list)
    //   2) repo.segments (array of segment strings)
    //   3) repo.segmentCount (precomputed number, non-filterable)
    //   4) [] if none of the above exist
    // ============================================================

    function getEndpointSegments(endpointPath) {

        return String(endpointPath || '').split('/').filter(Boolean);

    }


    function getRepoSegments(repo) {

        if (Array.isArray(repo.endpoints)) {

            const segments = new Set();

            repo.endpoints.forEach(endpoint => {
                getEndpointSegments(endpoint.endpoint).forEach(segment => segments.add(segment));
            });

            return [...segments];

        }

        if (Array.isArray(repo.segments)) {

            return repo.segments;

        }

        return [];

    }


    function getRepoSegmentCount(repo) {

        const segments = getRepoSegments(repo);

        if (segments.length) return segments.length;

        if (typeof repo.segmentCount === 'number') return repo.segmentCount;

        return 0;

    }


    // ============================================================
    // RENDER
    // ============================================================

    function render() {

        renderTable();

        renderPagination();

        renderSelectionUI();

        updateStats();

        syncSidebar();

        syncSidebar();

        renderActiveRowData();

    }


    // ============================================================
    // TABLE
    // ============================================================

    function renderTable() {

        const tbody = document.getElementById('repoTableBody');

        if (!tbody) return;

        tbody.innerHTML = '';

        const items = currentPageItems();

        if (items.length === 0) {

            tbody.innerHTML = `
                <tr>
                    <td colspan="15" class="px-4 py-16 text-center">
                        <p class="text-sm font-medium text-slate-500 dark:text-slate-400">No repo views found</p>
                        <p class="mt-1 text-xs text-slate-400 dark:text-slate-600">Try changing the current search or filters.</p>
                    </td>
                </tr>
            `;

            updateSelectAllState();

            return;

        }

        items.forEach(repo => {
            tbody.appendChild(createRepoRow(repo));
        });

        updateSelectAllState();

    }


    function createRepoRow(repo) {

        const row = document.createElement('tr');

        const id = Number(repo.id);

        const isSelected = state.selected.has(id);
        const isActive = state.activeRowId === id;

        row.dataset.id = String(id);

        row.className = [
            'repo-row',
            'border-b',
            'border-slate-200',
            'dark:border-slate-800',
            'transition-colors',
            'duration-100',
            'cursor-pointer',
            'hover:bg-slate-100',
            'dark:hover:bg-slate-800/60',
            isSelected ? 'bg-cyan-500/[0.045]' : '',
            isActive ? 'bg-slate-200 dark:bg-slate-800/80 ring-1 ring-inset ring-cyan-500/50' : ''
        ].filter(Boolean).join(' ');

        const dataTagsCount = (repo.dataTags || []).length;
        const segmentCount = getRepoSegmentCount(repo);

        row.innerHTML = `

            <!-- Select -->
            <td class="px-2 py-2 align-middle">
                <input type="checkbox" class="repo-select h-3.5 w-3.5 accent-cyan-400" ${isSelected ? 'checked' : ''}>
            </td>

            <!-- Folder Name -->
            <td class="px-2 py-2 font-medium text-slate-800 dark:text-slate-200">
                <button
                    type="button"
                    class="repo-name-btn truncate text-left hover:text-cyan-600 dark:hover:text-cyan-400"
                    title="${escapeAttr(repo.name)}">
                    ${escapeHtml(repo.name)}
                </button>
            </td>

            <!-- Annotation -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="repo-annotation-btn block max-w-xs truncate text-left text-[11px] text-slate-500 hover:text-cyan-600 dark:hover:text-cyan-400"
                    title="${escapeAttr(repo.annotation || '')}">
                    ${escapeHtml(repo.annotation || '—')}
                </button>
            </td>

            <!-- Tags -->
            <td class="px-2 py-2">
                <div class="flex flex-wrap gap-1">
                    ${renderRepoTags(repo)}
                </div>
            </td>

            <!-- Date Created -->
            <td class="px-2 py-2 text-slate-500 whitespace-nowrap">
                ${escapeHtml(repo.dateCreated || '—')}
            </td>

            <!-- Size -->
            <td class="px-2 py-2">
                <button type="button" class="repo-size-btn text-slate-800 dark:text-slate-300 hover:text-cyan-600 dark:hover:text-cyan-400">
                    ${formatSize(repo.dataSizeBytes, sizeUnits[repo.id])}
                </button>
            </td>

            <!-- GET -->
            <td class="px-1 py-2 text-center border-l border-slate-200 dark:border-slate-800">${crudCountBadge(repo.crud?.GET, 'text-emerald-600 dark:text-emerald-400')}</td>
            <!-- POST -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.POST, 'text-sky-600 dark:text-sky-400')}</td>
            <!-- PUT -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.PUT, 'text-amber-600 dark:text-amber-400')}</td>
            <!-- PATCH -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.PATCH, 'text-violet-600 dark:text-violet-400')}</td>
            <!-- DELETE -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.DELETE, 'text-rose-600 dark:text-rose-400')}</td>

            <!-- EID Count -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${repo.eidCount ?? 0}</td>

            <!-- Segments -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${segmentCount}</td>

            <!-- Data Tags -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="repo-datatags-btn rounded-md border border-slate-300 dark:border-slate-700 bg-slate-200 dark:bg-slate-800 px-2 py-0.5 text-[11px] text-slate-800 dark:text-slate-300 transition hover:border-cyan-500/60 hover:text-cyan-600 dark:hover:text-cyan-400">
                    ${dataTagsCount} tag${dataTagsCount === 1 ? '' : 's'}
                </button>
            </td>

            <!-- QP Count -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${repo.qpCount ?? 0}</td>

            <!-- Index Lists -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${repo.indexLists ?? 0}</td>


        `;


        // --------------------------------------------------------
        // Click row -> make active
        // --------------------------------------------------------

        row.addEventListener('click', (event) => {
            if (event.target.closest('button, input, select, a')) return;
            if (state.activeRowId === id) {
                state.activeRowId = null;
            } else {
                state.activeRowId = id;
            }
            renderTable();
            renderActiveRowData();
        });

        // --------------------------------------------------------
        // Checkbox
        // --------------------------------------------------------

        const checkbox = row.querySelector('.repo-select');

        checkbox.addEventListener('click', event => event.stopPropagation());

        checkbox.addEventListener('change', () => {

            if (checkbox.checked) state.selected.add(id);
            else state.selected.delete(id);

            renderSelectionUI();
            updateSelectAllState();
            renderTable();

        });


        // --------------------------------------------------------
        // Folder name -> edit modal
        // --------------------------------------------------------

        row.querySelector('.repo-name-btn').addEventListener('click', event => {
            event.stopPropagation();
            openEditRepoModal(id);
        });


        // --------------------------------------------------------
        // Annotation -> annotation modal
        // --------------------------------------------------------

        row.querySelector('.repo-annotation-btn').addEventListener('click', event => {
            event.stopPropagation();
            openAnnotationModal(id);
        });


        // --------------------------------------------------------
        // Size -> cycle unit
        // --------------------------------------------------------

        row.querySelector('.repo-size-btn').addEventListener('click', event => {
            event.stopPropagation();
            cycleSizeUnit(id);
        });


        // --------------------------------------------------------
        // Data tags -> popup list
        // --------------------------------------------------------

        row.querySelector('.repo-datatags-btn').addEventListener('click', event => {
            event.stopPropagation();
            openDataTagsModal(id);
        });


        // --------------------------------------------------------
        // Tags -> filter
        // --------------------------------------------------------

        row.querySelectorAll('.repo-tag').forEach(button => {

            button.addEventListener('click', event => {

                event.stopPropagation();

                const tag = button.dataset.tag;

                state.tags.clear();
                state.tags.add(tag);
                state.page = 1;

                applyFilters();

            });

        });


        return row;

    }


    function renderRepoTags(repo) {

        const tags = Array.isArray(repo.tags) ? repo.tags : [];

        if (!tags.length) {
            return `<span class="text-[10px] text-slate-400 dark:text-slate-600">—</span>`;
        }

        return tags.map(tag => `
            <button
                type="button"
                class="repo-tag inline-flex rounded bg-sky-100 dark:bg-sky-900/40 px-1.5 py-0.5 text-[10px] text-sky-700 dark:text-sky-300 transition hover:bg-sky-200 dark:hover:bg-sky-500/20 hover:text-sky-800 dark:hover:text-sky-200"
                data-tag="${escapeAttr(tag)}">
                ${escapeHtml(tag)}
            </button>
        `).join('');

    }


    function crudCountBadge(count, color) {

        const value = Number(count || 0);

        return `
            <span class="inline-flex min-w-6 items-center justify-center rounded-md bg-slate-200 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] ${value > 0 ? color : 'text-slate-400 dark:text-slate-600'}">
                ${value}
            </span>
        `;

    }


    // ============================================================
    // DATA TAGS POPUP
    // ============================================================

    function openDataTagsModal(id) {

        const repo = getRepo(id);

        if (!repo) return;

        const tags = repo.dataTags || [];

        openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">EQP Tags</h2>

            <div class="flex flex-wrap gap-2">
                ${tags.length
                ? tags.map(tag => `
                            <span class="rounded-md border border-slate-300 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 px-2.5 py-1 text-xs text-slate-800 dark:text-slate-300">
                                ${escapeHtml(tag)}
                            </span>
                        `).join('')
                : '<span class="text-xs text-slate-400 dark:text-slate-600">No EQP Tags.</span>'
            }
            </div>

            <div class="mt-4 flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">
                    Close
                </button>
            </div>

        `);

    }


    // ============================================================
    // PAGINATION
    // ============================================================

    function renderPagination() {

        const total = state.filtered.length;

        const start = total ? ((state.page - 1) * PAGE_SIZE) + 1 : 0;

        const end = Math.min(state.page * PAGE_SIZE, total);

        document.getElementById('repoRangeStart').textContent = start;
        document.getElementById('repoRangeEnd').textContent = end;
        document.getElementById('repoTotal').textContent = total;

        const totalPages = getTotalPages();

        const numbers = document.getElementById('repoPagination');

        numbers.innerHTML = '';

        getPageRange(state.page, totalPages).forEach(page => {

            if (page === '...') {

                const span = document.createElement('span');
                span.className = 'px-1 text-slate-400 dark:text-slate-600';
                span.textContent = '…';
                numbers.appendChild(span);
                return;

            }

            const button = document.createElement('button');
            button.type = 'button';
            button.textContent = page;

            button.className = page === state.page
                ? 'min-w-7 rounded-md bg-slate-900 dark:bg-cyan-500 px-2 py-1 text-[11px] text-white'
                : 'min-w-7 rounded-md border border-slate-300 dark:border-slate-700 px-2 py-1 text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-slate-200 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-white';

            button.addEventListener('click', () => {
                state.page = page;
                render();
            });

            numbers.appendChild(button);

        });

        document.getElementById('repoPrev').disabled = state.page <= 1;
        document.getElementById('repoNext').disabled = state.page >= totalPages;

    }


    function getPageRange(current, total) {

        if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);

        const pages = [1];

        if (current > 4) pages.push('...');

        const start = Math.max(2, current - 1);
        const end = Math.min(total - 1, current + 1);

        for (let page = start; page <= end; page++) pages.push(page);

        if (current < total - 3) pages.push('...');

        pages.push(total);

        return pages;

    }


    // ============================================================
    // SIDEBAR
    // ============================================================

    function wireSidebar() {

        document.getElementById('sidebarToggle')?.addEventListener('click', toggleSidebar);

    }


    function toggleSidebar() {

        const sidebar = document.getElementById('filterSidebar');
        const content = document.getElementById('sidebarContent');
        const title = document.getElementById('sidebarTitle');
        const icon = document.getElementById('sidebarToggleIcon');

        state.sidebarCollapsed = !state.sidebarCollapsed;

        if (state.sidebarCollapsed) {

            sidebar.classList.remove('w-64');
            sidebar.classList.add('w-10');
            content.classList.add('hidden');
            title.classList.add('hidden');
            icon.innerHTML = `<path d="m9 18 6-6-6-6"/>`;

        } else {

            sidebar.classList.remove('w-10');
            sidebar.classList.add('w-64');
            content.classList.remove('hidden');
            title.classList.remove('hidden');
            icon.innerHTML = `<path d="m15 18-6-6 6-6"/>`;

        }

    }


    function renderSidebar() {

        renderTags();
        renderSegments();
        updateStats();

    }


    function renderTags() {

        const container = document.getElementById('tagsContainer');

        if (!container) return;

        const counts = {};

        state.repos.forEach(repo => {
            (repo.tags || []).forEach(tag => {
                if (!tag) return;
                counts[tag] = (counts[tag] || 0) + 1;
            });
        });

        container.innerHTML = '';

        Object.entries(counts)
            .sort((a, b) => a[0].localeCompare(b[0]))
            .forEach(([tag, count]) => {

                const wrapper = document.createElement('label');
                wrapper.className = 'group flex cursor-pointer items-center gap-2';

                wrapper.innerHTML = `
                    <input type="checkbox" class="repo-tag-checkbox h-3.5 w-3.5 accent-cyan-400" data-tag="${escapeAttr(tag)}">
                    <button type="button" class="repo-tag-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-sky-500/10 hover:text-sky-700 dark:hover:text-sky-300" data-tag="${escapeAttr(tag)}">
                        ${escapeHtml(tag)} <span class="text-slate-400 dark:text-slate-600">(${count})</span>
                    </button>
                `;

                const checkbox = wrapper.querySelector('.repo-tag-checkbox');
                const button = wrapper.querySelector('.repo-tag-filter');

                checkbox.addEventListener('change', () => setTagFilter(tag, checkbox.checked));

                button.addEventListener('click', event => {
                    event.preventDefault();
                    const enabled = !state.tags.has(tag);
                    checkbox.checked = enabled;
                    setTagFilter(tag, enabled);
                });

                container.appendChild(wrapper);

            });

    }


    function renderSegments() {

        const container = document.getElementById('segmentsContainer');

        if (!container) return;

        const counts = {};

        state.repos.forEach(repo => {
            getRepoSegments(repo).forEach(segment => {
                counts[segment] = (counts[segment] || 0) + 1;
            });
        });

        container.innerHTML = '';

        if (Object.keys(counts).length === 0) {

            container.innerHTML = `<span class="text-[11px] text-slate-400 dark:text-slate-600">No segment data available.</span>`;
            return;

        }

        Object.entries(counts)
            .sort((a, b) => a[0].localeCompare(b[0]))
            .forEach(([segment, count]) => {

                const wrapper = document.createElement('label');
                wrapper.className = 'group flex cursor-pointer items-center gap-2';

                wrapper.innerHTML = `
                    <input type="checkbox" class="repo-segment-checkbox h-3.5 w-3.5 accent-cyan-400" data-segment="${escapeAttr(segment)}">
                    <button type="button" class="repo-segment-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-cyan-500/10 hover:text-cyan-700 dark:hover:text-cyan-300" data-segment="${escapeAttr(segment)}">
                        ${escapeHtml(segment)} <span class="text-slate-400 dark:text-slate-600">(${count})</span>
                    </button>
                `;

                const checkbox = wrapper.querySelector('.repo-segment-checkbox');
                const button = wrapper.querySelector('.repo-segment-filter');

                checkbox.addEventListener('change', () => setSegmentFilter(segment, checkbox.checked));

                button.addEventListener('click', event => {
                    event.preventDefault();
                    const enabled = !state.segments.has(segment);
                    checkbox.checked = enabled;
                    setSegmentFilter(segment, enabled);
                });

                container.appendChild(wrapper);

            });

    }


    function setTagFilter(tag, enabled) {

        if (enabled) state.tags.add(tag);
        else state.tags.delete(tag);

        state.page = 1;

        applyFilters();

    }


    function setSegmentFilter(segment, enabled) {

        if (enabled) state.segments.add(segment);
        else state.segments.delete(segment);

        state.page = 1;

        applyFilters();

    }


    function syncSidebar() {

        document.querySelectorAll('.repo-tag-checkbox').forEach(checkbox => {
            checkbox.checked = state.tags.has(checkbox.dataset.tag);
        });

        document.querySelectorAll('.repo-segment-checkbox').forEach(checkbox => {
            checkbox.checked = state.segments.has(checkbox.dataset.segment);
        });

    }


    let activeTab = 'repo';

    function wireSidebarTabs() {
        const tabRepo = document.getElementById('activeTabRepo');
        const tabEqp = document.getElementById('activeTabEqp');
        const contentRepo = document.getElementById('activeContentRepo');
        const contentEqp = document.getElementById('activeContentEqp');

        if (!tabRepo || !tabEqp) return;

        const activeClass = 'flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium bg-white dark:bg-slate-700 text-slate-900 dark:text-white shadow-sm transition';
        const inactiveClass = 'flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-slate-200 transition';

        function updateTabs() {
            if (activeTab === 'repo') {
                tabRepo.className = activeClass;
                tabEqp.className = inactiveClass;
                contentRepo.classList.remove('hidden');
                contentEqp.classList.add('hidden');
            } else {
                tabEqp.className = activeClass;
                tabRepo.className = inactiveClass;
                contentEqp.classList.remove('hidden');
                contentRepo.classList.add('hidden');
            }
        }

        tabRepo.addEventListener('click', () => { activeTab = 'repo'; updateTabs(); });
        tabEqp.addEventListener('click', () => { activeTab = 'eqp'; updateTabs(); });
    }

    function renderActiveRowData() {

        const section = document.getElementById('sidebarActiveSection');
        const contentRepo = document.getElementById('activeContentRepo');
        const contentEqp = document.getElementById('activeContentEqp');

        if (!section || !contentRepo || !contentEqp) return;

        if (!state.activeRowId) {
            section.classList.add('hidden');
            return;
        }

        const repo = getRepo(state.activeRowId);

        if (!repo) {
            section.classList.add('hidden');
            return;
        }

        section.classList.remove('hidden');

        const tagsHtml = (repo.tags || []).map(t => `<span class="rounded bg-sky-100 dark:bg-sky-900/40 px-1.5 py-0.5 text-[10px] text-sky-700 dark:text-sky-300">${escapeHtml(t)}</span>`).join('');
        const eqpTagsHtml = (repo.dataTags || []).map(t => `<span class="rounded bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-300">${escapeHtml(t)}</span>`).join('');
        const segments = Array.isArray(repo.segments) ? repo.segments : [];
        const eqpSegmentsHtml = segments.map(segment => `
            <span class="rounded bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-300">
                ${escapeHtml(segment)}
            </span>
        `).join('');

        contentRepo.innerHTML = `
            <div>
                <p class="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Tags</p>
                <div class="flex flex-wrap gap-1">${tagsHtml || '<span class="text-xs text-slate-400 dark:text-slate-600">—</span>'}</div>
            </div>
        `;

        contentEqp.innerHTML = `
            <div>
                <p class="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Tags</p>
                <div class="flex flex-wrap gap-1">${eqpTagsHtml || '<span class="text-xs text-slate-400 dark:text-slate-600">—</span>'}</div>
            </div>
            <div>
                <p class="text-[10px] text-slate-500 uppercase tracking-wider mb-1">Segments</p>
                <div class="flex flex-wrap gap-1">${eqpSegmentsHtml || '<span class="text-xs text-slate-400 dark:text-slate-600">—</span>'}</div>
            </div>
        `;

    }


    // ============================================================
    // STATS
    // ============================================================

    function updateStats() {

        const repoStat = document.getElementById('repoStat');
        const tagStat = document.getElementById('tagStat');
        const segmentStat = document.getElementById('segmentStat');

        const tags = new Set();
        const segments = new Set();

        state.repos.forEach(repo => {

            (repo.tags || []).forEach(tag => tags.add(tag));

            getRepoSegments(repo).forEach(segment => segments.add(segment));

        });

        if (repoStat) repoStat.textContent = state.repos.length;
        if (tagStat) tagStat.textContent = tags.size;
        if (segmentStat) segmentStat.textContent = segments.size;

    }


    // ============================================================
    // SELECTION
    // ============================================================

    function wireSelection() {

        document.getElementById('clearRepoSelection')?.addEventListener('click', clearSelection);

        document.getElementById('selectAllRepos')?.addEventListener('change', event => {

            currentPageItems().forEach(repo => {

                if (event.target.checked) state.selected.add(Number(repo.id));
                else state.selected.delete(Number(repo.id));

            });

            render();

        });

    }


    function clearSelection() {

        state.selected.clear();

        render();

    }


    function renderSelectionUI() {

        const bar = document.getElementById('selectionBar');
        const count = document.getElementById('selectedRepoCount');
        const list = document.getElementById('selectedRepoList');

        const selected = getSelectedRepos();

        if (selected.length) {
            bar?.classList.remove('hidden');
            bar?.classList.add('flex');
        } else {
            bar?.classList.add('hidden');
            bar?.classList.remove('flex');
        }

        if (count) count.textContent = `${selected.length} selected`;

        if (list) {

            list.innerHTML = selected.map(repo => `
                <span class="shrink-0 rounded-md border border-cyan-500/20 bg-cyan-500/10 px-2 py-1 font-mono text-[10px] text-cyan-700 dark:text-cyan-300">
                    ${escapeHtml(repo.name)}
                </span>
            `).join('');

        }

    }


    function updateSelectAllState() {

        const selectAll = document.getElementById('selectAllRepos');

        if (!selectAll) return;

        const items = currentPageItems();

        const selectedCount = items.filter(repo => state.selected.has(Number(repo.id))).length;

        selectAll.checked = items.length > 0 && selectedCount === items.length;

        selectAll.indeterminate = selectedCount > 0 && selectedCount < items.length;

    }


    function getSelectedRepos() {

        return state.repos.filter(repo => state.selected.has(Number(repo.id)));

    }


    // ============================================================
    // TOOLBAR
    // ============================================================

    function wireToolbar() {

        document.getElementById('repoSearch')?.addEventListener('input', event => {
            state.search = event.target.value;
            state.page = 1;
            applyFilters();
        });

        document.getElementById('crudFilter')?.addEventListener('change', event => {
            state.crud = event.target.value;
            state.page = 1;
            applyFilters();
        });

        document.getElementById('resetFilters')?.addEventListener('click', resetFilters);

        document.getElementById('createRepoBtn')?.addEventListener('click', openCreateModal);

        document.getElementById('repoPrev')?.addEventListener('click', () => {
            if (state.page > 1) {
                state.page--;
                render();
            }
        });

        document.getElementById('repoNext')?.addEventListener('click', () => {
            const totalPages = getTotalPages();
            if (state.page < totalPages) {
                state.page++;
                render();
            }
        });

    }


    function resetFilters() {

        state.search = '';
        state.crud = 'all';
        state.tags.clear();
        state.segments.clear();
        state.page = 1;

        const search = document.getElementById('repoSearch');
        if (search) search.value = '';

        const crud = document.getElementById('crudFilter');
        if (crud) crud.value = 'all';

        applyFilters();

    }


    // ============================================================
    // TAG INPUT HELPER
    // ============================================================

    function setupTagInput({ listElId, inputElId, addBtnElId, initialTags = [] }) {
        const listEl = document.getElementById(listElId);
        const inputEl = document.getElementById(inputElId);
        const addBtnEl = document.getElementById(addBtnElId);
        const tags = [...new Set((initialTags || []).map(t => String(t).trim()).filter(Boolean))];

        function render() {
            if (!listEl) return;
            listEl.innerHTML = tags.map((tag, index) => `
<span class="inline-flex items-center gap-1.5 rounded-md
             border border-cyan-700/40 dark:border-cyan-500/30
             bg-cyan-200 dark:bg-cyan-500/10
             px-2 py-0.5 text-xs
             text-cyan-900 dark:text-cyan-300">
                                 <span>${escapeHtml(tag)}</span>
                    <button type="button" data-remove-tag-index="${index}" class="ml-0.5 rounded px-1 leading-none
       text-cyan-800 dark:text-cyan-400
       hover:bg-cyan-300 dark:hover:bg-cyan-500/30
       hover:text-cyan-950 dark:hover:text-white
       transition cursor-pointer""Remove ${escapeAttr(tag)}">&times;</button>
                </span>
            `).join('');
        }

        function addTagFromInput() {
            if (!inputEl) return;
            const raw = inputEl.value;
            if (!raw || !raw.trim()) return;
            raw.split(',').map(t => t.trim()).filter(Boolean).forEach(item => {
                if (!tags.includes(item)) tags.push(item);
            });
            inputEl.value = '';
            render();
        }

        inputEl?.addEventListener('keydown', event => {
            if (event.key === 'Enter') {
                event.preventDefault();
                event.stopPropagation();
                addTagFromInput();
            }
        });

        addBtnEl?.addEventListener('click', event => {
            event.preventDefault();
            addTagFromInput();
        });

        listEl?.addEventListener('click', event => {
            const btn = event.target.closest('[data-remove-tag-index]');
            if (!btn) return;
            event.preventDefault();
            const idx = parseInt(btn.dataset.removeTagIndex, 10);
            if (!isNaN(idx) && idx >= 0 && idx < tags.length) {
                tags.splice(idx, 1);
                render();
            }
        });

        render();
        return { getTags: () => { addTagFromInput(); return [...tags]; } };
    }


    // ============================================================
    // CREATE
    // ============================================================

    let activeCreateTagHelper = null;

    async function openCreateModal() {

        openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Create Repo View</h2>

            <label class="mb-1 block text-[11px] text-slate-500">Folder Name <span class="text-rose-500 dark:text-rose-400">*</span></label>
            <input id="createName" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200 placeholder:text-slate-400 dark:placeholder:text-slate-600" placeholder="e.g. Production APIs">

            <label class="mb-1 block text-[11px] text-slate-500">Source Collection <span class="text-rose-500 dark:text-rose-400">*</span></label>
            <select id="createSourceCollection" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">
                <option value="" disabled selected>Loading collections...</option>
            </select>

           <label class="mb-1 block text-[11px] text-slate-500">Tags</label>

<input
    id="createTags"
    class="mb-3 h-9 w-full rounded-md
           border border-slate-300 dark:border-slate-700
           bg-white dark:bg-slate-950
           px-3 text-xs
           text-slate-800 dark:text-slate-200
           placeholder:text-slate-400 dark:placeholder:text-slate-600
           outline-none focus:border-cyan-500"
    placeholder="api, production, users"
>

<div id="createTagsList"
     class="mb-2 flex flex-wrap gap-1.5 empty:hidden">
</div>

<div class="mb-3 flex items-center gap-2">
    <input
        id="createTagInput"
        class="h-9 flex-1 rounded-md
               border border-slate-300 dark:border-slate-700
               bg-white dark:bg-slate-950
               px-3 text-xs
               text-slate-800 dark:text-slate-200
               placeholder:text-slate-400 dark:placeholder:text-slate-600
               outline-none focus:border-cyan-500"
        placeholder="Type a tag and press Enter"
    >

    <button
        id="createAddTagBtn"
        type="button"
        class="h-9 rounded-md border
               border-slate-300 dark:border-slate-700
               bg-slate-100 dark:bg-slate-800
               px-3 text-xs
               text-slate-700 dark:text-slate-300
               hover:bg-slate-200 dark:hover:bg-slate-700
               hover:text-slate-900 dark:hover:text-white
               transition"
    >
        Add
    </button>
</div>

            <label class="mb-1 block text-[11px] text-slate-500">Annotation</label>
            <textarea id="createAnnotation" class="mb-4 h-20 w-full resize-none rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 py-2 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200 placeholder:text-slate-400 dark:placeholder:text-slate-600" placeholder="Repository description..."></textarea>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmCreate" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Create</button>
            </div>

        `);

        activeCreateTagHelper = setupTagInput({
            listElId: 'createTagsList',
            inputElId: 'createTagInput',
            addBtnElId: 'createAddTagBtn',
            initialTags: []
        });

        document.getElementById('confirmCreate')?.addEventListener('click', createRepo);

        const select = document.getElementById('createSourceCollection');
        try {
            let collections = [];
            if (window.EdmsAPI && typeof window.EdmsAPI.listCollections === 'function') {
                const res = await window.EdmsAPI.listCollections();
                if (res && res.ok && res.data) {
                    if (Array.isArray(res.data.items)) {
                        collections = res.data.items.map(c => typeof c === 'string' ? c : c.name);
                    } else if (Array.isArray(res.data)) {
                        collections = res.data.map(c => typeof c === 'string' ? c : c.name);
                    }
                }
            }
            if (select) {
                if (collections.length === 0) {
                    select.innerHTML = '<option value="" disabled selected>No collections available</option>';
                } else {
                    select.innerHTML = '<option value="" disabled selected>Select a collection...</option>' +
                        collections.map(c => `<option value="${escapeAttr(c)}">${escapeHtml(c)}</option>`).join('');
                }
            }
        } catch (err) {
            console.warn('Failed to load collections for create repo modal:', err);
            if (select) {
                select.innerHTML = '<option value="" disabled selected>Failed to load collections</option>';
            }
        }

    }


    async function createRepo() {

        const name = document.getElementById('createName')?.value.trim();
        const sourceCollection = document.getElementById('createSourceCollection')?.value.trim();

        if (!name) {
            showAlert('Folder name is required.');
            return;
        }

        if (!sourceCollection) {
            showAlert('Source collection is required.');
            return;
        }

        if (state.repos.some(repo => repo.name.toLowerCase() === name.toLowerCase())) {
            showAlert('A Repo View with this name already exists.');
            return;
        }

        try {
            const annotation = document.getElementById('createAnnotation')?.value.trim() || undefined;
            const res = await window.EdmsAPI.createRepoview({
                name,
                source_collection: sourceCollection,
                annotation
            });

            if (!res.ok) {
                const errMsg = res.data?.error || res.data?.message || `Failed to create Repo View via API (HTTP ${res.status})`;
                showAlert(errMsg);
                return;
            }

            const tags = activeCreateTagHelper ? activeCreateTagHelper.getTags() : [];

            for (const t of tags) {
                if (window.EdmsAPI && typeof window.EdmsAPI.addRepoviewMembershipTag === 'function') {
                    try {
                        await window.EdmsAPI.addRepoviewMembershipTag(name, t);
                    } catch (e) {
                        console.warn('Failed to add membership tag on create:', e);
                    }
                } else if (window.EdmsAPI && typeof window.EdmsAPI.createRepoviewTag === 'function') {
                    try {
                        await window.EdmsAPI.createRepoviewTag(t);
                    } catch (e) {
                        console.warn('Failed to create repoview tag:', e);
                    }
                }
            }

            showAlert(`Repo View "${name}" created successfully.`);
            closeModal();
            await loadData();

            state.selected.clear();
            state.page = 1;

            renderSidebar();
            applyFilters();
        } catch (error) {
            console.error(error);
            showAlert(`Error creating repo view: ${error.message}`);
        }

    }


    // ============================================================
    // DUPLICATE
    // ============================================================

    async function duplicateRepo(id) {

        const source = getRepo(id);

        if (!source) return;

        let copyName = `${source.name} (copy)`;
        let counter = 2;

        while (state.repos.some(repo => repo.name.toLowerCase() === copyName.toLowerCase())) {
            copyName = `${source.name} (copy ${counter})`;
            counter++;
        }

        try {
            if (window.EdmsAPI && typeof window.EdmsAPI.duplicateRepoview === 'function') {
                const res = await window.EdmsAPI.duplicateRepoview(source.name, copyName);
                if (res && !res.ok) {
                    const errMsg = res.data?.error || res.data?.message || `Failed to duplicate repo view (HTTP ${res.status})`;
                    showAlert(errMsg);
                    return;
                }
            }
        } catch (err) {
            showAlert(`Error duplicating repo view: ${err.message}`);
            return;
        }

        showAlert(`Repo View "${source.name}" duplicated as "${copyName}".`);
        await loadData();
        state.selected.clear();
        state.page = 1;

        renderSidebar();
        applyFilters();

    }


    // ============================================================
    // DELETE
    // ============================================================

    function openDeleteModal(ids) {

        const repos = ids.map(getRepo).filter(Boolean);

        if (!repos.length) return;

        openModal(`

            <div class="flex items-start gap-3 rounded-lg border border-rose-500/20 bg-rose-500/5 p-3">
                <div class="mt-0.5 text-rose-600 dark:text-rose-400">${iconTrash()}</div>
                <div>
                    <h2 class="text-sm font-semibold text-rose-700 dark:text-rose-300">Delete ${repos.length > 1 ? 'Repo Views' : 'Repo View'}</h2>
                    <p class="mt-1 text-[11px] text-slate-500">This removes the selected repo view(s) from the system.</p>
                </div>
            </div>

            <div class="my-4 max-h-48 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800">
                ${repos.map(repo => `
                    <div class="border-b border-slate-200 dark:border-slate-800 px-3 py-2 last:border-0">
                        <p class="font-mono text-xs text-slate-800 dark:text-slate-300">${escapeHtml(repo.name)}</p>
                    </div>
                `).join('')}
            </div>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmDelete" type="button" class="rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-rose-500">Delete</button>
            </div>

        `);

        document.getElementById('confirmDelete')?.addEventListener('click', async () => {
            await deleteRepos(ids);
            closeModal();
        });

    }


    async function deleteRepos(ids) {

        const normalizedIds = ids.map(String);
        const reposToDelete = state.repos.filter(repo => normalizedIds.includes(String(repo.id || repo.name)));
        const names = reposToDelete.map(r => r.name || r.id).filter(Boolean);

        if (names.length === 0) return;

        try {
            if (window.EdmsAPI) {
                let res;
                if (names.length === 1 && typeof window.EdmsAPI.deleteRepoview === 'function') {
                    res = await window.EdmsAPI.deleteRepoview(names[0]);
                } else if (typeof window.EdmsAPI.deleteRepoviews === 'function') {
                    res = await window.EdmsAPI.deleteRepoviews(names);
                }
                if (res && !res.ok) {
                    const errMsg = res.data?.error || res.data?.message || `Failed to delete repo view (HTTP ${res.status})`;
                    showAlert(errMsg);
                    return;
                }
            }
        } catch (err) {
            showAlert(`Error deleting repo view: ${err.message}`);
            return;
        }

        showAlert(`Deleted ${names.length > 1 ? `${names.length} repo views` : `"${names[0]}"`}.`);
        await loadData();
        state.repos = state.repos.filter(repo => !normalizedIds.includes(String(repo.id || repo.name)) && !names.includes(repo.name));
        normalizedIds.forEach(id => state.selected.delete(Number(id)));

        renderSidebar();
        applyFilters();

    }


    // ============================================================
    // EDIT NAME / TAGS
    // ============================================================

    function openEditRepoModal(id) {

        const repo = getRepo(id);

        if (!repo) return;

        openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Edit Repo View</h2>

            <label class="mb-1 block text-[11px] text-slate-500">Folder Name</label>
            <input id="editRepoName" value="${escapeAttr(repo.name)}" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

            <label class="mb-1 block text-[11px] text-slate-500">Tags</label>
            <input id="editRepoTags" value="${escapeAttr((repo.tags || []).join(', '))}" class="mb-4 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="saveRepoEdit" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save</button>
            </div>

        `);

        document.getElementById('saveRepoEdit')?.addEventListener('click', async () => {

            const newName = document.getElementById('editRepoName')?.value.trim();

            if (!newName) {
                showAlert('Folder name is required.');
                return;
            }

            const duplicate = state.repos.some(item =>
                String(item.id || item.name) !== String(repo.id || repo.name) &&
                item.name.toLowerCase() === newName.toLowerCase()
            );

            if (duplicate) {
                showAlert('Folder name must be unique.');
                return;
            }

            const oldName = repo.name;
            if (oldName !== newName) {
                try {
                    if (window.EdmsAPI && typeof window.EdmsAPI.renameRepoview === 'function') {
                        const res = await window.EdmsAPI.renameRepoview(oldName, newName);
                        if (res && !res.ok) {
                            const errMsg = res.data?.error || res.data?.message || `Failed to rename repo view (HTTP ${res.status})`;
                            showAlert(errMsg);
                            return;
                        }
                    }
                } catch (err) {
                    showAlert(`Error renaming repo view: ${err.message}`);
                    return;
                }
            }

            const currentTags = Array.isArray(repo.tags) ? repo.tags : [];
            const newTags = document.getElementById('editRepoTags')?.value
                .split(',').map(tag => tag.trim()).filter(Boolean) || [];

            const targetRepoName = newName || repo.name;

            if (window.EdmsAPI && typeof window.EdmsAPI.addRepoviewMembershipTag === 'function') {
                const addedTags = newTags.filter(t => !currentTags.includes(t));
                const removedTags = currentTags.filter(t => !newTags.includes(t));

                for (const t of addedTags) {
                    try {
                        await window.EdmsAPI.addRepoviewMembershipTag(targetRepoName, t);
                    } catch (e) {
                        console.warn('Failed to add membership tag:', e);
                    }
                }
                for (const t of removedTags) {
                    try {
                        if (typeof window.EdmsAPI.removeRepoviewMembershipTag === 'function') {
                            await window.EdmsAPI.removeRepoviewMembershipTag(targetRepoName, t);
                        }
                    } catch (e) {
                        console.warn('Failed to remove membership tag:', e);
                    }
                }
            }

            repo.name = newName;
            repo.tags = newTags;

            showAlert(`Repo View updated successfully.`);
            closeModal();
            await loadData();
            renderSidebar();
            applyFilters();

        });

    }


    // ============================================================
    // ANNOTATION
    // ============================================================

    function openAnnotationModal(id) {

        const repo = getRepo(id);

        if (!repo) return;

        openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Annotation</h2>

            <p class="mb-2 text-[11px] text-slate-500">Markdown annotation</p>

            <textarea id="annotationEditor" class="h-64 w-full resize-none rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 py-2 font-mono text-sm text-slate-800 dark:text-slate-300 outline-none focus:border-cyan-500">${escapeHtml(repo.annotation || '')}</textarea>

            <div class="mt-4 flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="saveAnnotation" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save</button>
            </div>

        `);

        document.getElementById('saveAnnotation')?.addEventListener('click', async () => {

            const annotationVal = document.getElementById('annotationEditor')?.value;

            try {
                if (window.EdmsAPI && typeof window.EdmsAPI.setRepoviewAnnotation === 'function') {
                    const res = await window.EdmsAPI.setRepoviewAnnotation(repo.name, annotationVal);
                    if (res && !res.ok) {
                        const errMsg = res.data?.error || res.data?.message || `Failed to set annotation (HTTP ${res.status})`;
                        showAlert(errMsg);
                        return;
                    }
                }
            } catch (err) {
                showAlert(`Error saving annotation: ${err.message}`);
                return;
            }

            repo.annotation = annotationVal;

            showAlert(`Annotation updated for "${repo.name}".`);
            closeModal();
            render();

        });

    }


    // ============================================================
    // COMBINE
    // ============================================================

    function combineRepos(ids) {

        const targetIds = (ids && ids.length) ? ids : [...state.selected];
        const repos = targetIds.map(getRepo).filter(Boolean);

        if (repos.length < 2) {
            showAlert('Please select at least 2 Repo Views to combine.');
            return;
        }

        const defaultName = `${repos[0].name}_combined`;

        openModal(`
            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Combine Repo Views</h2>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Combine ${repos.length} repo views into a unified repo view:</p>
            <div class="mb-4 max-h-36 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950/60 p-2 text-xs font-mono text-slate-800 dark:text-slate-300">
                ${repos.map(r => `<div>• ${escapeHtml(r.name)}</div>`).join('')}
            </div>
            <label class="mb-1 block text-[11px] text-slate-500">Combined Repo View Name <span class="text-rose-500 dark:text-rose-400">*</span></label>
            <input id="combinedRepoName" value="${escapeAttr(defaultName)}" class="mb-4 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">
            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmCombineRepo" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Combine</button>
            </div>
        `);

        document.getElementById('confirmCombineRepo')?.addEventListener('click', () => {
            const newName = document.getElementById('combinedRepoName')?.value.trim();
            if (!newName) {
                showAlert('Combined name is required.');
                return;
            }
            executeCombineRepo(repos, newName);
        });

    }

    async function executeCombineRepo(repos, targetName, onExists, renameVal) {

        const sources = repos.map(r => r.name);
        if (sources.length < 2) {
            showAlert('At least two sources are required to combine.');
            return;
        }

        const payload = {
            name: targetName,
            sources
        };

        if (onExists) {
            payload.on_exists = onExists;
            if (onExists === 'rename' && renameVal) {
                payload.new_name = renameVal;
            }
        }

        try {
            const res = await window.EdmsAPI.combineRepoviews(payload);
            if (!res.ok) {
                if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
                    showCombineConflictModal(repos, targetName, res.data.options, res.data.error || `A Repo View named "${targetName}" already exists.`);
                    return;
                }
                const errMsg = res.data?.error || res.data?.message || `Failed to combine repo views (HTTP ${res.status})`;
                showAlert(errMsg);
                return;
            }

            const finalName = (onExists === 'rename' && renameVal) ? renameVal : targetName;
            showAlert(`Repo views combined into "${finalName}".`);
            closeModal();
            await loadData();
            state.selected.clear();
            renderSidebar();
            applyFilters();
        } catch (err) {
            showAlert(`Error combining repo views: ${err.message}`);
        }

    }

    function showCombineConflictModal(repos, targetName, options, errorMsg) {

        openModal(`
            <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
                <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: View Already Exists (409)</div>
                <div class="text-[11px] text-amber-800 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
            </div>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this conflict:</p>
            <div class="mb-4 space-y-3">
                ${options.includes('merge') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Merge</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Merge sources into the existing "${escapeHtml(targetName)}" Repo View.</p>
<button
    id="conflictMergeBtn"
    type="button"
    class="rounded bg-orange-600 px-2.5 py-1 text-xs text-white
           hover:bg-orange-700 active:bg-orange-800
           dark:bg-amber-700 dark:hover:bg-amber-600 dark:active:bg-amber-800
           transition">
    Merge into "${escapeHtml(targetName)}"
</button>                    </div>
                ` : ''}
                ${options.includes('rename') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Rename</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Save with a new, non-conflicting name.</p>
                        <div class="flex gap-2">
                            <input id="conflictRenameInput" value="${escapeAttr(targetName + '_1')}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
                            <button id="conflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Combine</button>
                        </div>
                    </div>
                ` : ''}
            </div>
            <div class="flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
            </div>
        `);

        document.getElementById('conflictMergeBtn')?.addEventListener('click', () => {
            executeCombineRepo(repos, targetName, 'merge');
        });

        document.getElementById('conflictRenameBtn')?.addEventListener('click', () => {
            const renVal = document.getElementById('conflictRenameInput')?.value.trim();
            if (!renVal) {
                showAlert('New name is required.');
                return;
            }
            executeCombineRepo(repos, targetName, 'rename', renVal);
        });

    }


    // ============================================================
    // CONVERT TO COLLECTION
    // ============================================================

    function convertToCollection(id) {

        const repo = getRepo(id);
        if (!repo) return;

        openModal(`
            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Convert to Collection</h2>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Convert Repo View <strong class="text-slate-900 dark:text-white">${escapeHtml(repo.name)}</strong> into a Collection.</p>
            
            <label class="mb-1 block text-[11px] text-slate-500">Collection Name <span class="text-rose-500 dark:text-rose-400">*</span></label>
            <input id="convertCollectionName" value="${escapeAttr(repo.name)}" class="mb-4 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmConvertToCollection" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Convert</button>
            </div>
        `);

        document.getElementById('confirmConvertToCollection')?.addEventListener('click', () => {
            const colName = document.getElementById('convertCollectionName')?.value.trim();
            if (!colName) {
                showAlert('Collection name is required.');
                return;
            }
            executeConvertRepo(repo, colName);
        });

    }

    async function executeConvertRepo(repo, colName, onExists, renameVal) {

        const payload = { collection: colName };
        if (onExists) {
            payload.on_exists = onExists;
            if (onExists === 'rename' && renameVal) {
                payload.new_name = renameVal;
            }
        }

        try {
            const res = await window.EdmsAPI.convertRepoviewToCollection(repo.name, payload);
            if (!res.ok) {
                if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
                    showConvertConflictModal(repo, colName, res.data.options, res.data.error || `Collection "${colName}" already exists.`);
                    return;
                }
                const errMsg = res.data?.error || res.data?.message || `Failed to convert to collection (HTTP ${res.status})`;
                showAlert(errMsg);
                return;
            }

            const finalName = (onExists === 'rename' && renameVal) ? renameVal : colName;
            showAlert(`Repo View "${repo.name}" successfully converted to Collection "${finalName}".`);
            closeModal();
            await loadData();
            state.selected.clear();
            renderSidebar();
            applyFilters();
        } catch (err) {
            showAlert(`Error converting to collection: ${err.message}`);
        }

    }

    function showConvertConflictModal(repo, colName, options, errorMsg) {

        openModal(`
            <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
                <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: Collection Already Exists (409)</div>
                <div class="text-[11px] text-amber-800 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
            </div>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this conflict:</p>
            <div class="mb-4 space-y-3">
                ${options.includes('merge') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Merge</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Merge into the existing collection "${escapeHtml(colName)}".</p>
                        <button id="convertConflictMergeBtn" type="button" class="rounded bg-amber-600 px-2.5 py-1 text-xs text-white hover:bg-amber-500">Merge into "${escapeHtml(colName)}"</button>
                    </div>
                ` : ''}
                ${options.includes('rename') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Rename</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Convert to a collection with a new name.</p>
                        <div class="flex gap-2">
                            <input id="convertConflictRenameInput" value="${escapeAttr(colName + '_1')}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
                            <button id="convertConflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Convert</button>
                        </div>
                    </div>
                ` : ''}
            </div>
            <div class="flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
            </div>
        `);

        document.getElementById('convertConflictMergeBtn')?.addEventListener('click', () => {
            executeConvertRepo(repo, colName, 'merge');
        });

        document.getElementById('convertConflictRenameBtn')?.addEventListener('click', () => {
            const renVal = document.getElementById('convertConflictRenameInput')?.value.trim();
            if (!renVal) {
                showAlert('New name is required.');
                return;
            }
            executeConvertRepo(repo, colName, 'rename', renVal);
        });

    }


    // ============================================================
    // TAKEOUT
    // ============================================================

    function takeoutRepo(id) {

        const repo = getRepo(id);
        if (!repo) return;

        openModal(`
            <h2 class="mb-4 text-sm font-semibold text-slate-900 dark:text-white">Takeout Repo View</h2>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Export and takeout Repo View <strong class="text-slate-900 dark:text-white">${escapeHtml(repo.name)}</strong>.</p>
            
            <label class="mb-1 block text-[11px] text-slate-500">Destination Folder Name (Optional)</label>
            <input id="takeoutDestName" value="${escapeAttr(repo.name)}" placeholder="${escapeAttr(repo.name)}" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

            <label class="mb-4 flex items-center gap-2 cursor-pointer">
                <input id="takeoutOverwrite" type="checkbox" class="h-4 w-4 rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-cyan-600 focus:ring-0 focus:ring-offset-0">
                <span class="text-xs text-slate-800 dark:text-slate-300">Overwrite if destination already exists</span>
            </label>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmRepoTakeout" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Takeout</button>
            </div>
        `);

        document.getElementById('confirmRepoTakeout')?.addEventListener('click', () => {
            const destName = document.getElementById('takeoutDestName')?.value.trim() || undefined;
            const overwrite = Boolean(document.getElementById('takeoutOverwrite')?.checked);
            executeTakeoutRepo(repo, destName, overwrite);
        });

    }

    async function executeTakeoutRepo(repo, destName, overwrite) {

        const payload = {
            dest_name: destName || repo.name,
            overwrite: Boolean(overwrite)
        };

        try {
            const res = await window.EdmsAPI.takeoutRepoview(repo.name, payload);
            if (!res.ok) {
                if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
                    showTakeoutConflictModal(repo, payload.dest_name, res.data.options, res.data.error || `Destination "${payload.dest_name}" already exists.`);
                    return;
                }
                const errMsg = res.data?.error || res.data?.message || `Takeout request failed (HTTP ${res.status})`;
                showAlert(errMsg);
                return;
            }

            const jobId = res.data?.job_id || res.data?.id;
            if (jobId) {
                showAlert(`Takeout accepted (Job ID: ${jobId}). Processing in background...`);
                if (window.EdmsAPI && typeof window.EdmsAPI.listenForJob === 'function') {
                    window.EdmsAPI.listenForJob(jobId, () => {
                        showAlert(`Takeout complete for Repo View "${repo.name}"!`);
                    }, (err) => {
                        showAlert(`Takeout job ${jobId} failed: ${err.message || 'Error'}`);
                    });
                }
            } else {
                showAlert(`Takeout initiated successfully for Repo View "${repo.name}".`);
            }
            closeModal();
        } catch (err) {
            showAlert(`Error requesting takeout: ${err.message}`);
        }

    }

    function showTakeoutConflictModal(repo, destName, options, errorMsg) {

        openModal(`
            <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
                <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: Destination Already Exists (409)</div>
                <div class="text-[11px] text-amber-800 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
            </div>
            <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this collision:</p>
            <div class="mb-4 space-y-3">
                ${options.includes('overwrite') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Overwrite</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Overwrite existing files in "${escapeHtml(destName)}".</p>
                        <button id="takeoutConflictOverwriteBtn" type="button" class="rounded bg-rose-600 px-2.5 py-1 text-xs text-white hover:bg-rose-500">Overwrite Existing</button>
                    </div>
                ` : ''}
                ${options.includes('rename') ? `
                    <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900/60 p-3">
                        <div class="text-xs font-medium text-slate-900 dark:text-white mb-1">Rename</div>
                        <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Save takeout to a different folder name.</p>
                        <div class="flex gap-2">
                            <input id="takeoutConflictRenameInput" value="${escapeAttr(destName + '_1')}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
                            <button id="takeoutConflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Takeout</button>
                        </div>
                    </div>
                ` : ''}
            </div>
            <div class="flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
            </div>
        `);

        document.getElementById('takeoutConflictOverwriteBtn')?.addEventListener('click', () => {
            executeTakeoutRepo(repo, destName, true);
        });

        document.getElementById('takeoutConflictRenameBtn')?.addEventListener('click', () => {
            const renVal = document.getElementById('takeoutConflictRenameInput')?.value.trim();
            if (!renVal) {
                showAlert('Destination folder name is required.');
                return;
            }
            executeTakeoutRepo(repo, renVal, false);
        });

    }


    // ============================================================
    // MODAL
    // ============================================================

    function wireModal() {

        const backdrop = document.getElementById('modalBackdrop');

        if (!backdrop) return;

        backdrop.addEventListener('click', event => {

            if (event.target === backdrop) closeModal();

            if (event.target.closest('[data-modal-close]')) closeModal();

        });

        document.addEventListener('keydown', event => {

            if (event.key === 'Escape' && !backdrop.classList.contains('hidden')) {
                closeModal();
            }

        });

    }


    function openModal(html) {

        const backdrop = document.getElementById('modalBackdrop');
        const content = document.getElementById('modalContent');

        if (!backdrop || !content) return;

        content.innerHTML = html;

        backdrop.classList.remove('hidden');
        backdrop.classList.add('flex');

    }


    function closeModal() {

        const backdrop = document.getElementById('modalBackdrop');
        const content = document.getElementById('modalContent');

        backdrop?.classList.add('hidden');
        backdrop?.classList.remove('flex');

        if (content) content.innerHTML = '';

    }


    // ============================================================
    // SIZE
    // ============================================================

    function cycleSizeUnit(id) {

        const units = ['B', 'KB', 'MB', 'GB'];

        const current = sizeUnits[id] || 'MB';

        const index = units.indexOf(current);

        sizeUnits[id] = units[(index + 1) % units.length];

        renderTable();

    }


    function formatSize(bytes, unit) {

        const value = Number(bytes) || 0;

        switch (unit) {

            case 'B': return `${value.toFixed(0)} B`;
            case 'KB': return `${(value / 1024).toFixed(1)} KB`;
            case 'GB': return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
            case 'MB':
            default: return `${(value / 1024 / 1024).toFixed(1)} MB`;

        }

    }


    // ============================================================
    // HELPERS
    // ============================================================

    function getRepo(id) {

        return state.repos.find(repo => Number(repo.id) === Number(id));

    }


    function escapeHtml(value) {

        const div = document.createElement('div');
        div.textContent = String(value ?? '');
        return div.innerHTML;

    }


    function escapeAttr(value) {

        return escapeHtml(value).replace(/"/g, '&quot;');

    }


    function showAlert(message) {

        window.alert(message);

    }


    function iconTrash() {

        return `
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <path d="M4 7h16"/>
                <path d="M10 11v6M14 11v6"/>
                <path d="M6 7l1 13h10l1-13"/>
                <path d="M9 7V4h6v3"/>
            </svg>
        `;

    }


    // ============================================================
    // PUBLIC API (consumed by rmbrv.js)
    // ============================================================

    window.RepoView = {

        getState: () => state,

        getRepo,

        duplicateRepo,

        openEditRepoModal,

        openAnnotationModal,

        openDataTagsModal,

        openDeleteModal,

        deleteRepos,

        openCreateModal,

        combineRepos,

        convertToCollection,

        takeoutRepo,

        loadData

    };

    // Global references for convenience
    window.duplicateRepo = duplicateRepo;
    window.deleteRepos = deleteRepos;
    window.openDeleteModal = openDeleteModal;
    window.combineRepos = combineRepos;
    window.convertToCollection = convertToCollection;
    window.takeoutRepo = takeoutRepo;

})();