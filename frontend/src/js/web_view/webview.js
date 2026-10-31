(() => {
  "use strict";

  // ============================================================
  // CONFIG
  // ============================================================

  const DATA_URL = "../data/webview.json";

  const PAGE_SIZE = 10;

  const CRUD_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE"];

  // ============================================================
  // STATE
  // ============================================================

  const state = {
    webs: [],

    filtered: [],

    page: 1,

    search: "",

    crud: "all",

    tags: new Set(),

    segments: new Set(),

    selected: new Set(),

    activeRowId: null,

    sidebarCollapsed: false,
  };

  let sizeUnits = {};

  const API_BASE = "http://localhost:3000";

  // ============================================================
  // INIT
  // ============================================================

  document.addEventListener("DOMContentLoaded", init);

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
        console.warn("EdmsAPI not loaded yet");
        return;
      }

      const response = await window.EdmsAPI.listWebviews();

      if (!response.ok) throw new Error(`HTTP ${response.status}`);

      const data = response.data;
      let list = [];
      if (data && Array.isArray(data.items)) {
        list = data.items;
      } else if (Array.isArray(data)) {
        list = data;
      }

      // Map the backend items to the structure expected by the frontend table
      state.webs = list.map((item) => {
        const id = item.id || item.name;
        const name = item.name || item.id || "";
        const segments = Array.isArray(item.segments) ? item.segments : [];
        const segmentCount =
          typeof item.segmentCount === "number"
            ? item.segmentCount
            : Array.isArray(item.segments)
              ? item.segments.length
              : typeof item.segments === "number"
                ? item.segments
                : 0;

        return {
          id: id,
          name: name,
          annotation: item.annotation || "",
          source: item.source || "",
          tags: Array.isArray(item.tags) ? item.tags : (item.tags ? [item.tags] : []),
          dataTags: Array.isArray(item.dataTags) ? item.dataTags : (item.dataTags ? [item.dataTags] : []),
          crud: item.crud || { GET: 0, POST: 0, PUT: 0, PATCH: 0, DELETE: 0 },
          segments: segments,
          segmentCount: segmentCount,
          segmentFrequency: item.segmentFrequency || {},
          eidCount: item.eidCount ?? item.endpoint_count ?? 0,
          qpCount: item.qpCount ?? 0,
          indexLists: item.indexLists ?? 0,
          dataSizeBytes: item.dataSizeBytes ?? 0,
          dateCreated: item.dateCreated || (item.created_at ? item.created_at.split(" ")[0] : ""),
        };
      });

      if (state.webs.length === 0) {
        state.webs.push({
          id: "demo webview",
          name: "demo webview",
          annotation: "just for referencing (dummy entry)",
          dateCreated: new Date().toISOString().split("T")[0],
          dataSizeBytes: 15 * 1024 * 1024,
          eidCount: 5,
          segments: ["api", "v1"],
          segmentCount: 2,
          dataTags: ["eqp-demo"],
          tags: ["demo"],
          qpCount: 2,
          indexLists: 1,
          crud: { GET: 2, POST: 1, PUT: 0, PATCH: 0, DELETE: 1 },
        });
      }

      state.webs.forEach((item) => {
        if (!sizeUnits[item.id]) {
          sizeUnits[item.id] = "MB";
        }
      });
    } catch (error) {
      console.error("Failed to load Web View data:", error);

      state.webs = [];
    }
  }

  // ============================================================
  // FILTERING
  // ============================================================

  function applyFilters() {
    const term = state.search.trim().toLowerCase();

    state.filtered = state.webs.filter((web) => {
      const webTags = Array.isArray(web.tags) ? web.tags : [];
      const dataTags = Array.isArray(web.dataTags) ? web.dataTags : [];

      const matchesSearch =
        !term ||
        String(web.name || "")
          .toLowerCase()
          .includes(term) ||
        String(web.annotation || "")
          .toLowerCase()
          .includes(term) ||
        webTags.some((tag) => String(tag).toLowerCase().includes(term)) ||
        dataTags.some((tag) => String(tag).toLowerCase().includes(term));

      const matchesCrud =
        state.crud === "all" || Number(web.crud?.[state.crud] || 0) > 0;

      const matchesTags =
        state.tags.size === 0 ||
        [...state.tags].every((tag) => webTags.includes(tag));

      const webSegments = getWebSegments(web);

      const matchesSegments =
        state.segments.size === 0 ||
        [...state.segments].every((segment) => webSegments.includes(segment));

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
  // Falls back gracefully depending on what a given web entry
  // actually provides:
  //   1) web.endpoints (full endpoint list)
  //   2) web.segments (array of segment strings)
  //   3) web.segmentCount (precomputed number, non-filterable)
  //   4) [] if none of the above exist
  // ============================================================

  function getEndpointSegments(endpointPath) {
    return String(endpointPath || "")
      .split("/")
      .filter(Boolean);
  }

  function getWebSegments(web) {
    if (Array.isArray(web.endpoints)) {
      const segments = new Set();

      web.endpoints.forEach((endpoint) => {
        getEndpointSegments(endpoint.endpoint).forEach((segment) =>
          segments.add(segment),
        );
      });

      return [...segments];
    }

    if (Array.isArray(web.segments)) {
      return web.segments;
    }

    return [];
  }

  function getWebSegmentCount(web) {
    if (typeof web.segmentCount === "number") return web.segmentCount;

    const segments = getWebSegments(web);

    if (segments.length) return segments.length;

    if (typeof web.segments === "number") return web.segments;

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
    const tbody = document.getElementById("webTableBody");

    if (!tbody) return;

    tbody.innerHTML = "";

    const items = currentPageItems();

    if (items.length === 0) {
      tbody.innerHTML = `
                <tr>
                    <td colspan="18" class="px-4 py-16 text-center">
                        <p class="text-sm font-medium text-slate-500 dark:text-slate-400">No web views found</p>
                        <p class="mt-1 text-xs text-slate-400 dark:text-slate-600">Try changing the current search or filters.</p>
                    </td>
                </tr>
            `;

      updateSelectAllState();

      return;
    }

    items.forEach((web) => {
      tbody.appendChild(createWebRow(web));
    });

    updateSelectAllState();
  }

  function createWebRow(web) {
    const row = document.createElement("tr");

    const id = String(web.id || web.name);

    const isSelected = state.selected.has(id);
    const isActive = state.activeRowId === id;

    row.dataset.id = id;

    row.className = [
      "web-row",
      "border-b",
      "border-slate-200 dark:border-slate-800",
      "transition-colors",
      "duration-100",
      "cursor-pointer",
      "hover:bg-slate-100 dark:hover:bg-slate-800/60",
      isSelected ? "bg-cyan-500/[0.045]" : "",
      isActive ? "bg-slate-200/70 dark:bg-slate-800/80 ring-1 ring-inset ring-cyan-500/50" : "",
    ]
      .filter(Boolean)
      .join(" ");

    const dataTagsCount = (web.dataTags || []).length;
    const segmentCount = getWebSegmentCount(web);

    row.innerHTML = `

            <!-- Select -->
            <td class="px-2 py-2 align-middle">
                <input type="checkbox" class="web-select h-3.5 w-3.5 accent-slate-900 dark:accent-cyan-400" ${isSelected ? "checked" : ""}>
            </td>

            <!-- Folder Name -->
            <td class="px-2 py-2 font-medium text-slate-800 dark:text-slate-200">
                <button
                    type="button"
                    class="web-name-btn truncate text-left hover:text-cyan-600 dark:hover:text-cyan-400"
                    title="${escapeAttr(web.name)}">
                    ${escapeHtml(web.name)}
                </button>
            </td>

            <!-- Annotation -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="web-annotation-btn block max-w-xs truncate text-left text-[11px] text-slate-500 hover:text-cyan-600 dark:hover:text-cyan-400"
                    title="${escapeAttr(web.annotation || "")}">
                    ${escapeHtml(web.annotation || "—")}
                </button>
            </td>

            <!-- Tags -->
            <td class="px-2 py-2">
                <div class="flex flex-wrap gap-1">
                    ${renderWebTags(web)}
                </div>
            </td>

            <!-- Date Created -->
            <td class="px-2 py-2 text-slate-500 whitespace-nowrap">
                ${escapeHtml(web.dateCreated || "—")}
            </td>

            <!-- Size -->
            <td class="px-2 py-2">
                <button type="button" class="web-size-btn text-slate-800 dark:text-slate-300 hover:text-cyan-600 dark:hover:text-cyan-400">
                    ${formatSize(web.dataSizeBytes, sizeUnits[web.id])}
                </button>
            </td>

            <!-- GET -->
            <td class="px-1 py-2 text-center border-l border-slate-200 dark:border-slate-800">${crudCountBadge(web.crud?.GET, "text-emerald-600 dark:text-emerald-400")}</td>
            <!-- POST -->
            <td class="px-1 py-2 text-center">${crudCountBadge(web.crud?.POST, "text-sky-600 dark:text-sky-400")}</td>
            <!-- PUT -->
            <td class="px-1 py-2 text-center">${crudCountBadge(web.crud?.PUT, "text-amber-600 dark:text-amber-400")}</td>
            <!-- PATCH -->
            <td class="px-1 py-2 text-center">${crudCountBadge(web.crud?.PATCH, "text-violet-600 dark:text-violet-400")}</td>
            <!-- DELETE -->
            <td class="px-1 py-2 text-center">${crudCountBadge(web.crud?.DELETE, "text-rose-600 dark:text-rose-400")}</td>

            <!-- EID Count -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${web.eidCount ?? 0}</td>

            <!-- Segments -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${segmentCount}</td>

            <!-- Data Tags -->
            <td class="px-2 py-2">
                <button
                    type="button"
                    class="web-datatags-btn rounded-md border border-slate-300 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 px-2 py-0.5 text-[11px] text-slate-800 dark:text-slate-300 transition hover:border-cyan-500/60 hover:text-cyan-600 dark:hover:text-cyan-400">
                    ${dataTagsCount} tag${dataTagsCount === 1 ? "" : "s"}
                </button>
            </td>

            <!-- QP Count -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${web.qpCount ?? 0}</td>

            <!-- Index Lists -->
            <td class="px-2 py-2 text-slate-800 dark:text-slate-300">${web.indexLists ?? 0}</td>

            <!-- Preview -->
            <td class="px-2 py-2 text-center">
                <button
                    type="button"
                    class="web-action-preview-btn inline-flex items-center gap-1.5 rounded-md border border-cyan-500/30 bg-cyan-500/10 px-2 py-1 text-[11px] font-medium text-cyan-600 dark:text-cyan-400 transition hover:bg-cyan-500/20 hover:text-cyan-700 dark:hover:text-cyan-300"
                    title="Preview Webview"
                >
                    <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                        <circle cx="12" cy="12" r="3" />
                    </svg>
                    Preview
                </button>
            </td>

            <!-- Action -->
            <td class="px-2 py-2 text-center">
                <button
                    type="button"
                    class="web-action-edit-btn inline-flex items-center gap-1.5 rounded-md border border-cyan-500/30 bg-cyan-500/10 px-2 py-1 text-[11px] font-medium text-cyan-600 dark:text-cyan-400 transition hover:bg-cyan-500/20 hover:text-cyan-700 dark:hover:text-cyan-300"
                    title="Edit Webview"
                >
                    <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
                    </svg>
                    Edit
                </button>
            </td>

        `;

    // --------------------------------------------------------
    // Click row -> make active
    // --------------------------------------------------------

    row.addEventListener("click", (event) => {
      if (event.target.closest("button, input, select, a")) return;
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

    const checkbox = row.querySelector(".web-select");

    checkbox.addEventListener("click", (event) => event.stopPropagation());

    checkbox.addEventListener("change", () => {
      if (checkbox.checked) state.selected.add(id);
      else state.selected.delete(id);

      renderSelectionUI();
      updateSelectAllState();
      renderTable();
    });

    // --------------------------------------------------------
    // Folder name -> edit modal
    // --------------------------------------------------------

    row.querySelector(".web-name-btn").addEventListener("click", (event) => {
      event.stopPropagation();
      openEditWebModal(id);
    });

    // --------------------------------------------------------
    // Preview -> preview function (stub)
    // --------------------------------------------------------

    row
      .querySelector(".web-action-preview-btn")
      ?.addEventListener("click", (event) => {
        event.stopPropagation();
        previewWebView(id);
      });

    // --------------------------------------------------------
    // Action -> edit modal
    // --------------------------------------------------------

    row
      .querySelector(".web-action-edit-btn")
      ?.addEventListener("click", (event) => {
        event.stopPropagation();
        openEditWebFrontPageModal(id);
        // openEditWebModal(id);
      });

    // --------------------------------------------------------
    // Annotation -> annotation modal
    // --------------------------------------------------------

    row
      .querySelector(".web-annotation-btn")
      .addEventListener("click", (event) => {
        event.stopPropagation();
        openAnnotationModal(id);
      });

    // --------------------------------------------------------
    // Size -> cycle unit
    // --------------------------------------------------------

    row.querySelector(".web-size-btn").addEventListener("click", (event) => {
      event.stopPropagation();
      cycleSizeUnit(id);
    });

    // --------------------------------------------------------
    // Data tags -> popup list
    // --------------------------------------------------------

    row
      .querySelector(".web-datatags-btn")
      .addEventListener("click", (event) => {
        event.stopPropagation();
        openDataTagsModal(id);
      });

    // --------------------------------------------------------
    // Tags -> filter
    // --------------------------------------------------------

    row.querySelectorAll(".web-tag").forEach((button) => {
      button.addEventListener("click", (event) => {
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

  function renderWebTags(web) {
    const tags = Array.isArray(web.tags) ? web.tags : [];

    if (!tags.length) {
      return `<span class="text-[10px] text-slate-400 dark:text-slate-600">—</span>`;
    }

    return tags
      .map(
        (tag) => `
            <button
                type="button"
                class="web-tag inline-flex rounded bg-sky-100 dark:bg-sky-900/40 px-1.5 py-0.5 text-[10px] text-sky-700 dark:text-sky-300 transition hover:bg-sky-500/20 hover:text-sky-800 dark:hover:text-sky-200"
                data-tag="${escapeAttr(tag)}">
                ${escapeHtml(tag)}
            </button>
        `,
      )
      .join("");
  }

  function crudCountBadge(count, color) {
    const value = Number(count || 0);

    return `
            <span class="inline-flex min-w-6 items-center justify-center rounded-md bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 text-[10px] ${value > 0 ? color : "text-slate-400 dark:text-slate-600"}">
                ${value}
            </span>
        `;
  }

  // ============================================================
  // DATA TAGS POPUP
  // ============================================================

  function openDataTagsModal(id) {
    const web = getWeb(id);

    if (!web) return;

    const tags = web.dataTags || [];

    openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">EQP Tags</h2>

            <div class="flex flex-wrap gap-2">
                ${tags.length
        ? tags
          .map(
            (tag) => `
                            <span class="rounded-md border border-slate-300 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 px-2.5 py-1 text-xs text-slate-800 dark:text-slate-300">
                                ${escapeHtml(tag)}
                            </span>
                        `,
          )
          .join("")
        : '<span class="text-xs text-slate-400 dark:text-slate-600">No EQP Tags.</span>'
      }
            </div>

            <div class="mt-4 flex justify-end">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">
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

    const start = total ? (state.page - 1) * PAGE_SIZE + 1 : 0;

    const end = Math.min(state.page * PAGE_SIZE, total);

    document.getElementById("webRangeStart").textContent = start;
    document.getElementById("webRangeEnd").textContent = end;
    document.getElementById("webTotal").textContent = total;

    const totalPages = getTotalPages();

    const numbers = document.getElementById("webPagination");

    numbers.innerHTML = "";

    getPageRange(state.page, totalPages).forEach((page) => {
      if (page === "...") {
        const span = document.createElement("span");
        span.className = "px-1 text-slate-400 dark:text-slate-600";
        span.textContent = "…";
        numbers.appendChild(span);
        return;
      }

      const button = document.createElement("button");
      button.type = "button";
      button.textContent = page;

      button.className =
        page === state.page
          ? "min-w-7 rounded-md bg-slate-900 dark:bg-cyan-500 px-2 py-1 text-[11px] text-white"
          : "min-w-7 rounded-md border border-slate-300 dark:border-slate-700 px-2 py-1 text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-white";

      button.addEventListener("click", () => {
        state.page = page;
        render();
      });

      numbers.appendChild(button);
    });

    document.getElementById("webPrev").disabled = state.page <= 1;
    document.getElementById("webNext").disabled = state.page >= totalPages;
  }

  function getPageRange(current, total) {
    if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);

    const pages = [1];

    if (current > 4) pages.push("...");

    const start = Math.max(2, current - 1);
    const end = Math.min(total - 1, current + 1);

    for (let page = start; page <= end; page++) pages.push(page);

    if (current < total - 3) pages.push("...");

    pages.push(total);

    return pages;
  }

  // ============================================================
  // SIDEBAR
  // ============================================================

  function wireSidebar() {
    document
      .getElementById("sidebarToggle")
      ?.addEventListener("click", toggleSidebar);
  }

  function toggleSidebar() {
    const sidebar = document.getElementById("filterSidebar");
    const content = document.getElementById("sidebarContent");
    const title = document.getElementById("sidebarTitle");
    const icon = document.getElementById("sidebarToggleIcon");

    state.sidebarCollapsed = !state.sidebarCollapsed;

    if (state.sidebarCollapsed) {
      sidebar.classList.remove("w-64");
      sidebar.classList.add("w-10");
      content.classList.add("hidden");
      title.classList.add("hidden");
      icon.innerHTML = `<path d="m9 18 6-6-6-6"/>`;
    } else {
      sidebar.classList.remove("w-10");
      sidebar.classList.add("w-64");
      content.classList.remove("hidden");
      title.classList.remove("hidden");
      icon.innerHTML = `<path d="m15 18-6-6 6-6"/>`;
    }
  }

  function renderSidebar() {
    renderTags();
    renderSegments();
    updateStats();
  }

  function renderTags() {
    const container = document.getElementById("tagsContainer");

    if (!container) return;

    const counts = {};

    state.webs.forEach((web) => {
      (web.tags || []).forEach((tag) => {
        if (!tag) return;
        counts[tag] = (counts[tag] || 0) + 1;
      });
    });

    container.innerHTML = "";

    Object.entries(counts)
      .sort((a, b) => a[0].localeCompare(b[0]))
      .forEach(([tag, count]) => {
        const wrapper = document.createElement("label");
        wrapper.className = "group flex cursor-pointer items-center gap-2";

        wrapper.innerHTML = `
                    <input type="checkbox" class="web-tag-checkbox h-3.5 w-3.5 accent-slate-900 dark:accent-cyan-400" data-tag="${escapeAttr(tag)}">
                    <button type="button" class="web-tag-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-sky-500/10 hover:text-sky-700 dark:hover:text-sky-300" data-tag="${escapeAttr(tag)}">
                        ${escapeHtml(tag)} <span class="text-slate-400 dark:text-slate-600">(${count})</span>
                    </button>
                `;

        const checkbox = wrapper.querySelector(".web-tag-checkbox");
        const button = wrapper.querySelector(".web-tag-filter");

        checkbox.addEventListener("change", () =>
          setTagFilter(tag, checkbox.checked),
        );

        button.addEventListener("click", (event) => {
          event.preventDefault();
          const enabled = !state.tags.has(tag);
          checkbox.checked = enabled;
          setTagFilter(tag, enabled);
        });

        container.appendChild(wrapper);
      });
  }

  function renderSegments() {
    const container = document.getElementById("segmentsContainer");

    if (!container) return;

    const counts = {};

    state.webs.forEach((web) => {
      getWebSegments(web).forEach((segment) => {
        counts[segment] = (counts[segment] || 0) + 1;
      });
    });

    container.innerHTML = "";

    if (Object.keys(counts).length === 0) {
      container.innerHTML = `<span class="text-[11px] text-slate-400 dark:text-slate-600">No segment data available.</span>`;
      return;
    }

    Object.entries(counts)
      .sort((a, b) => a[0].localeCompare(b[0]))
      .forEach(([segment, count]) => {
        const wrapper = document.createElement("label");
        wrapper.className = "group flex cursor-pointer items-center gap-2";

        wrapper.innerHTML = `
                    <input type="checkbox" class="web-segment-checkbox h-3.5 w-3.5 accent-slate-900 dark:accent-cyan-400" data-segment="${escapeAttr(segment)}">
                    <button type="button" class="web-segment-filter min-w-0 flex-1 truncate rounded px-1.5 py-1 text-left text-[11px] text-slate-500 dark:text-slate-400 transition hover:bg-cyan-500/10 hover:text-cyan-700 dark:hover:text-cyan-300" data-segment="${escapeAttr(segment)}">
                        ${escapeHtml(segment)} <span class="text-slate-400 dark:text-slate-600">(${count})</span>
                    </button>
                `;

        const checkbox = wrapper.querySelector(".web-segment-checkbox");
        const button = wrapper.querySelector(".web-segment-filter");

        checkbox.addEventListener("change", () =>
          setSegmentFilter(segment, checkbox.checked),
        );

        button.addEventListener("click", (event) => {
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
    document.querySelectorAll(".web-tag-checkbox").forEach((checkbox) => {
      checkbox.checked = state.tags.has(checkbox.dataset.tag);
    });

    document.querySelectorAll(".web-segment-checkbox").forEach((checkbox) => {
      checkbox.checked = state.segments.has(checkbox.dataset.segment);
    });
  }

  let activeTab = "web";

  function wireSidebarTabs() {
    const tabRepo = document.getElementById("activeTabRepo");
    const tabEqp = document.getElementById("activeTabEqp");
    const contentRepo = document.getElementById("activeContentRepo");
    const contentEqp = document.getElementById("activeContentEqp");

    if (!tabRepo || !tabEqp) return;

    function updateTabs() {
      if (activeTab === "web") {
        tabRepo.className =
          "flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium bg-white dark:bg-slate-700 text-slate-900 dark:text-white shadow-sm transition";
        tabEqp.className =
          "flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 transition";
        contentRepo.classList.remove("hidden");
        contentEqp.classList.add("hidden");
      } else {
        tabEqp.className =
          "flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium bg-white dark:bg-slate-700 text-slate-900 dark:text-white shadow-sm transition";
        tabRepo.className =
          "flex-1 rounded-sm px-2 py-1.5 text-[10px] font-medium text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 transition";
        contentEqp.classList.remove("hidden");
        contentRepo.classList.add("hidden");
      }
    }

    tabRepo.addEventListener("click", () => {
      activeTab = "web";
      updateTabs();
    });
    tabEqp.addEventListener("click", () => {
      activeTab = "eqp";
      updateTabs();
    });
  }

  function renderActiveRowData() {
    const section = document.getElementById("sidebarActiveSection");
    const contentRepo = document.getElementById("activeContentRepo");
    const contentEqp = document.getElementById("activeContentEqp");

    if (!section || !contentRepo || !contentEqp) return;

    if (!state.activeRowId) {
      section.classList.add("hidden");
      return;
    }

    const web = getWeb(state.activeRowId);

    if (!web) {
      section.classList.add("hidden");
      return;
    }

    section.classList.remove("hidden");

    const tagsHtml = (web.tags || [])
      .map(
        (t) =>
          `<span class="rounded bg-sky-100 dark:bg-sky-900/40 px-1.5 py-0.5 text-[10px] text-sky-700 dark:text-sky-300">${escapeHtml(t)}</span>`,
      )
      .join("");
    const eqpTagsHtml = (web.dataTags || [])
      .map(
        (t) =>
          `<span class="rounded bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-300">${escapeHtml(t)}</span>`,
      )
      .join("");
    const segments = Array.isArray(web.segments) ? web.segments : [];
    const eqpSegmentsHtml = segments
      .map(
        (segment) => `
            <span class="rounded bg-slate-100 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 px-1.5 py-1 text-[11px] text-slate-800 dark:text-slate-300">
                ${escapeHtml(segment)}
            </span>
        `,
      )
      .join("");

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
    const webStat = document.getElementById("webStat");
    const tagStat = document.getElementById("tagStat");
    const segmentStat = document.getElementById("segmentStat");

    const tags = new Set();
    const segments = new Set();

    state.webs.forEach((web) => {
      (web.tags || []).forEach((tag) => tags.add(tag));

      getWebSegments(web).forEach((segment) => segments.add(segment));
    });

    if (webStat) webStat.textContent = state.webs.length;
    if (tagStat) tagStat.textContent = tags.size;
    if (segmentStat) segmentStat.textContent = segments.size;
  }

  // ============================================================
  // SELECTION
  // ============================================================

  function wireSelection() {
    document
      .getElementById("clearWebSelection")
      ?.addEventListener("click", clearSelection);

    document
      .getElementById("combineWebSelection")
      ?.addEventListener("click", () => {
        combineWebs([...state.selected]);
      });

    document
      .getElementById("deleteWebSelection")
      ?.addEventListener("click", () => {
        openDeleteModal([...state.selected]);
      });

    document
      .getElementById("selectAllWebs")
      ?.addEventListener("change", (event) => {
        currentPageItems().forEach((web) => {
          const id = String(web.id || web.name);
          if (event.target.checked) state.selected.add(id);
          else state.selected.delete(id);
        });

        render();
      });
  }

  function clearSelection() {
    state.selected.clear();

    render();
  }

  function renderSelectionUI() {
    const bar = document.getElementById("selectionBar");
    const count = document.getElementById("selectedWebCount");
    const list = document.getElementById("selectedWebList");

    const selected = getSelectedWebs();

    if (selected.length) {
      bar?.classList.remove("hidden");
      bar?.classList.add("flex");
    } else {
      bar?.classList.add("hidden");
      bar?.classList.remove("flex");
    }

    if (count) count.textContent = `${selected.length} selected`;

    if (list) {
      list.innerHTML = selected
        .map(
          (web) => `
                <span class="shrink-0 rounded-md border border-cyan-500/20 bg-cyan-500/10 px-2 py-1 font-mono text-[10px] text-cyan-700 dark:text-cyan-300">
                    ${escapeHtml(web.name)}
                </span>
            `,
        )
        .join("");
    }

    const combineBtn = document.getElementById("combineWebSelection");
    if (combineBtn) {
      if (selected.length >= 2) {
        combineBtn.classList.remove("hidden");
      } else {
        combineBtn.classList.add("hidden");
      }
    }

    const deleteBtn = document.getElementById("deleteWebSelection");
    if (deleteBtn) {
      if (selected.length >= 1) {
        deleteBtn.classList.remove("hidden");
      } else {
        deleteBtn.classList.add("hidden");
      }
    }
  }

  function updateSelectAllState() {
    const selectAll = document.getElementById("selectAllWebs");

    if (!selectAll) return;

    const items = currentPageItems();

    const selectedCount = items.filter((web) =>
      state.selected.has(String(web.id || web.name)),
    ).length;

    selectAll.checked = items.length > 0 && selectedCount === items.length;

    selectAll.indeterminate = selectedCount > 0 && selectedCount < items.length;
  }

  function getSelectedWebs() {
    return state.webs.filter((web) => state.selected.has(String(web.id || web.name)));
  }

  // ============================================================
  // TOOLBAR
  // ============================================================

  function wireToolbar() {
    document.getElementById("webSearch")?.addEventListener("input", (event) => {
      state.search = event.target.value;
      state.page = 1;
      applyFilters();
    });

    document
      .getElementById("crudFilter")
      ?.addEventListener("change", (event) => {
        state.crud = event.target.value;
        state.page = 1;
        applyFilters();
      });

    document
      .getElementById("resetFilters")
      ?.addEventListener("click", resetFilters);

    document
      .getElementById("createWebBtn")
      ?.addEventListener("click", openCreateModal);

    document.getElementById("webPrev")?.addEventListener("click", () => {
      if (state.page > 1) {
        state.page--;
        render();
      }
    });

    document.getElementById("webNext")?.addEventListener("click", () => {
      const totalPages = getTotalPages();
      if (state.page < totalPages) {
        state.page++;
        render();
      }
    });
  }

  function resetFilters() {
    state.search = "";
    state.crud = "all";
    state.tags.clear();
    state.segments.clear();
    state.page = 1;

    const search = document.getElementById("webSearch");
    if (search) search.value = "";

    const crud = document.getElementById("crudFilter");
    if (crud) crud.value = "all";

    applyFilters();
  }

  // ============================================================
  // TAG INPUT COMPONENT HELPER
  // ============================================================

  function setupTagInput({ listElId, inputElId, addBtnElId, initialTags = [] }) {
    const listEl = document.getElementById(listElId);
    const inputEl = document.getElementById(inputElId);
    const addBtnEl = document.getElementById(addBtnElId);
    const tags = [...new Set((initialTags || []).map((t) => String(t).trim()).filter(Boolean))];

    function render() {
      if (!listEl) return;
      listEl.innerHTML = tags
        .map(
          (tag, index) => `
          <span class="inline-flex items-center gap-1.5 rounded-md
             border border-cyan-500/30
             bg-cyan-500/10
             px-2 py-0.5 text-xs
             text-cyan-700 dark:text-cyan-300">
  <span>${escapeHtml(tag)}</span>
  <button
    type="button"
    data-remove-tag-index="${index}"
    class="ml-0.5 rounded px-1 leading-none
           text-cyan-600 dark:text-cyan-400
           hover:bg-cyan-500/20
           hover:text-cyan-900 dark:hover:text-white
           transition cursor-pointer"
    title="Remove ${escapeAttr(tag)}"
  >&times;</button>
</span>
          `,
        )
        .join("");
    }

    function addTagFromInput() {
      if (!inputEl) return;
      const raw = inputEl.value;
      if (!raw || !raw.trim()) return;
      const items = raw
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
      for (const item of items) {
        if (!tags.includes(item)) {
          tags.push(item);
        }
      }
      inputEl.value = "";
      render();
    }

    inputEl?.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        event.stopPropagation();
        addTagFromInput();
      }
    });

    addBtnEl?.addEventListener("click", (event) => {
      event.preventDefault();
      addTagFromInput();
    });

    listEl?.addEventListener("click", (event) => {
      const btn = event.target.closest("[data-remove-tag-index]");
      if (!btn) return;
      event.preventDefault();
      const idx = parseInt(btn.dataset.removeTagIndex, 10);
      if (!isNaN(idx) && idx >= 0 && idx < tags.length) {
        tags.splice(idx, 1);
        render();
      }
    });

    render();

    return {
      getTags: () => {
        addTagFromInput();
        return [...tags];
      }
    };
  }

  // ============================================================
  // CREATE
  // ============================================================

  let activeCreateTagHelper = null;

  async function openCreateModal() {
    openModal(`
      <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Create Web View</h2>

      <label class="mb-1 block text-[11px] text-slate-500">Folder Name <span class="text-rose-600 dark:text-rose-400">*</span></label>
      <input id="createName" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200" placeholder="e.g. Production APIs">

      <label class="mb-1 block text-[11px] text-slate-500">Source Collection <span class="text-rose-600 dark:text-rose-400">*</span></label>
      <select id="createSourceCollection" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">
        <option value="" disabled selected>Loading collections...</option>
      </select>

   <label class="mb-1 block text-[11px] text-slate-500">
  Tags
</label>

<div id="createTagsList"
     class="mb-2 flex flex-wrap gap-1.5 empty:hidden">
</div>

<div class="mb-3 flex items-center gap-2">
 <input
  id="createTags"
  class="h-9 flex-1 rounded-md
         border border-slate-300 dark:border-slate-700
         !bg-white dark:!bg-slate-950
         px-3 text-xs
         !text-slate-800 dark:!text-slate-200
         placeholder:text-slate-400 dark:placeholder:text-slate-600
         outline-none focus:border-cyan-500"
  placeholder="Type a tag and press Enter"
/>
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
      <textarea id="createAnnotation" class="mb-4 h-20 w-full resize-none rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 py-2 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200" placeholder="Web view description..."></textarea>

      <div class="flex justify-end gap-2">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
        <button id="confirmCreate" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Create</button>
      </div>
    `);

    activeCreateTagHelper = setupTagInput({
      listElId: "createTagsList",
      inputElId: "createTags",
      addBtnElId: "createAddTagBtn",
      initialTags: []
    });

    document
      .getElementById("confirmCreate")
      ?.addEventListener("click", () => createWeb(activeCreateTagHelper));

    const select = document.getElementById("createSourceCollection");
    try {
      let collections = [];
      if (window.EdmsAPI && typeof window.EdmsAPI.listCollections === "function") {
        const res = await window.EdmsAPI.listCollections();
        if (res && res.ok && res.data) {
          if (Array.isArray(res.data.items)) {
            collections = res.data.items.map((c) => (typeof c === "string" ? c : c.name));
          } else if (Array.isArray(res.data)) {
            collections = res.data.map((c) => (typeof c === "string" ? c : c.name));
          }
        }
      }
      if (select) {
        if (collections.length === 0) {
          select.innerHTML = '<option value="" disabled selected>No collections available</option>';
        } else {
          select.innerHTML =
            '<option value="" disabled selected>Select a collection...</option>' +
            collections
              .map((c) => `<option value="${escapeAttr(c)}">${escapeHtml(c)}</option>`)
              .join("");
        }
      }
    } catch (err) {
      console.warn("Failed to load collections for create modal:", err);
      if (select) {
        select.innerHTML = '<option value="" disabled selected>Failed to load collections</option>';
      }
    }
  }

  async function createWeb(tagHelper) {
    const name = document.getElementById("createName")?.value.trim();
    const sourceCollection = document.getElementById("createSourceCollection")?.value.trim();

    if (!name) {
      showAlert("Folder name is required.");
      return;
    }

    if (!sourceCollection) {
      showAlert("Source collection is required.");
      return;
    }

    if (
      state.webs.some((web) => web.name.toLowerCase() === name.toLowerCase())
    ) {
      showAlert("A Web View with this name already exists.");
      return;
    }

    try {
      const annotation = document.getElementById("createAnnotation")?.value.trim() || undefined;
      const res = await window.EdmsAPI.createWebview({
        name,
        source_collection: sourceCollection,
        annotation
      });

      if (!res.ok) {
        const errMsg = res.data?.error || res.data?.message || `Failed to create Web View via API (HTTP ${res.status})`;
        showAlert(errMsg);
        return;
      }

      const tags = tagHelper
        ? tagHelper.getTags()
        : (document.getElementById("createTags")?.value || "")
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean);

      for (const t of tags) {
        if (window.EdmsAPI && typeof window.EdmsAPI.addWebviewMembershipTag === "function") {
          try {
            await window.EdmsAPI.addWebviewMembershipTag(name, t);
          } catch (e) {
            console.warn("Failed to add membership tag on create:", e);
          }
        }
      }

      showAlert(`Web View "${name}" created successfully.`);
      closeModal();
      await loadData();

      state.selected.clear();
      state.page = 1;

      renderSidebar();
      applyFilters();
    } catch (error) {
      console.error(error);
      showAlert(`Error creating web view: ${error.message}`);
    }
  }

  // ============================================================
  // DUPLICATE
  // ============================================================

  async function duplicateWeb(id) {
    const source = getWeb(id);

    if (!source) return;

    let copyName = `${source.name} (copy)`;
    let counter = 2;

    while (
      state.webs.some(
        (web) => web.name.toLowerCase() === copyName.toLowerCase(),
      )
    ) {
      copyName = `${source.name} (copy ${counter})`;
      counter++;
    }

    try {
      if (window.EdmsAPI && typeof window.EdmsAPI.duplicateWebview === "function") {
        const res = await window.EdmsAPI.duplicateWebview(source.name, copyName);
        if (res && !res.ok) {
          const errMsg = res.data?.error || res.data?.message || `Failed to duplicate web view (HTTP ${res.status})`;
          showAlert(errMsg);
          return;
        }
      }
    } catch (err) {
      showAlert(`Error duplicating web view: ${err.message}`);
      return;
    }

    // Refresh from backend or keep in local memory if endpoint is pending backend implementation
    const prevCount = state.webs.length;
    await loadData();
    if (state.webs.length === prevCount) {
      const copy = structuredClone(source);
      copy.id = copyName;
      copy.name = copyName;
      copy.dateCreated = new Date().toISOString().slice(0, 10);
      state.webs.unshift(copy);
      sizeUnits[copy.id] = "MB";
    }

    state.selected.clear();
    state.page = 1;

    renderSidebar();
    applyFilters();
  }

  // ============================================================
  // DELETE
  // ============================================================

  function openDeleteModal(ids) {
    const webs = ids.map(getWeb).filter(Boolean);

    if (!webs.length) return;

    openModal(`

            <div class="flex items-start gap-3 rounded-lg border border-rose-500/20 bg-rose-500/5 p-3">
                <div class="mt-0.5 text-rose-600 dark:text-rose-400">${iconTrash()}</div>
                <div>
                    <h2 class="text-sm font-semibold text-rose-600 dark:text-rose-300">Delete ${webs.length > 1 ? "Web Views" : "Web View"}</h2>
                    <p class="mt-1 text-[11px] text-slate-500">This removes the selected web view(s) from the system.</p>
                </div>
            </div>

            <div class="my-4 max-h-48 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800">
                ${webs
        .map(
          (web) => `
                    <div class="border-b border-slate-200 dark:border-slate-800 px-3 py-2 last:border-0">
                        <p class="font-mono text-xs text-slate-800 dark:text-slate-300">${escapeHtml(web.name)}</p>
                    </div>
                `,
        )
        .join("")}
            </div>

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="confirmDelete" type="button" class="rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-rose-500">Delete</button>
            </div>

        `);

    document.getElementById("confirmDelete")?.addEventListener("click", async () => {
      await deleteWebs(ids);
      closeModal();
    });
  }

  async function deleteWebs(ids) {
    const normalizedIds = ids.map(String);
    const websToDelete = state.webs.filter((web) => normalizedIds.includes(String(web.id || web.name)));
    const names = websToDelete.map((w) => w.name || w.id).filter(Boolean);

    if (names.length === 0) return;

    try {
      if (window.EdmsAPI) {
        let res;
        if (names.length === 1 && typeof window.EdmsAPI.deleteWebview === "function") {
          res = await window.EdmsAPI.deleteWebview(names[0]);
        } else if (typeof window.EdmsAPI.deleteWebviews === "function") {
          res = await window.EdmsAPI.deleteWebviews(names);
        }
        if (res && !res.ok) {
          const errMsg = res.data?.error || res.data?.message || `Failed to delete web view (HTTP ${res.status})`;
          showAlert(errMsg);
          return;
        }
      }
    } catch (err) {
      showAlert(`Error deleting web view: ${err.message}`);
      return;
    }

    // Refresh from backend, plus clean up local state
    await loadData();
    state.webs = state.webs.filter(
      (web) => !normalizedIds.includes(String(web.id || web.name)) && !names.includes(web.name),
    );
    normalizedIds.forEach((id) => state.selected.delete(id));

    renderSidebar();
    applyFilters();
  }

  // ============================================================
  // EDIT NAME / TAGS
  // ============================================================

  function openEditWebModal(id) {
    const web = getWeb(id);

    if (!web) return;

    openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Edit Web View</h2>

            <label class="mb-1 block text-[11px] text-slate-500">Folder Name</label>
            <input id="editWebName" value="${escapeAttr(web.name)}" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500">

            <label class="mb-1 block text-[11px] text-slate-500">Tags</label>
            <div id="editWebTagsList" class="flex flex-wrap gap-1.5 mb-2 empty:hidden"></div>
            <div class="flex items-center gap-2 mb-4">
              <input
  id="editWebTags"
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
  id="editWebAddTagBtn"
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

            <div class="flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="saveWebEdit" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save</button>
            </div>

        `);

    const editTagHelper = setupTagInput({
      listElId: "editWebTagsList",
      inputElId: "editWebTags",
      addBtnElId: "editWebAddTagBtn",
      initialTags: Array.isArray(web.tags) ? web.tags : []
    });

    document.getElementById("saveWebEdit")?.addEventListener("click", async () => {
      const newName = document.getElementById("editWebName").value.trim();

      if (!newName) {
        showAlert("Folder name is required.");
        return;
      }

      const duplicate = state.webs.some(
        (item) =>
          String(item.id || item.name) !== String(web.id || web.name) &&
          item.name.toLowerCase() === newName.toLowerCase(),
      );

      if (duplicate) {
        showAlert("Folder name must be unique.");
        return;
      }

      const oldName = web.name;
      if (oldName !== newName) {
        try {
          if (window.EdmsAPI && typeof window.EdmsAPI.renameWebview === "function") {
            const res = await window.EdmsAPI.renameWebview(oldName, newName);
            if (res && !res.ok) {
              const errMsg = res.data?.error || res.data?.message || `Failed to rename web view (HTTP ${res.status})`;
              showAlert(errMsg);
              return;
            }
          }
        } catch (err) {
          showAlert(`Error renaming web view: ${err.message}`);
          return;
        }
      }

      const currentTags = Array.isArray(web.tags) ? web.tags : [];
      const newTags = editTagHelper
        ? editTagHelper.getTags()
        : document
          .getElementById("editWebTags")
          ?.value.split(",")
          .map((tag) => tag.trim())
          .filter(Boolean) || [];

      const targetWebName = newName || web.name;

      if (window.EdmsAPI && typeof window.EdmsAPI.addWebviewMembershipTag === "function") {
        const addedTags = newTags.filter((t) => !currentTags.includes(t));
        const removedTags = currentTags.filter((t) => !newTags.includes(t));

        for (const t of addedTags) {
          try {
            await window.EdmsAPI.addWebviewMembershipTag(targetWebName, t);
          } catch (e) {
            console.warn("Failed to add membership tag:", e);
          }
        }
        for (const t of removedTags) {
          try {
            if (typeof window.EdmsAPI.removeWebviewMembershipTag === "function") {
              await window.EdmsAPI.removeWebviewMembershipTag(targetWebName, t);
            }
          } catch (e) {
            console.warn("Failed to remove membership tag:", e);
          }
        }
      }

      web.name = newName;
      web.id = newName;
      web.tags = newTags;

      closeModal();
      renderSidebar();
      applyFilters();
    });
  }

  // ============================================================
  // ANNOTATION
  // ============================================================

  function openAnnotationModal(id) {
    const web = getWeb(id);

    if (!web) return;

    openModal(`

            <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Annotation</h2>

            <p class="mb-2 text-[11px] text-slate-500">Markdown annotation</p>

            <textarea id="annotationEditor" class="h-64 w-full resize-none rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 py-2 font-mono text-sm text-slate-800 dark:text-slate-300 outline-none focus:border-cyan-500">${escapeHtml(web.annotation || "")}</textarea>

            <div class="mt-4 flex justify-end gap-2">
                <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
                <button id="saveAnnotation" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Save</button>
            </div>

        `);

    document.getElementById("saveAnnotation")?.addEventListener("click", async () => {
      const annotationVal = document.getElementById("annotationEditor").value;

      try {
        if (window.EdmsAPI && typeof window.EdmsAPI.setWebviewAnnotation === "function") {
          const res = await window.EdmsAPI.setWebviewAnnotation(web.name, annotationVal);
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

      web.annotation = annotationVal;

      closeModal();
      render();
    });
  }

  // ============================================================
  // COMBINE
  // ============================================================

  function combineWebs(ids) {
    const targetIds = (ids && ids.length) ? ids : [...state.selected];
    const webs = targetIds.map(getWeb).filter(Boolean);

    if (webs.length < 2) {
      showAlert("Please select at least 2 Web Views to combine.");
      return;
    }

    const defaultName = `${webs[0].name}_combined`;

    openModal(`
      <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Combine Web Views</h2>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Combine ${webs.length} web views into a unified web view:</p>
      <div class="mb-4 max-h-36 overflow-y-auto rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/60 p-2 text-xs font-mono text-slate-800 dark:text-slate-300">
        ${webs.map((w) => `<div>• ${escapeHtml(w.name)}</div>`).join("")}
      </div>
      <label class="mb-1 block text-[11px] text-slate-500">Combined Web View Name <span class="text-rose-600 dark:text-rose-400">*</span></label>
      <input id="combinedWebName" value="${escapeAttr(defaultName)}" class="mb-4 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">
      <div class="flex justify-end gap-2">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
        <button id="confirmCombineWeb" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Combine</button>
      </div>
    `);

    document.getElementById("confirmCombineWeb")?.addEventListener("click", () => {
      const newName = document.getElementById("combinedWebName")?.value.trim();
      if (!newName) {
        showAlert("Combined name is required.");
        return;
      }
      executeCombine(webs, newName);
    });
  }

  async function executeCombine(webs, targetName, onExists, renameVal) {
    const sources = webs.map((w) => w.name);
    if (sources.length < 2) {
      showAlert("At least two sources are required to combine.");
      return;
    }

    const payload = {
      name: targetName,
      sources
    };

    if (onExists) {
      payload.on_exists = onExists;
      if (onExists === "rename" && renameVal) {
        payload.new_name = renameVal;
      }
    }

    try {
      const res = await window.EdmsAPI.combineWebviews(payload);
      if (!res.ok) {
        if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
          showCombineConflictModal(webs, targetName, res.data.options, res.data.error || `A Web View named "${targetName}" already exists.`);
          return;
        }
        const errMsg = res.data?.error || res.data?.message || `Failed to combine web views (HTTP ${res.status})`;
        showAlert(errMsg);
        return;
      }

      const finalName = (onExists === "rename" && renameVal) ? renameVal : targetName;
      showAlert(`Web views combined into "${finalName}".`);
      closeModal();
      await loadData();
      state.selected.clear();
      renderSidebar();
      applyFilters();
    } catch (err) {
      showAlert(`Error combining web views: ${err.message}`);
    }
  }

  function showCombineConflictModal(webs, targetName, options, errorMsg) {
    openModal(`
      <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
        <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: View Already Exists (409)</div>
        <div class="text-[11px] text-amber-700/80 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
      </div>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this conflict:</p>
      <div class="mb-4 space-y-3">
        ${options.includes("merge") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Merge</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Merge sources into the existing "${escapeHtml(targetName)}" Web View.</p>
            <button id="conflictMergeBtn" type="button" class="rounded bg-amber-600 px-2.5 py-1 text-xs text-white hover:bg-amber-500">Merge into "${escapeHtml(targetName)}"</button>
          </div>
        ` : ""}
        ${options.includes("rename") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Rename</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Save with a new, non-conflicting name.</p>
            <div class="flex gap-2">
              <input id="conflictRenameInput" value="${escapeAttr(targetName + "_1")}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
              <button id="conflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Combine</button>
            </div>
          </div>
        ` : ""}
      </div>
      <div class="flex justify-end">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
      </div>
    `);

    document.getElementById("conflictMergeBtn")?.addEventListener("click", () => {
      executeCombine(webs, targetName, "merge");
    });

    document.getElementById("conflictRenameBtn")?.addEventListener("click", () => {
      const renVal = document.getElementById("conflictRenameInput")?.value.trim();
      if (!renVal) {
        showAlert("New name is required.");
        return;
      }
      executeCombine(webs, targetName, "rename", renVal);
    });
  }

  // ============================================================
  // CONVERT TO COLLECTION
  // ============================================================

  function convertToCollection(id) {
    const web = getWeb(id);
    if (!web) return;

    openModal(`
      <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Convert to Collection</h2>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Convert Web View <strong class="text-slate-900 dark:text-white">${escapeHtml(web.name)}</strong> into a Collection.</p>
      
      <label class="mb-1 block text-[11px] text-slate-500">Collection Name <span class="text-rose-600 dark:text-rose-400">*</span></label>
      <input id="convertCollectionName" value="${escapeAttr(web.name)}" class="mb-4 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

      <div class="flex justify-end gap-2">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
        <button id="confirmConvertToCollection" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Convert</button>
      </div>
    `);

    document.getElementById("confirmConvertToCollection")?.addEventListener("click", () => {
      const colName = document.getElementById("convertCollectionName")?.value.trim();
      if (!colName) {
        showAlert("Collection name is required.");
        return;
      }
      executeConvert(web, colName);
    });
  }

  async function executeConvert(web, colName, onExists, renameVal) {
    const payload = { collection: colName };
    if (onExists) {
      payload.on_exists = onExists;
      if (onExists === "rename" && renameVal) {
        payload.new_name = renameVal;
      }
    }

    try {
      const res = await window.EdmsAPI.convertWebviewToCollection(web.name, payload);
      if (!res.ok) {
        if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
          showConvertConflictModal(web, colName, res.data.options, res.data.error || `Collection "${colName}" already exists.`);
          return;
        }
        const errMsg = res.data?.error || res.data?.message || `Failed to convert to collection (HTTP ${res.status})`;
        showAlert(errMsg);
        return;
      }

      const finalName = (onExists === "rename" && renameVal) ? renameVal : colName;
      showAlert(`Web View "${web.name}" successfully converted to Collection "${finalName}".`);
      closeModal();
      await loadData();
      state.selected.clear();
      renderSidebar();
      applyFilters();
    } catch (err) {
      showAlert(`Error converting to collection: ${err.message}`);
    }
  }

  function showConvertConflictModal(web, colName, options, errorMsg) {
    openModal(`
      <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
        <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: Collection Already Exists (409)</div>
        <div class="text-[11px] text-amber-700/80 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
      </div>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this conflict:</p>
      <div class="mb-4 space-y-3">
        ${options.includes("merge") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Merge</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Merge into the existing collection "${escapeHtml(colName)}".</p>
            <button id="convertConflictMergeBtn" type="button" class="rounded bg-amber-600 px-2.5 py-1 text-xs text-white hover:bg-amber-500">Merge into "${escapeHtml(colName)}"</button>
          </div>
        ` : ""}
        ${options.includes("rename") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Rename</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Convert to a collection with a new name.</p>
            <div class="flex gap-2">
              <input id="convertConflictRenameInput" value="${escapeAttr(colName + "_1")}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
              <button id="convertConflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Convert</button>
            </div>
          </div>
        ` : ""}
      </div>
      <div class="flex justify-end">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
      </div>
    `);

    document.getElementById("convertConflictMergeBtn")?.addEventListener("click", () => {
      executeConvert(web, colName, "merge");
    });

    document.getElementById("convertConflictRenameBtn")?.addEventListener("click", () => {
      const renVal = document.getElementById("convertConflictRenameInput")?.value.trim();
      if (!renVal) {
        showAlert("New name is required.");
        return;
      }
      executeConvert(web, colName, "rename", renVal);
    });
  }

  // ============================================================
  // TAKEOUT
  // ============================================================

  function takeoutWeb(id) {
    const web = getWeb(id);
    if (!web) return;

    openModal(`
      <h2 class="mb-4 text-sm font-semibold text-slate-800 dark:text-white">Takeout Web View</h2>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Export and takeout Web View <strong class="text-slate-900 dark:text-white">${escapeHtml(web.name)}</strong>.</p>
      
      <label class="mb-1 block text-[11px] text-slate-500">Destination Folder Name (Optional)</label>
      <input id="takeoutDestName" value="${escapeAttr(web.name)}" placeholder="${escapeAttr(web.name)}" class="mb-3 h-9 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-3 text-xs outline-none focus:border-cyan-500 text-slate-800 dark:text-slate-200">

      <label class="mb-4 flex items-center gap-2 cursor-pointer">
        <input id="takeoutOverwrite" type="checkbox" class="h-4 w-4 rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-cyan-600 focus:ring-0 focus:ring-offset-0">
        <span class="text-xs text-slate-800 dark:text-slate-300">Overwrite if destination already exists</span>
      </label>

      <div class="flex justify-end gap-2">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
        <button id="confirmWebTakeout" type="button" class="rounded-md bg-slate-900 dark:bg-cyan-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Takeout</button>
      </div>
    `);

    document.getElementById("confirmWebTakeout")?.addEventListener("click", () => {
      const destName = document.getElementById("takeoutDestName")?.value.trim() || undefined;
      const overwrite = Boolean(document.getElementById("takeoutOverwrite")?.checked);
      executeTakeout(web, destName, overwrite);
    });
  }

  async function executeTakeout(web, destName, overwrite) {
    const payload = {
      dest_name: destName || web.name,
      overwrite: Boolean(overwrite)
    };

    try {
      const res = await window.EdmsAPI.takeoutWebview(web.name, payload);
      if (!res.ok) {
        if (res.status === 409 && res.data && Array.isArray(res.data.options)) {
          showTakeoutConflictModal(web, payload.dest_name, res.data.options, res.data.error || `Destination "${payload.dest_name}" already exists.`);
          return;
        }
        const errMsg = res.data?.error || res.data?.message || `Takeout request failed (HTTP ${res.status})`;
        showAlert(errMsg);
        return;
      }

      const jobId = res.data?.job_id || res.data?.id;
      if (jobId) {
        showAlert(`Takeout accepted (Job ID: ${jobId}). Processing in background...`);
        if (window.EdmsAPI && typeof window.EdmsAPI.listenForJob === "function") {
          window.EdmsAPI.listenForJob(jobId, () => {
            showAlert(`Takeout complete for Web View "${web.name}"!`);
          }, (err) => {
            showAlert(`Takeout job ${jobId} failed: ${err.message || "Error"}`);
          });
        }
      } else {
        showAlert(`Takeout initiated successfully for Web View "${web.name}".`);
      }
      closeModal();
    } catch (err) {
      showAlert(`Error requesting takeout: ${err.message}`);
    }
  }

  function showTakeoutConflictModal(web, destName, options, errorMsg) {
    openModal(`
      <div class="rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 mb-3">
        <div class="text-xs font-semibold text-amber-700 dark:text-amber-300">Conflict: Destination Already Exists (409)</div>
        <div class="text-[11px] text-amber-700/80 dark:text-amber-200/80 mt-1">${escapeHtml(errorMsg)}</div>
      </div>
      <p class="mb-3 text-[11px] text-slate-500 dark:text-slate-400">Choose how to resolve this collision:</p>
      <div class="mb-4 space-y-3">
        ${options.includes("overwrite") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Overwrite</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Overwrite existing files in "${escapeHtml(destName)}".</p>
            <button id="takeoutConflictOverwriteBtn" type="button" class="rounded bg-rose-600 px-2.5 py-1 text-xs text-white hover:bg-rose-500">Overwrite Existing</button>
          </div>
        ` : ""}
        ${options.includes("rename") ? `
          <div class="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 p-3">
            <div class="text-xs font-medium text-slate-800 dark:text-white mb-1">Rename</div>
            <p class="text-[11px] text-slate-500 dark:text-slate-400 mb-2">Save takeout to a different folder name.</p>
            <div class="flex gap-2">
              <input id="takeoutConflictRenameInput" value="${escapeAttr(destName + "_1")}" class="h-8 flex-1 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 px-2 text-xs text-slate-800 dark:text-slate-200 outline-none focus:border-cyan-500">
              <button id="takeoutConflictRenameBtn" type="button" class="rounded bg-slate-900 dark:bg-cyan-600 px-2.5 py-1 text-xs text-white hover:bg-slate-800 dark:hover:bg-cyan-500">Rename & Takeout</button>
            </div>
          </div>
        ` : ""}
      </div>
      <div class="flex justify-end">
        <button type="button" data-modal-close class="rounded-md border border-slate-300 dark:border-slate-700 px-3 py-1.5 text-xs text-slate-500 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800">Cancel</button>
      </div>
    `);

    document.getElementById("takeoutConflictOverwriteBtn")?.addEventListener("click", () => {
      executeTakeout(web, destName, true);
    });

    document.getElementById("takeoutConflictRenameBtn")?.addEventListener("click", () => {
      const renVal = document.getElementById("takeoutConflictRenameInput")?.value.trim();
      if (!renVal) {
        showAlert("Destination folder name is required.");
        return;
      }
      executeTakeout(web, renVal, false);
    });
  }

  // ============================================================
  // MODAL
  // ============================================================

  function wireModal() {
    const backdrop = document.getElementById("modalBackdrop");

    if (!backdrop) return;

    backdrop.addEventListener("click", (event) => {
      if (event.target === backdrop) closeModal();

      if (event.target.closest("[data-modal-close]")) closeModal();
    });

    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !backdrop.classList.contains("hidden")) {
        closeModal();
      }
    });
  }

  function openModal(html) {
    const backdrop = document.getElementById("modalBackdrop");
    const content = document.getElementById("modalContent");

    if (!backdrop || !content) return;

    content.innerHTML = html;

    backdrop.classList.remove("hidden");
    backdrop.classList.add("flex");
  }

  function closeModal() {
    const backdrop = document.getElementById("modalBackdrop");
    const content = document.getElementById("modalContent");

    backdrop?.classList.add("hidden");
    backdrop?.classList.remove("flex");

    if (content) content.innerHTML = "";
  }

  // ============================================================
  // SIZE
  // ============================================================

  function cycleSizeUnit(id) {
    const units = ["B", "KB", "MB", "GB"];

    const current = sizeUnits[id] || "MB";

    const index = units.indexOf(current);

    sizeUnits[id] = units[(index + 1) % units.length];

    renderTable();
  }

  function formatSize(bytes, unit) {
    const value = Number(bytes) || 0;

    switch (unit) {
      case "B":
        return `${value.toFixed(0)} B`;
      case "KB":
        return `${(value / 1024).toFixed(1)} KB`;
      case "GB":
        return `${(value / 1024 / 1024 / 1024).toFixed(2)} GB`;
      case "MB":
      default:
        return `${(value / 1024 / 1024).toFixed(1)} MB`;
    }
  }

  // ============================================================
  // HELPERS
  // ============================================================

  function getWeb(id) {
    if (id === null || id === undefined) return null;
    return state.webs.find((web) => String(web.id) === String(id) || web.name === String(id));
  }

  function escapeHtml(value) {
    const div = document.createElement("div");
    div.textContent = String(value ?? "");
    return div.innerHTML;
  }

  function escapeAttr(value) {
    return escapeHtml(value).replace(/"/g, "&quot;");
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
  // PREVIEW
  // ============================================================

  function previewWebView(id, options = {}) {
    if (typeof window.previewWebViewPage === "function") {
      const localWeb = getWeb(id);
      const defaultLoadWebView = async (webId) => {
        const targetWeb = getWeb(webId) || localWeb || {};
        const inMemoryContent =
          targetWeb.front_page ||
          targetWeb.content ||
          targetWeb.frontpageContent ||
          targetWeb.frontPageContent;
        if (inMemoryContent && typeof inMemoryContent === "object") {
          return { ...targetWeb, front_page: inMemoryContent, content: inMemoryContent };
        }
        try {
          const webName = targetWeb.name || webId;
          const endpoint = window.WEBVIEW_FRONT_PAGE_API_URL
            ? window.WEBVIEW_FRONT_PAGE_API_URL.replace(
              ":name",
              encodeURIComponent(String(webName))
            ).replace(
              ":id",
              encodeURIComponent(String(webName))
            )
            : `/api/webview/${encodeURIComponent(String(webName))}/front-page`;
          const res = await fetch(endpoint, {
            headers: { Accept: "application/json" },
          });
          if (res.ok) {
            const payload = await res.json();
            const data = payload && payload.data ? payload.data : payload;
            const doc =
              data.front_page || data.content || data.frontpageContent || data.frontPageContent;
            if (doc && typeof doc === "object") {
              return {
                ...targetWeb,
                ...data,
                front_page: doc,
                content: doc,
                name: targetWeb.name || data.name,
                annotation: targetWeb.annotation || data.annotation,
              };
            }
          }
        } catch (e) {
          console.warn("Could not fetch WebView frontpage content:", e);
        }

        if (targetWeb && targetWeb.name) {
          return {
            ...targetWeb,
            isMock: true,
            content: window.WEBVIEW_PREVIEW_MOCK_DATA?.content,
          };
        }
        throw new Error("No WebView content returned");
      };

      return window.previewWebViewPage(id, {
        loadWebView: options.loadWebView || defaultLoadWebView,
        ...options,
      });
    }

    const web = getWeb(id);
    return web;
  }

  // ============================================================
  // PUBLIC API
  // ============================================================

  window.WebView = {
    getState: () => state,

    getWeb,

    duplicateWeb,

    openEditWebModal,

    openAnnotationModal,

    openDataTagsModal,

    openDeleteModal,

    deleteWebs,

    openCreateModal,

    combineWebs,

    convertToCollection,

    takeoutWeb,

    previewWebView,
  };

  // Expose globally for edit-webview-frontpage.js, rmbwv.js & preview
  window.getWeb = getWeb;
  window.openModal = openModal;
  window.closeModal = closeModal;
  window.showAlert = showAlert;
  window.previewWebView = previewWebView;
  window.openPreviewWebModal = previewWebView;
  window.duplicateWeb = duplicateWeb;
  window.deleteWebs = deleteWebs;
  window.openDeleteModal = openDeleteModal;
  window.combineWebs = combineWebs;
  window.convertToCollection = convertToCollection;
  window.takeoutWeb = takeoutWeb;
  window.state = state;
})();