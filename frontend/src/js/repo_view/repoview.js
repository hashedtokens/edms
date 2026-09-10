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

        sidebarCollapsed: false

    };

    let sizeUnits = {};

    const API_BASE = 'http://localhost:3000';


    // ============================================================
    // INIT
    // ============================================================

    document.addEventListener('DOMContentLoaded', init);


    async function init() {

        await loadData();

        wireToolbar();

        wireSidebar();

        wireSelection();

        wireModal();

        renderSidebar();

        applyFilters();

    }


    // ============================================================
    // DATA
    // ============================================================

    async function loadData() {

        try {

            const response = await fetch(DATA_URL);

            if (!response.ok) throw new Error(`HTTP ${response.status}`);

            const json = await response.json();

            state.repos = Array.isArray(json.repoViews) ? json.repoViews : [];

            nextId = Math.max(10000, ...state.repos.map(item => Number(item.id) || 0)) + 1;

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
                        <p class="text-sm font-medium text-slate-400">No repo views found</p>
                        <p class="mt-1 text-xs text-slate-600">Try changing the current search or filters.</p>
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

        row.dataset.id = String(id);

        row.className = [
            'repo-row',
            'border-b',
            'border-slate-800',
            'transition-colors',
            'duration-100',
            'hover:bg-slate-800/60',
            isSelected ? 'bg-cyan-500/[0.045]' : ''
        ].filter(Boolean).join(' ');

        const dataTagsCount = (repo.dataTags || []).length;
        const segmentCount = getRepoSegmentCount(repo);

        row.innerHTML = `

            <!-- Select -->
            <td class="px-2 py-2 align-middle">
                <input type="checkbox" class="repo-select h-3.5 w-3.5 accent-cyan-400" ${isSelected ? 'checked' : ''}>
            </td>

            <!-- Folder Name -->
            <td class="px-2 py-2 font-medium text-slate-200">
                <button
                    type="button"
                    class="repo-name-btn truncate text-left hover:text-cyan-400"
                    title="${escapeAttr(repo.name)}">
                    ${escapeHtml(repo.name)}
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

            <!-- GET -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.GET, 'text-emerald-400')}</td>
            <!-- POST -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.POST, 'text-sky-400')}</td>
            <!-- PUT -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.PUT, 'text-amber-400')}</td>
            <!-- PATCH -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.PATCH, 'text-violet-400')}</td>
            <!-- DELETE -->
            <td class="px-1 py-2 text-center">${crudCountBadge(repo.crud?.DELETE, 'text-rose-400')}</td>

            <!-- Size -->
            <td class="px-2 py-2">
                <button type="button" class="repo-size-btn text-slate-300 hover:text-cyan-400">
                    ${formatSize(repo.dataSizeBytes, sizeUnits[repo.id])}
                </button>
            </td>

            <!-- EID Count -->
            <td class="px-2 py-2 text-slate-300">${repo.eidCount ?? 0}</td>

            <!-- Segments -->
            <td class="px-2 py-2 text-slate-300">${segmentCount}</td>

            <!-- Data Tags -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="repo-datatags-btn rounded-md border border-slate-700 bg-slate-800 px-2 py-0.5 text-[11px] text-slate-300 transition hover:border-cyan-500/60 hover:text-cyan-400">
                    ${dataTagsCount} tag${dataTagsCount === 1 ? '' : 's'}
                </button>
            </td>

            <!-- QP Count -->
            <td class="px-2 py-2 text-slate-300">${repo.qpCount ?? 0}</td>

            <!-- Index Lists -->
            <td class="px-2 py-2 text-slate-300">${repo.indexLists ?? 0}</td>

            <!-- Annotation -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="repo-annotation-btn block max-w-xs truncate text-left text-[11px] text-slate-500 hover:text-cyan-400"
                    title="${escapeAttr(repo.annotation || '')}">
                    ${escapeHtml(repo.annotation || '—')}
                </button>
            </td>

        `;


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
            return `<span class="text-[10px] text-slate-600">—</span>`;
        }

        return tags.map(tag => `
            <button
                type="button"
                class="repo-tag inline-flex rounded bg-sky-900/40 px-1.5 py-0.5 text-[10px] text-sky-300 transition hover:bg-sky-500/20 hover:text-sky-200"
                data-tag="${escapeAttr(tag)}">
                ${escapeHtml(tag)}
            </button>
        `).join('');

    }


    function crudCountBadge(count, color) {

        const value = Number(count || 0);

        return `
            <span class="inline-flex min-w-6 items-center justify-center rounded-md bg-slate-800 px-1.5 py-0.5 text-[10px] ${value > 0 ? color : 'text-slate-600'}">
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

            <h2 class="mb-4 text-sm font-semibold text-white">Tags in Data</h2>

            <div class="flex flex-wrap gap-2">
                ${
                    tags.length
                        ? tags.map(tag => `
                            <span class="rounded-md border border-slate-700 bg-slate-800 px-2.5 py-1 text-xs text-slate-300">
                                ${escapeHtml(tag)}
                            </span>
                        `).join('')
                        : '<span class="text-xs text-slate-600">No tags in data.</span>'
                }
            </div>

            <div class="mt-4 flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800">
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
                span.className = 'px-1 text-slate-600';
                span.textContent = '…';
                numbers.appendChild(span);
                return;

            }

            const button = document.createElement('button');
            button.type = 'button';
            button.textContent = page;

            button.className = page === state.page
                ? 'min-w-7 rounded-md bg-cyan-500 px-2 py-1 text-[11px] text-white'
                : 'min-w-7 rounded-md border border-slate-700 px-2 py-1 text-[11px] text-slate-400 transition hover:bg-slate-800 hover:text-white';

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

            sidebar.classList.remove('w-48');
            sidebar.classList.add('w-10');
            content.classList.add('hidden');
            title.classList.add('hidden');
            icon.innerHTML = `<path d="m9 18 6-6-6-6"/>`;

        } else {

            sidebar.classList.remove('w-10');
            sidebar.classList.add('w-48');
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
                    <button type="button" class="repo-tag-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-400 transition hover:bg-sky-500/10 hover:text-sky-300" data-tag="${escapeAttr(tag)}">
                        ${escapeHtml(tag)} <span class="text-slate-600">(${count})</span>
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

            container.innerHTML = `<span class="text-[11px] text-slate-600">No segment data available.</span>`;
            return;

        }

        Object.entries(counts)
            .sort((a, b) => a[0].localeCompare(b[0]))
            .forEach(([segment, count]) => {

                const wrapper = document.createElement('label');
                wrapper.className = 'group flex cursor-pointer items-center gap-2';

                wrapper.innerHTML = `
                    <input type="checkbox" class="repo-segment-checkbox h-3.5 w-3.5 accent-cyan-400" data-segment="${escapeAttr(segment)}">
                    <button type="button" class="repo-segment-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-400 transition hover:bg-cyan-500/10 hover:text-cyan-300" data-segment="${escapeAttr(segment)}">
                        ${escapeHtml(segment)} <span class="text-slate-600">(${count})</span>
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
                <span class="shrink-0 rounded-md border border-cyan-500/20 bg-cyan-500/10 px-2 py-1 font-mono text-[10px] text-cyan-300">
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
    // CREATE
    // ============================================================

    function openCreateModal() {

        openModal(`

            <h2 class="mb-4 text-sm font-semibold text-white">Create Repo View</h2>

            <label class="mb-1 block text-[11px] text-slate-500">Folder Name</label>
            <input id="createName" class="mb-3 h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500" placeholder="e.g. Production APIs">

            <label class="mb-1 block text-[11px] text-slate-500">Tags</label>
            <input id="createTags" class="mb-3 h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500" placeholder="api, production, users">

            <label class="mb-1 block text-[11px] text-slate-500">Annotation</label>
            <textarea id="createAnnotation" class="mb-4 h-20 w-full resize-none rounded-md border border-slate-700 bg-slate-950 px-3 py-2 text-xs outline-none focus:border-cyan-500" placeholder="Repository description..."></textarea>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800">Cancel</button>
                <button id="confirmCreate" type="button" class="rounded-md bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-cyan-500">Create</button>
            </div>

        `);

        document.getElementById('confirmCreate')?.addEventListener('click', createRepo);

    }


    function createRepo() {

        const name = document.getElementById('createName').value.trim();

        if (!name) {
            showAlert('Folder name is required.');
            return;
        }

        if (state.repos.some(repo => repo.name.toLowerCase() === name.toLowerCase())) {
            showAlert('A Repo View with this name already exists.');
            return;
        }

        const tags = document.getElementById('createTags').value
            .split(',').map(tag => tag.trim()).filter(Boolean);

        const repo = {
            id: nextId++,
            name,
            annotation: document.getElementById('createAnnotation').value.trim(),
            tags,
            dateCreated: new Date().toISOString().slice(0, 10),
            dataSizeBytes: 0,
            eidCount: 0,
            dataTags: [],
            qpCount: 0,
            indexLists: 0,
            crud: { GET: 0, POST: 0, PUT: 0, PATCH: 0, DELETE: 0 }
        };

        state.repos.unshift(repo);
        sizeUnits[repo.id] = 'MB';
        state.selected.clear();
        state.page = 1;

        closeModal();
        renderSidebar();
        applyFilters();

    }


    // ============================================================
    // DUPLICATE
    // ============================================================

    function duplicateRepo(id) {

        const source = getRepo(id);

        if (!source) return;

        let copyName = `${source.name} (copy)`;
        let counter = 2;

        while (state.repos.some(repo => repo.name.toLowerCase() === copyName.toLowerCase())) {
            copyName = `${source.name} (copy ${counter})`;
            counter++;
        }

        const copy = structuredClone(source);

        copy.id = nextId++;
        copy.name = copyName;
        copy.dateCreated = new Date().toISOString().slice(0, 10);

        state.repos.unshift(copy);
        sizeUnits[copy.id] = 'MB';
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
                <div class="mt-0.5 text-rose-400">${iconTrash()}</div>
                <div>
                    <h2 class="text-sm font-semibold text-rose-300">Delete ${repos.length > 1 ? 'Repo Views' : 'Repo View'}</h2>
                    <p class="mt-1 text-[11px] text-slate-500">This removes the selected repo view(s) from the current prototype session.</p>
                </div>
            </div>

            <div class="my-4 max-h-48 overflow-y-auto rounded-lg border border-slate-800">
                ${repos.map(repo => `
                    <div class="border-b border-slate-800 px-3 py-2 last:border-0">
                        <p class="font-mono text-xs text-slate-300">${escapeHtml(repo.name)}</p>
                    </div>
                `).join('')}
            </div>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800">Cancel</button>
                <button id="confirmDelete" type="button" class="rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-rose-500">Delete</button>
            </div>

        `);

        document.getElementById('confirmDelete')?.addEventListener('click', () => {
            deleteRepos(ids);
            closeModal();
        });

    }


    function deleteRepos(ids) {

        const normalizedIds = ids.map(Number);

        state.repos = state.repos.filter(repo => !normalizedIds.includes(Number(repo.id)));

        normalizedIds.forEach(id => state.selected.delete(id));

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

            <h2 class="mb-4 text-sm font-semibold text-white">Edit Repo View</h2>

            <label class="mb-1 block text-[11px] text-slate-500">Folder Name</label>
            <input id="editRepoName" value="${escapeAttr(repo.name)}" class="mb-3 h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500">

            <label class="mb-1 block text-[11px] text-slate-500">Tags</label>
            <input id="editRepoTags" value="${escapeAttr((repo.tags || []).join(', '))}" class="mb-4 h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500">

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800">Cancel</button>
                <button id="saveRepoEdit" type="button" class="rounded-md bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-cyan-500">Save</button>
            </div>

        `);

        document.getElementById('saveRepoEdit')?.addEventListener('click', () => {

            const newName = document.getElementById('editRepoName').value.trim();

            if (!newName) {
                showAlert('Folder name is required.');
                return;
            }

            const duplicate = state.repos.some(item =>
                Number(item.id) !== Number(repo.id) &&
                item.name.toLowerCase() === newName.toLowerCase()
            );

            if (duplicate) {
                showAlert('Folder name must be unique.');
                return;
            }

            repo.name = newName;

            repo.tags = document.getElementById('editRepoTags').value
                .split(',').map(tag => tag.trim()).filter(Boolean);

            closeModal();
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

            <h2 class="mb-4 text-sm font-semibold text-white">Annotation</h2>

            <p class="mb-2 text-[11px] text-slate-500">Markdown annotation</p>

            <textarea id="annotationEditor" class="h-64 w-full resize-none rounded-md border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-sm text-slate-300 outline-none focus:border-cyan-500">${escapeHtml(repo.annotation || '')}</textarea>

            <div class="mt-4 flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800">Cancel</button>
                <button id="saveAnnotation" type="button" class="rounded-md bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-cyan-500">Save</button>
            </div>

        `);

        document.getElementById('saveAnnotation')?.addEventListener('click', () => {

            repo.annotation = document.getElementById('annotationEditor').value;

            closeModal();
            render();

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

        openCreateModal

    };

})();