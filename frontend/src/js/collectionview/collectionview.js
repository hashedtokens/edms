(() => {
  "use strict";

  // ============================================================
  // CONFIG
  // ============================================================

  const PAGE_SIZE = 50;

  const API_BASE = "http://localhost:3000";

  let nextId = 9000;

  // ============================================================
  // STATE
  // ============================================================

  const state = {
    folders: [],

    filtered: [],

    page: 1,

    search: "",

    purpose: "all",

    tags: new Set(),

    segments: new Set(),

    selected: new Set(),

    activeFolderId: null,

    prevActiveFolderId: null,

    sidebarCollapsed: false,
  };

  // ============================================================
  // INIT
  // ============================================================

  document.addEventListener("DOMContentLoaded", init);

  async function init() {
    wireToolbar();

    wireTableEvents();

    wireSidebar();

    wireSelection();

    wireModal();

    await loadFolders();

    renderSidebar();

    applyFilters();

    if (typeof TagManager !== "undefined") {
      TagManager.init({
        view: "collectionview",
        getItems: () => state.selected.size > 0
          ? state.folders.filter(f => state.selected.has(Number(f.id)))
          : state.folders,
        getTags: folder => Array.isArray(folder.tags) ? folder.tags : [],
        setTags: (folder, tags) => {
          folder.tags = tags;
        },
        addTag: async (folder, tag) => {
          const api = window.EdmsAPI;
          if (!api || typeof api.addMembershipTag !== "function") return;
          await api.addMembershipTag(folder.name, tag);
          if (!Array.isArray(folder.tags)) folder.tags = [];
          if (!folder.tags.includes(tag)) folder.tags.push(tag);
        },
        removeTag: async (folder, tag) => {
          const api = window.EdmsAPI;
          if (!api || typeof api.removeMembershipTag !== "function") return;
          await api.removeMembershipTag(folder.name, tag);
          folder.tags = Array.isArray(folder.tags) ? folder.tags.filter(item => item !== tag) : [];
        },
        onChange: () => {
          renderSidebar();
          applyFilters();
        }
      });
    }
  }

  // ============================================================
  // DATA
  // ============================================================

  function getApi() {
    return window.EdmsAPI || null;
  }

  function getApiData(response) {
    if (!response) {
      return null;
    }

    return response.data ?? response;
  }

  function getCollectionItems(response) {
    const data = getApiData(response);

    if (!data) {
      return [];
    }

    if (Array.isArray(data)) {
      return data;
    }

    if (Array.isArray(data.items)) {
      return data.items;
    }

    if (Array.isArray(data.collections)) {
      return data.collections;
    }

    return [];
  }

  function collectionName(item) {
    if (typeof item === "string") {
      return item;
    }

    return String(
      item?.name ?? item?.collection_name ?? item?.collection ?? "",
    );
  }

  function makeLocalId(name) {
    /*
     * The collections API identifies collections by name.
     * The existing UI uses numeric IDs internally, so keep a
     * local UI-only ID without inventing backend data.
     */

    let hash = 0;

    const text = String(name);

    for (let index = 0; index < text.length; index++) {
      hash = ((hash << 5) - hash + text.charCodeAt(index)) | 0;
    }

    return Math.abs(hash) || nextId++;
  }

  async function loadFolders() {
    const api = getApi();

    if (!api || typeof api.listCollections !== "function") {
      console.error("EdmsAPI.listCollections() is not available.");

      state.folders = [];

      return;
    }

    try {
      const response = await api.listCollections();

      if (response && response.ok === false) {
        throw new Error(
          `Failed to load collections (${response.status ?? "unknown"})`,
        );
      }

      const items = getCollectionItems(response);

      const previousActive = state.activeFolderId;

      const previousActiveFolder = getFolder(previousActive);

      const previousActiveName = previousActiveFolder?.name;

      const folders = items
        .map((item) => {
          const name = collectionName(item).trim();

          if (!name) {
            return null;
          }

          return {
            id: makeLocalId(name),

            name,

            /*
             * These fields are deliberately not
             * populated because the backend collection
             * API does not provide them (except annotation).
             */
            purpose: undefined,

            datatype: undefined,

            annotation: item.annotation ?? undefined,

            source: undefined,

            tags: [],

            endpoints: [],
          };
        })
        .filter(Boolean);

      state.folders = folders;

      nextId =
        Math.max(
          nextId,
          ...folders.map((folder) => Number(folder.id) || 0),
          0,
        ) + 1;

      /*
       * Fetch the actual endpoint membership and membership
       * tags for every collection.
       */
      await Promise.all(
        state.folders.map((folder) => loadCollectionData(folder)),
      );
      /*
       * Enrich collection memberships with the central
       * endpoint metadata supplied by the backend.
       */
      await enrichEndpointMetadata();

      const restored = previousActiveName
        ? state.folders.find((folder) => folder.name === previousActiveName)
        : null;

      if (restored) {
        state.activeFolderId = restored.id;
      } else if (state.folders.length) {
        /*
         * Preserve the existing prototype default:
         * second collection, otherwise first.
         */
        state.activeFolderId =
          state.folders[1]?.id ?? state.folders[0]?.id ?? null;
      } else {
        state.activeFolderId = null;
      }

      /*
       * Remove selections that no longer exist.
       */
      const validIds = new Set(
        state.folders.map((folder) => Number(folder.id)),
      );

      state.selected = new Set(
        [...state.selected].filter((id) => validIds.has(Number(id))),
      );
    } catch (error) {
      console.error("Failed to load collections from backend:", error);

      state.folders = [];

      state.activeFolderId = null;
    }
  }

  async function loadCollectionData(folder) {
    const api = getApi();

    if (!api) {
      return;
    }

    /*
     * --------------------------------------------------------
     * Collection endpoint membership
     * --------------------------------------------------------
     */

    try {
      if (typeof api.listCollectionEndpoints === "function") {
        const response = await api.listCollectionEndpoints(folder.name);

        const data = getApiData(response);

        const items = Array.isArray(data)
          ? data
          : Array.isArray(data?.endpoints)
            ? data.endpoints
            : Array.isArray(data?.items)
              ? data.items
              : [];

        folder.endpoints = items
          .map((item) => {
            if (typeof item === "string") {
              return {
                id: item,

                endpoint_id: item,

                added_at: undefined,

                endpoint: "",

                endpoint_str: "",

                annotation: undefined,

                method: "",
              };
            }

            const endpointId = String(item?.endpoint_id ?? item?.id ?? "");

            if (!endpointId) {
              return null;
            }

            return {
              id: endpointId,

              endpoint_id: endpointId,

              added_at: item?.added_at,

              endpoint: "",

              endpoint_str: "",

              annotation: undefined,

              method: "",
            };
          })
          .filter(Boolean);
      }
    } catch (error) {
      console.error(
        `Failed to load endpoints for collection "${folder.name}":`,
        error,
      );

      folder.endpoints = [];
    }

    /*
     * --------------------------------------------------------
     * Collection membership tags
     * --------------------------------------------------------
     */

    try {
      if (typeof api.listMembershipTags === "function") {
        const response = await api.listMembershipTags(folder.name);

        const data = getApiData(response);

        const tags = Array.isArray(data)
          ? data
          : Array.isArray(data?.tags)
            ? data.tags
            : Array.isArray(data?.items)
              ? data.items
              : [];

        folder.tags = tags
          .map((tag) =>
            typeof tag === "string" ? tag : String(tag?.name ?? tag?.tag ?? ""),
          )
          .map((tag) => tag.trim())
          .filter(Boolean);
      }
    } catch (error) {
      console.error(
        `Failed to load tags for collection "${folder.name}":`,
        error,
      );

      folder.tags = [];
    }
  }

  // ============================================================
  // ENDPOINT METADATA
  // ============================================================

  async function loadEndpointMetadata() {
    const api = getApi();

    if (!api || typeof api.connectEndpointLoader !== "function") {
      return new Map();
    }

    return new Promise((resolve) => {
      const metadata = new Map();

      let socket = null;

      let settled = false;

      const finish = () => {
        if (settled) {
          return;
        }

        settled = true;

        try {
          socket?.close();
        } catch (_) {
          // Ignore close errors.
        }

        resolve(metadata);
      };

      const timeout = setTimeout(finish, 5000);

      try {
        socket = api.connectEndpointLoader();

        if (!socket) {
          clearTimeout(timeout);

          finish();

          return;
        }

        socket.onmessage = (event) => {
          try {
            const message =
              typeof event.data === "string"
                ? JSON.parse(event.data)
                : event.data;

            collectEndpointMetadata(message, metadata);

            /*
             * The loader sends the initial
             * snapshot when the connection opens.
             * Once we have useful endpoint data,
             * the current collection memberships
             * can be enriched and the socket closed.
             */
            if (metadata.size > 0) {
              clearTimeout(timeout);

              finish();
            }
          } catch (error) {
            console.error("Failed to parse endpoint loader message:", error);
          }
        };

        socket.onerror = () => {
          clearTimeout(timeout);

          finish();
        };

        socket.onclose = () => {
          clearTimeout(timeout);

          finish();
        };
      } catch (error) {
        console.error("Failed to connect endpoint loader:", error);

        clearTimeout(timeout);

        finish();
      }
    });
  }

  function collectEndpointMetadata(value, metadata) {
    if (!value) {
      return;
    }

    if (Array.isArray(value)) {
      value.forEach((item) => collectEndpointMetadata(item, metadata));

      return;
    }

    if (typeof value !== "object") {
      return;
    }

    const endpointId = value.endpoint_id ?? value.id;

    if (
      endpointId &&
      (value.endpoint_str !== undefined ||
        value.endpoint !== undefined ||
        value.method !== undefined ||
        value.annotation !== undefined)
    ) {
      metadata.set(String(endpointId), {
        endpoint_id: String(endpointId),

        endpoint_str: value.endpoint_str ?? value.endpoint ?? "",

        annotation: value.annotation,

        method: value.method,
      });
    }

    Object.values(value).forEach((child) =>
      collectEndpointMetadata(child, metadata),
    );
  }

  async function enrichEndpointMetadata() {
    const api = getApi();

    if (!api || typeof api.connectEndpointLoader !== "function") {
      return;
    }

    const metadata = await loadEndpointMetadata();

    if (!metadata.size) {
      return;
    }

    state.folders.forEach((folder) => {
      folder.endpoints = (
        Array.isArray(folder.endpoints) ? folder.endpoints : []
      ).map((endpoint) => {
        const info = metadata.get(
          String(endpoint.endpoint_id ?? endpoint.id ?? ""),
        );

        if (!info) {
          return endpoint;
        }

        return {
          ...endpoint,

          endpoint: info.endpoint_str ?? endpoint.endpoint ?? "",

          endpoint_str: info.endpoint_str ?? endpoint.endpoint_str ?? "",

          annotation: info.annotation ?? endpoint.annotation,

          method: info.method ?? endpoint.method ?? "",
        };
      });
    });
  }

  // ============================================================
  // FILTERING
  // ============================================================

  function applyFilters() {
    const term = state.search.trim().toLowerCase();

    state.filtered = state.folders.filter((folder) => {
      const endpoints = Array.isArray(folder.endpoints) ? folder.endpoints : [];

      const folderTags = Array.isArray(folder.tags) ? folder.tags : [];

      const matchesSearch =
        !term ||
        String(folder.name || "")
          .toLowerCase()
          .includes(term) ||
        String(folder.annotation || "")
          .toLowerCase()
          .includes(term) ||
        folderTags.some((tag) => String(tag).toLowerCase().includes(term)) ||
        endpoints.some((endpoint) => {
          const endpointPath = String(
            endpoint.endpoint || endpoint.endpoint_str || "",
          ).toLowerCase();

          const method = String(endpoint.method || "").toLowerCase();

          const endpointTags = Array.isArray(endpoint.tags)
            ? endpoint.tags
            : [];

          return (
            endpointPath.includes(term) ||
            method.includes(term) ||
            endpointTags.some((tag) => String(tag).toLowerCase().includes(term))
          );
        });

      /*
       * Purpose is not supplied by the backend
       * collection APIs, so it must not filter out
       * otherwise valid backend collections.
       */
      const matchesPurpose =
        state.purpose === "all" ||
        !folder.purpose ||
        folder.purpose === state.purpose;

      const matchesTags =
        state.tags.size === 0 ||
        [...state.tags].every(
          (tag) =>
            folderTags.includes(tag) ||
            endpoints.some(
              (endpoint) =>
                Array.isArray(endpoint.tags) && endpoint.tags.includes(tag),
            ),
        );

      const folderSegments = getFolderSegments(folder);

      const matchesSegments =
        state.segments.size === 0 ||
        [...state.segments].every((segment) =>
          folderSegments.includes(segment),
        );

      return matchesSearch && matchesPurpose && matchesTags && matchesSegments;
    });

    const totalPages = getTotalPages();

    if (state.page > totalPages) {
      state.page = totalPages;
    }

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
  // ============================================================

  function getEndpointSegments(endpointPath) {
    return String(endpointPath || "")
      .split("/")
      .filter(Boolean);
  }

  function getFolderSegments(folder) {
    const segments = new Set();

    const endpoints = Array.isArray(folder.endpoints) ? folder.endpoints : [];

    endpoints.forEach((endpoint) => {
      getEndpointSegments(endpoint.endpoint || endpoint.endpoint_str).forEach(
        (segment) => segments.add(segment),
      );
    });

    return [...segments];
  }

  // ============================================================
  // RENDER
  // ============================================================

  function render() {
    renderTable();

    renderPagination();

    renderActiveFolderIndicator();

    renderSelectionUI();

    updateStats();

    syncSidebar();
  }

  // ============================================================
  // TABLE
  // ============================================================

  function renderTable() {
    const tbody = document.getElementById("folderTableBody");

    if (!tbody) return;

    tbody.innerHTML = "";

    const items = currentPageItems();

    if (items.length === 0) {
      tbody.innerHTML = `

            <tr>

                <td
                    colspan="9"
                    class="px-4 py-16 text-center"
                >

                    <p
                        class="text-sm font-medium
                               text-slate-400"
                    >
                        No collections found
                    </p>

                    <p
                        class="mt-1 text-xs
                               text-slate-600"
                    >
                        Try changing the current
                        search or filters.
                    </p>

                </td>

            </tr>

        `;

      updateSelectAllState();

      return;
    }

    items.forEach((folder) => {
      tbody.appendChild(createFolderRow(folder));
    });

    updateSelectAllState();
  }

  function createFolderRow(folder) {
    const row = document.createElement("tr");

    const id = Number(folder.id);

    const isSelected = state.selected.has(id);

    const endpoints = Array.isArray(folder.endpoints) ? folder.endpoints : [];

    const counts = getCrudCounts(endpoints);

    row.dataset.id = String(id);

    row.className = [
      "folder-row",
      "group",
      "border-b",
      "border-slate-200", "dark:border-slate-800",
      "transition-colors",
      "duration-100",
      "cursor-pointer",
      "hover:bg-slate-100", "dark:hover:bg-slate-800/60",

      isSelected ? "bg-cyan-500/[0.045]" : "",
    ]
      .filter(Boolean)
      .join(" ");

    row.innerHTML = `

        <td class="px-2 py-2 align-middle">

            <input
                type="checkbox"
                class="folder-select
                       h-3.5 w-3.5
                       accent-cyan-400"
                ${isSelected ? "checked" : ""}
            >

        </td>


        <td
            class="px-2 py-2
                   font-medium text-slate-800 dark:text-slate-200"
        >

            <div
                class="flex min-w-0
                       items-center gap-2"
            >

                <span
                    class="truncate flex-1"
                    title="${escapeAttr(folder.name)}"
                >
                    ${escapeHtml(folder.name)}
                </span>



            </div>

        </td>


        <td class="px-2 py-2">

            <div
                class="flex flex-wrap
                       gap-1"
            >
                ${renderFolderTags(folder)}
            </div>

        </td>


        <td class="px-1 py-2 text-center">
            ${crudCountBadge(counts.GET, "text-emerald-400")}
        </td>


        <td class="px-1 py-2 text-center">
            ${crudCountBadge(counts.POST, "text-sky-400")}
        </td>


        <td class="px-1 py-2 text-center">
            ${crudCountBadge(counts.PUT, "text-amber-400")}
        </td>


        <td class="px-1 py-2 text-center">
            ${crudCountBadge(counts.PATCH, "text-violet-400")}
        </td>


        <td class="px-1 py-2 text-center">
            ${crudCountBadge(counts.DELETE, "text-rose-400")}
        </td>


        <td
            class="px-2 py-2
                   text-slate-400"
        >
            —
        </td>


        <td class="px-2 py-2">

            <span
                class="line-clamp-2
                       text-[11px]
                       text-slate-500"
                title="${escapeAttr(folder.annotation || "")}"
            >
                ${escapeHtml(folder.annotation || "—")}
            </span>

        </td>

    `;

    const checkbox = row.querySelector(".folder-select");

    checkbox.addEventListener("click", (event) => {
      event.stopPropagation();
    });

    checkbox.addEventListener("change", () => {
      if (checkbox.checked) {
        state.selected.add(id);
      } else {
        state.selected.delete(id);
      }

      renderSelectionUI();

      updateSelectAllState();

      renderTable();
    });



    row.querySelectorAll(".folder-tag").forEach((button) => {
      button.addEventListener("click", (event) => {
        event.stopPropagation();

        const tag = button.dataset.tag;

        state.tags.clear();

        state.tags.add(tag);

        state.page = 1;

        applyFilters();
      });
    });

    row.addEventListener("click", (event) => {
      if (event.target.closest("button,input")) {
        return;
      }

      openDataView(id);
    });

    row.addEventListener("dblclick", (event) => {
      if (event.target.closest("button,input")) {
        return;
      }

      openDataView(id);
    });

    return row;
  }

  function renderFolderTags(folder) {
    const tags = Array.isArray(folder.tags) ? folder.tags : [];

    if (!tags.length) {
      return `
            <span
                class="text-[10px]
                       text-slate-600"
            >
                —
            </span>
        `;
    }

    return tags
      .map(
        (tag) => `

                <button
                    type="button"
                   class="folder-tag
       inline-flex
       rounded
       border border-transparent
       bg-slate-200 dark:bg-slate-800
       px-1.5 py-0.5
       text-[10px]
       text-sky-600 dark:text-sky-300
       transition
       hover:border-blue-500 dark:hover:border-blue-400
       hover:bg-sky-100 dark:hover:bg-sky-500/20
       hover:text-blue-700 dark:hover:text-sky-200
       hover:ring-1 hover:ring-blue-400/50"
                    data-tag="${escapeAttr(tag)}"
                >
                    ${escapeHtml(tag)}
                </button>

            `,
      )
      .join("");
  }

  function crudCountBadge(count, color) {
    return `

        <span
            class="inline-flex min-w-6
                   items-center
                   justify-center
                   rounded-md bg-slate-200 dark:bg-slate-800
                   px-1.5 py-0.5
                   text-[10px]
                   ${color}"
        >
            ${count}
        </span>

    `;
  }

  function getCrudCounts(endpoints) {
    const counts = {
      GET: 0,

      POST: 0,

      PUT: 0,

      PATCH: 0,

      DELETE: 0,
    };

    endpoints.forEach((endpoint) => {
      const method = String(endpoint.method || "").toUpperCase();

      if (Object.prototype.hasOwnProperty.call(counts, method)) {
        counts[method]++;
      }
    });

    return counts;
  }

  // ============================================================
  // PAGINATION
  // ============================================================

  function renderPagination() {
    const total = state.filtered.length;

    const start = total ? (state.page - 1) * PAGE_SIZE + 1 : 0;

    const end = Math.min(state.page * PAGE_SIZE, total);

    const rangeStart = document.getElementById("paginationRangeStart");

    const rangeEnd = document.getElementById("paginationRangeEnd");

    const paginationTotal = document.getElementById("paginationTotal");

    if (rangeStart) {
      rangeStart.textContent = start;
    }

    if (rangeEnd) {
      rangeEnd.textContent = end;
    }

    if (paginationTotal) {
      paginationTotal.textContent = total;
    }

    const totalPages = getTotalPages();

    const numbers = document.getElementById("paginationNumbers");

    if (!numbers) {
      return;
    }

    numbers.innerHTML = "";

    getPageRange(state.page, totalPages).forEach((page) => {
      if (page === "...") {
        const span = document.createElement("span");

        span.className = "px-1 text-slate-600";

        span.textContent = "…";

        numbers.appendChild(span);

        return;
      }

      const button = document.createElement("button");

      button.type = "button";

      button.textContent = page;

      button.className =
        page === state.page
          ? `
                         min-w-7 rounded-md
      bg-slate-900 dark:bg-cyan-500 px-2 py-1
      text-[11px] text-white
                      `
          : `
                        min-w-7 rounded-md
                        border border-slate-700
                        px-2 py-1
                        text-[11px]
                        text-slate-400
                        transition
                        hover:bg-slate-800
                        hover:text-white
                      `;

      button.addEventListener("click", () => {
        state.page = page;

        render();
      });

      numbers.appendChild(button);
    });

    const prev = document.getElementById("paginationPrev");

    const next = document.getElementById("paginationNext");

    if (prev) {
      prev.disabled = state.page <= 1;
    }

    if (next) {
      next.disabled = state.page >= totalPages;
    }
  }

  function getPageRange(current, total) {
    if (total <= 7) {
      return Array.from(
        {
          length: total,
        },
        (_, index) => index + 1,
      );
    }

    const pages = [1];

    if (current > 4) {
      pages.push("...");
    }

    const start = Math.max(2, current - 1);

    const end = Math.min(total - 1, current + 1);

    for (let page = start; page <= end; page++) {
      pages.push(page);
    }

    if (current < total - 3) {
      pages.push("...");
    }

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
      sidebar?.classList.remove("w-64");

      sidebar?.classList.add("w-10");

      content?.classList.add("hidden");

      title?.classList.add("hidden");

      if (icon) {
        icon.innerHTML = `
                <path d="m9 18 6-6-6-6"/>
            `;
      }
    } else {
      sidebar?.classList.remove("w-10");

      sidebar?.classList.add("w-64");

      content?.classList.remove("hidden");

      title?.classList.remove("hidden");

      if (icon) {
        icon.innerHTML = `
                <path d="m15 18-6-6 6-6"/>
            `;
      }
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

    state.folders.forEach((folder) => {
      const tags = Array.isArray(folder.tags) ? folder.tags : [];

      tags.forEach((tag) => {
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

                    <input
                        type="checkbox"
                        class="collection-tag-checkbox
                               h-3.5 w-3.5
                               accent-cyan-400"
                        data-tag="${escapeAttr(tag)}"
                    >

                    <button
                        type="button"
                        class="collection-tag-filter
                               min-w-0 flex-1
                               truncate rounded
                               px-1.5 py-1
                               text-left text-[11px]
                               text-slate-400
                               transition
                               hover:bg-sky-500/10
                               hover:text-sky-300"
                        data-tag="${escapeAttr(tag)}"
                    >
                        ${escapeHtml(tag)}

                        <span
                            class="text-slate-600"
                        >
                            (${count})
                        </span>
                    </button>

                `;

        const checkbox = wrapper.querySelector(".collection-tag-checkbox");

        const button = wrapper.querySelector(".collection-tag-filter");

        checkbox.addEventListener("change", () => {
          setTagFilter(tag, checkbox.checked);
        });

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

    state.folders.forEach((folder) => {
      getFolderSegments(folder).forEach((segment) => {
        counts[segment] = (counts[segment] || 0) + 1;
      });
    });

    container.innerHTML = "";

    Object.entries(counts)
      .sort((a, b) => a[0].localeCompare(b[0]))
      .forEach(([segment, count]) => {
        const wrapper = document.createElement("label");

        wrapper.className = "group flex cursor-pointer items-center gap-2";

        wrapper.innerHTML = `

                    <input
                        type="checkbox"
                        class="collection-segment-checkbox
                               h-3.5 w-3.5
                               accent-cyan-400"
                        data-segment="${escapeAttr(segment)}"
                    >

                    <button
                        type="button"
                        class="collection-segment-filter
                               min-w-0 flex-1
                               truncate rounded
                               px-1.5 py-1
                               text-left text-[11px]
                               text-slate-400
                               transition
                               hover:bg-cyan-500/10
                               hover:text-cyan-300"
                        data-segment="${escapeAttr(segment)}"
                    >
                        ${escapeHtml(segment)}

                        <span
                            class="text-slate-600"
                        >
                            (${count})
                        </span>
                    </button>

                `;

        const checkbox = wrapper.querySelector(".collection-segment-checkbox");

        const button = wrapper.querySelector(".collection-segment-filter");

        checkbox.addEventListener("change", () => {
          setSegmentFilter(segment, checkbox.checked);
        });

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
    if (enabled) {
      state.tags.add(tag);
    } else {
      state.tags.delete(tag);
    }

    state.page = 1;

    applyFilters();
  }

  function setSegmentFilter(segment, enabled) {
    if (enabled) {
      state.segments.add(segment);
    } else {
      state.segments.delete(segment);
    }

    state.page = 1;

    applyFilters();
  }

  function syncSidebar() {
    document
      .querySelectorAll(".collection-tag-checkbox")
      .forEach((checkbox) => {
        checkbox.checked = state.tags.has(checkbox.dataset.tag);
      });

    document
      .querySelectorAll(".collection-segment-checkbox")
      .forEach((checkbox) => {
        checkbox.checked = state.segments.has(checkbox.dataset.segment);
      });
  }

  // ============================================================
  // STATS
  // ============================================================

  function updateStats() {
    const folderStat = document.getElementById("folderStat");

    const endpointStat = document.getElementById("endpointStat");

    const tagStat = document.getElementById("tagStat");

    const segmentStat = document.getElementById("segmentStat");

    const tags = new Set();

    const segments = new Set();

    let endpointCount = 0;

    state.folders.forEach((folder) => {
      const folderTags = Array.isArray(folder.tags) ? folder.tags : [];

      folderTags.forEach((tag) => tags.add(tag));

      const endpoints = Array.isArray(folder.endpoints) ? folder.endpoints : [];

      endpointCount += endpoints.length;

      endpoints.forEach((endpoint) => {
        getEndpointSegments(endpoint.endpoint || endpoint.endpoint_str).forEach(
          (segment) => segments.add(segment),
        );
      });
    });

    if (folderStat) {
      folderStat.textContent = state.folders.length;
    }

    if (endpointStat) {
      endpointStat.textContent = endpointCount;
    }

    if (tagStat) {
      tagStat.textContent = tags.size;
    }

    if (segmentStat) {
      segmentStat.textContent = segments.size;
    }
  }

  // ============================================================
  // ACTIVE FOLDER
  // ============================================================

  function renderActiveFolderIndicator() {
    const active = getFolder(state.activeFolderId);

    const previous = getFolder(state.prevActiveFolderId);

    const activeLabel = document.getElementById("activeFolderLabel");

    const previousLabel = document.getElementById("prevActiveFolderLabel");

    if (activeLabel) {
      activeLabel.textContent = active ? active.name : "None";
    }

    if (previousLabel) {
      previousLabel.textContent = previous ? `(prev: ${previous.name})` : "";
    }
  }

  // ============================================================
  // ACTIVE FOLDER
  // ============================================================

  function renderActiveFolderIndicator() {
    const active = getFolder(state.activeFolderId);

    const previous = getFolder(state.prevActiveFolderId);

    const activeLabel = document.getElementById("activeFolderLabel");

    const previousLabel = document.getElementById("prevActiveFolderLabel");

    if (activeLabel) {
      activeLabel.textContent = active ? active.name : "None";
    }

    if (previousLabel) {
      previousLabel.textContent = previous ? `(prev: ${previous.name})` : "";
    }
  }

  function setActiveFolder(id) {
    const folder = getFolder(id);

    if (!folder) return;

    if (state.activeFolderId !== folder.id) {
      state.prevActiveFolderId = state.activeFolderId;

      state.activeFolderId = folder.id;
    }

    render();
  }

  /*
   * Load the selected collection into the backend
   * Active Collection workspace.
   *
   * This is different from merely selecting a row.
   * The backend must explicitly load the collection before
   * bookmark/save operations can persist into it.
   */
  async function loadCollectionIntoActive(id) {
    const folder = getFolder(id);

    if (!folder) {
      showAlert("Collection not found.");

      return false;
    }

    const api = getApi();

    if (!api || typeof api.loadCollection !== "function") {
      showAlert("Collection loading API is not available.");

      return false;
    }

    try {
      await api.loadCollection(folder.name);

      /*
       * Loading succeeded, so this collection is now
       * the collection represented by the Active Collection
       * workspace.
       */
      if (state.activeFolderId !== folder.id) {
        state.prevActiveFolderId = state.activeFolderId;
      }

      state.activeFolderId = folder.id;

      render();

      return true;
    } catch (error) {
      console.error(
        `Failed to load collection "${folder.name}" into Active Collection:`,
        error,
      );

      showAlert(
        error?.message || `Failed to load collection "${folder.name}".`,
      );

      return false;
    }
  }

  // ============================================================
  // SELECTION
  // ============================================================

  function wireSelection() {
    document
      .getElementById("clearFolderSelection")
      ?.addEventListener("click", clearSelection);

    document
      .getElementById("mergeCollectionsBtn")
      ?.addEventListener("click", () => {
        openMergeModal([...state.selected]);
      });

    document
      .getElementById("selectAllFolders")
      ?.addEventListener("change", (event) => {
        currentPageItems().forEach((folder) => {
          if (event.target.checked) {
            state.selected.add(Number(folder.id));
          } else {
            state.selected.delete(Number(folder.id));
          }
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

    const count = document.getElementById("selectedFolderCount");

    const list = document.getElementById("selectedFolderList");

    const selected = getSelectedFolders();

    if (selected.length) {
      bar?.classList.remove("hidden");

      bar?.classList.add("flex");
    } else {
      bar?.classList.add("hidden");

      bar?.classList.remove("flex");
    }

    if (count) {
      count.textContent = `${selected.length} selected`;
    }

    const mergeBtn = document.getElementById("mergeCollectionsBtn");
    if (mergeBtn) {
      if (selected.length >= 2) {
        mergeBtn.disabled = false;
        mergeBtn.classList.remove("opacity-40", "cursor-not-allowed");
        mergeBtn.title = `Merge ${selected.length} selected collections`;
      } else {
        mergeBtn.disabled = true;
        mergeBtn.classList.add("opacity-40", "cursor-not-allowed");
        mergeBtn.title = "Select at least 2 collections to merge";
      }
    }

    if (list) {
      list.innerHTML = selected
        .map(
          (folder) => `

                        <span
                            class="shrink-0
                                   rounded-md
                                   border
                                   border-cyan-500/20
                                   bg-cyan-500/10
                                   px-2 py-1
                                   font-mono
                                   text-[10px]
                                   text-cyan-300"
                        >
                            ${escapeHtml(folder.name)}
                        </span>

                    `,
        )
        .join("");
    }
  }

  function updateSelectAllState() {
    const selectAll = document.getElementById("selectAllFolders");

    if (!selectAll) return;

    const items = currentPageItems();

    const selectedCount = items.filter((folder) =>
      state.selected.has(Number(folder.id)),
    ).length;

    selectAll.checked = items.length > 0 && selectedCount === items.length;

    selectAll.indeterminate = selectedCount > 0 && selectedCount < items.length;
  }

  function getSelectedFolders() {
    return state.folders.filter((folder) =>
      state.selected.has(Number(folder.id)),
    );
  }

  // ============================================================
  // TOOLBAR
  // ============================================================

  function wireToolbar() {
    document
      .getElementById("collectionSearch")
      ?.addEventListener("input", (event) => {
        state.search = event.target.value;

        state.page = 1;

        applyFilters();
      });

    document
      .getElementById("purposeFilter")
      ?.addEventListener("change", (event) => {
        state.purpose = event.target.value;

        state.page = 1;

        applyFilters();
      });

    document
      .getElementById("resetFilters")
      ?.addEventListener("click", resetFilters);

    document
      .getElementById("newCollectionBtn")
      ?.addEventListener("click", openNewFolderModal);

    document
      .getElementById("importBtn")
      ?.addEventListener("click", openImportModal);

    document
      .getElementById("exportBtn")
      ?.addEventListener("click", openExportModal);

    document.getElementById("paginationPrev")?.addEventListener("click", () => {
      if (state.page > 1) {
        state.page--;

        render();
      }
    });

    document.getElementById("paginationNext")?.addEventListener("click", () => {
      const totalPages = getTotalPages();

      if (state.page < totalPages) {
        state.page++;

        render();
      }
    });
  }

  function resetFilters() {
    state.search = "";

    state.purpose = "all";

    state.tags.clear();

    state.segments.clear();

    state.page = 1;

    const search = document.getElementById("collectionSearch");

    if (search) {
      search.value = "";
    }

    const purpose = document.getElementById("purposeFilter");

    if (purpose) {
      purpose.value = "all";
    }

    applyFilters();
  }

  // ============================================================
  // TABLE EVENTS
  // ============================================================

  function wireTableEvents() {
    const tbody = document.getElementById("folderTableBody");

    if (!tbody) return;

    tbody.addEventListener("contextmenu", () => {
      /*
       * rmbcv.js owns the actual context menu.
       */
    });
  }

  // ============================================================
  // FOLDER OPERATIONS
  // ============================================================

  function getFolder(id) {
    return state.folders.find((folder) => Number(folder.id) === Number(id));
  }

  async function duplicateFolder(id) {
    const folder = getFolder(id);

    if (!folder) return;

    const api = getApi();

    if (!api || typeof api.createCollection !== "function") {
      showAlert("Collection API is not available.");

      return;
    }

    const name = `${folder.name} (copy)`;

    try {
      const response = await api.createCollection(name);

      if (response?.ok === false) {
        throw new Error(
          `Failed to create collection (${response.status ?? "unknown"})`,
        );
      }

      /*
       * Copy actual endpoint membership using the backend
       * collection API.
       */
      if (typeof api.addEndpointToCollection === "function") {
        for (const endpoint of folder.endpoints || []) {
          const endpointId = endpoint.endpoint_id ?? endpoint.id;

          if (!endpointId) {
            continue;
          }

          const result = await api.addEndpointToCollection(name, endpointId);

          if (result?.ok === false) {
            console.error(
              `Failed to copy endpoint ${endpointId} to ${name}`,
              result,
            );
          }
        }
      }

      /*
       * Copy actual membership tags.
       */
      if (typeof api.addMembershipTag === "function") {
        for (const tag of folder.tags || []) {
          const result = await api.addMembershipTag(name, tag);

          if (result?.ok === false) {
            console.error(`Failed to copy tag ${tag} to ${name}`, result);
          }
        }
      }

      await loadFolders();

      renderSidebar();

      applyFilters();
    } catch (error) {
      console.error("Failed to duplicate collection:", error);

      showAlert(`Failed to duplicate collection: ${error.message}`);
    }
  }

  async function createFolder(name, purpose, tags, annotation) {
    const collectionName = String(name || "").trim();

    if (!collectionName) {
      return;
    }

    const api = getApi();

    if (!api || typeof api.createCollection !== "function") {
      showAlert("Collection API is not available.");

      return;
    }

    try {
      const response = await api.createCollection(collectionName);

      if (response?.ok === false) {
        throw new Error(
          `Failed to create collection (${response.status ?? "unknown"})`,
        );
      }

      /*
       * Purpose and annotation are intentionally ignored:
       * there is no corresponding backend collection field/API.
       */

      const collectionTags = String(tags || "")
        .split(",")
        .map((tag) => tag.trim())
        .filter(Boolean);

      if (typeof api.addMembershipTag === "function") {
        for (const tag of collectionTags) {
          const tagResponse = await api.addMembershipTag(collectionName, tag);

          if (tagResponse?.ok === false) {
            console.error(`Failed to add tag ${tag}:`, tagResponse);
          }
        }
      }

      await loadFolders();

      renderSidebar();

      applyFilters();
    } catch (error) {
      console.error("Failed to create collection:", error);

      showAlert(`Failed to create collection: ${error.message}`);
    }
  }

  async function deleteFolders(ids) {
    const targets = ids.map(getFolder).filter(Boolean);

    if (!targets.length) {
      return;
    }

    const results = await Promise.allSettled(
      targets.map((folder) => deleteFolderOnBackend(folder)),
    );

    const succeededNames = [];

    const failedNames = [];

    results.forEach((result, index) => {
      const folder = targets[index];

      if (result.status === "fulfilled" && result.value?.ok !== false) {
        succeededNames.push(folder.name);
      } else {
        failedNames.push(folder.name);
      }
    });

    if (failedNames.length) {
      showAlert(`Failed to delete on server: ${failedNames.join(", ")}`);
    }

    if (succeededNames.length) {
      const deletedIds = targets
        .filter((folder) => succeededNames.includes(folder.name))
        .map((folder) => Number(folder.id));

      deletedIds.forEach((id) => state.selected.delete(id));

      if (deletedIds.includes(Number(state.activeFolderId))) {
        state.activeFolderId = null;
      }

      if (deletedIds.includes(Number(state.prevActiveFolderId))) {
        state.prevActiveFolderId = null;
      }
    }

    await loadFolders();

    renderSidebar();

    applyFilters();
  }

  async function deleteFolderOnBackend(folder) {
    const api = getApi();

    if (!api || typeof api.deleteCollection !== "function") {
      throw new Error("EdmsAPI.deleteCollection() is not available.");
    }

    const response = await api.deleteCollection(folder.name);

    if (response?.ok === false) {
      throw new Error(`HTTP ${response.status ?? "unknown"}`);
    }

    return response;
  }

  // ============================================================
  // MERGE
  // ============================================================

  function openMergeModal(ids) {
    const targetIds = (ids && ids.length) ? ids : [...state.selected];
    const uniqueIds = [...new Set(targetIds.map(Number))];
    const sourceFolders = uniqueIds.map(getFolder).filter(Boolean);

    if (sourceFolders.length < 2) {
      showAlert("Select at least two collections to merge.");
      return;
    }

    let mergeMode = "collection"; // "collection" | "tag"
    const tagSelections = {};
    const availableTagsMap = {};

    sourceFolders.forEach((f) => {
      const initialTags = Array.isArray(f.tags) ? [...f.tags] : [];
      if (Array.isArray(f.endpoints)) {
        f.endpoints.forEach((ep) => {
          if (Array.isArray(ep.tags)) {
            ep.tags.forEach((t) => {
              if (t && typeof t === "string") initialTags.push(t.trim());
              else if (t?.name) initialTags.push(String(t.name).trim());
            });
          }
        });
      }
      availableTagsMap[f.name] = [...new Set(initialTags.filter(Boolean))];
      tagSelections[f.name] = new Set();
    });

    openModal(`
      <div class="space-y-4">
        <div class="flex items-center justify-between border-b border-slate-800 pb-3">
          <div>
            <h2 class="text-sm font-semibold text-white flex items-center gap-2">
              <svg class="h-4 w-4 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M8 7v10M16 7v10M8 12h8" />
              </svg>
              Merge Collections
            </h2>
            <p class="mt-0.5 text-[11px] text-slate-400">
              Combine endpoints from selected collections into a target collection.
            </p>
          </div>
          <button
            type="button"
            data-modal-close
            class="rounded-md p-1 text-slate-500 hover:bg-slate-800 hover:text-white"
            title="Close"
          >
            ${iconClose()}
          </button>
        </div>

        <!-- Mode Segmented Control -->
        <div>
          <div class="flex items-center justify-between mb-1.5">
            <label class="block text-xs font-medium text-slate-300">
              Merge Mode
            </label>
            <span id="mergeModeHint" class="text-[11px] text-slate-400">
              Merge all endpoints from source collections
            </span>
          </div>
          <div class="grid grid-cols-2 gap-1 rounded-lg bg-slate-900 p-1 border border-slate-800 text-xs">
            <button
              type="button"
              id="modeBtnCollection"
              class="flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-white bg-slate-800 shadow-sm border border-slate-700/60"
            >
              <svg class="h-3.5 w-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
              </svg>
              Collection Mode
            </button>
            <button
              type="button"
              id="modeBtnTag"
              class="flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-slate-400 hover:text-slate-200 border border-transparent"
            >
              <svg class="h-3.5 w-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
              </svg>
              Tag Mode
            </button>
          </div>
        </div>

        <!-- Collection Mode Section -->
        <div id="mergeCollectionModeSection">
          <label class="mb-1.5 block text-xs font-medium text-slate-300">
            Selected Collections (${sourceFolders.length})
          </label>
          <div class="max-h-36 overflow-y-auto rounded-lg border border-slate-800 bg-slate-950/70 p-2 space-y-1.5 scrollbar-thin scrollbar-thumb-slate-800">
            ${sourceFolders
              .map(
                (f) => `
                <div class="flex items-center justify-between rounded px-2.5 py-1.5 bg-slate-900/80 border border-slate-800/80">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="inline-block h-2 w-2 rounded-full bg-cyan-400"></span>
                    <span class="text-xs font-mono font-medium text-slate-200 truncate">${escapeHtml(f.name)}</span>
                    ${f.annotation ? `<span class="text-[10px] text-slate-400 truncate">(${escapeHtml(f.annotation)})</span>` : ""}
                  </div>
                  <span class="shrink-0 text-[10px] text-cyan-400/80 font-mono">
                    ${Array.isArray(f.endpoints) ? f.endpoints.length : (f.endpoint_count ?? 0)} endpoints
                  </span>
                </div>
              `,
              )
              .join("")}
          </div>
        </div>

        <!-- Tag Mode Section -->
        <div id="mergeTagModeSection" class="hidden space-y-2">
          <div class="flex items-center justify-between">
            <label class="block text-xs font-medium text-slate-300">
              Filter Endpoints by Tag
            </label>
            <span class="text-[10px] text-slate-400">
              Only matching endpoints will merge into target
            </span>
          </div>
          <div id="mergeTagSourcesList" class="max-h-56 overflow-y-auto rounded-lg border border-slate-800 bg-slate-950/70 p-2 space-y-2.5 scrollbar-thin scrollbar-thumb-slate-800">
            <!-- Dynamically populated -->
          </div>
        </div>

        <!-- Primary / Target Collection Section -->
        <div>
          <label class="mb-1.5 block text-xs font-medium text-slate-300">
            Primary / Target Collection <span class="text-rose-400">*</span>
          </label>
          <select
            id="mergeTargetSelect"
            class="mb-2 h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs text-slate-200 outline-none focus:border-cyan-500"
          >
            ${sourceFolders
              .map(
                (f, idx) => `
                <option value="${escapeAttr(f.name)}" ${idx === 0 ? "selected" : ""}>
                  ${escapeHtml(f.name)} (Merge sources into this collection)
                </option>
              `,
              )
              .join("")}
            <option value="__custom__">+ Enter custom / new target collection name...</option>
          </select>
          <input
            id="customMergeTargetInput"
            type="text"
            placeholder="Enter target collection name"
            class="hidden h-9 w-full rounded-md border border-slate-700 bg-slate-950 px-3 text-xs text-slate-200 outline-none focus:border-cyan-500"
          />
        </div>

        <div id="mergeStatusArea" class="hidden rounded-lg border px-3 py-2 text-xs"></div>

        <div class="flex items-center justify-end gap-2 pt-3 border-t border-slate-800">
          <button
            type="button"
            id="cancelMergeBtn"
            data-modal-close
            class="rounded-md border border-slate-700 px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800"
          >
            Cancel
          </button>
          <button
            id="confirmMergeBtn"
            type="button"
            class="inline-flex items-center gap-1.5 rounded-md bg-cyan-600 px-3.5 py-1.5 text-xs font-medium text-white hover:bg-cyan-500 transition shadow-sm"
          >
            <span id="mergeBtnSpinner" class="hidden">
              <svg class="h-3.5 w-3.5 animate-spin text-white" viewBox="0 0 24 24" fill="none">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
              </svg>
            </span>
            <span id="mergeBtnText">Merge Collections</span>
          </button>
        </div>
      </div>
    `);

    const modeBtnCollection = document.getElementById("modeBtnCollection");
    const modeBtnTag = document.getElementById("modeBtnTag");
    const collectionModeSection = document.getElementById("mergeCollectionModeSection");
    const tagModeSection = document.getElementById("mergeTagModeSection");
    const tagSourcesList = document.getElementById("mergeTagSourcesList");
    const modeHint = document.getElementById("mergeModeHint");
    const targetSelect = document.getElementById("mergeTargetSelect");
    const customInput = document.getElementById("customMergeTargetInput");
    const confirmBtn = document.getElementById("confirmMergeBtn");
    const cancelBtn = document.getElementById("cancelMergeBtn");
    const statusArea = document.getElementById("mergeStatusArea");
    const spinner = document.getElementById("mergeBtnSpinner");
    const btnText = document.getElementById("mergeBtnText");

    function getSelectedTargetName() {
      if (!targetSelect) return "";
      if (targetSelect.value === "__custom__") {
        return customInput?.value.trim() || "";
      }
      return targetSelect.value;
    }

    function renderTagSourcesList() {
      if (!tagSourcesList) return;

      const currentTarget = getSelectedTargetName();
      const activeSources = sourceFolders.filter((f) => f.name !== currentTarget);

      if (activeSources.length === 0) {
        tagSourcesList.innerHTML = `
          <div class="py-4 text-center text-xs text-slate-500">
            No source collections available for the current target.
          </div>
        `;
        return;
      }

      tagSourcesList.innerHTML = activeSources
        .map((f) => {
          const selected = tagSelections[f.name] || new Set();
          const available = availableTagsMap[f.name] || [];
          const unselectedAvail = available.filter((t) => !selected.has(t));

          const selectedChips = Array.from(selected)
            .map(
              (tag) => `
              <span class="inline-flex items-center gap-1 rounded bg-cyan-950/80 border border-cyan-700/60 px-2 py-0.5 text-[11px] font-mono text-cyan-300">
                <span>${escapeHtml(tag)}</span>
                <button
                  type="button"
                  data-remove-tag="${escapeAttr(tag)}"
                  data-col="${escapeAttr(f.name)}"
                  class="text-cyan-400 hover:text-cyan-100 hover:bg-cyan-800/50 rounded p-0.5 ml-0.5 transition"
                  title="Remove tag"
                >
                  <svg class="h-2.5 w-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <line x1="18" y1="6" x2="6" y2="18"></line>
                    <line x1="6" y1="6" x2="18" y2="18"></line>
                  </svg>
                </button>
              </span>
            `,
            )
            .join("");

          const availPills = unselectedAvail
            .map(
              (tag) => `
              <button
                type="button"
                data-add-tag="${escapeAttr(tag)}"
                data-col="${escapeAttr(f.name)}"
                class="inline-flex items-center rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white px-1.5 py-0.5 text-[10px] font-mono transition border border-slate-700/60"
                title="Add tag ${escapeAttr(tag)}"
              >
                + ${escapeHtml(tag)}
              </button>
            `,
            )
            .join("");

          return `
            <div class="rounded-md border border-slate-800/80 bg-slate-900/80 p-2.5 space-y-2">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="inline-block h-2 w-2 rounded-full bg-cyan-400"></span>
                  <span class="text-xs font-mono font-medium text-slate-200 truncate">${escapeHtml(f.name)}</span>
                  <span class="text-[10px] text-cyan-400/80 font-mono">
                    (${Array.isArray(f.endpoints) ? f.endpoints.length : (f.endpoint_count ?? 0)} eps)
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  ${
                    unselectedAvail.length > 0
                      ? `
                    <button
                      type="button"
                      data-select-all-tags="${escapeAttr(f.name)}"
                      class="text-[10px] text-cyan-400 hover:text-cyan-300 underline font-mono"
                    >
                      Add all (${unselectedAvail.length})
                    </button>
                  `
                      : ""
                  }
                  <span class="text-[10px] font-mono ${selected.size === 0 ? "text-amber-400 font-semibold" : "text-slate-400"}">
                    ${selected.size === 0 ? "0 tags (required)" : `${selected.size} selected`}
                  </span>
                </div>
              </div>

              <!-- Selected Tag Chips -->
              <div class="flex flex-wrap items-center gap-1.5 min-h-[26px] p-1 rounded bg-slate-950/70 border ${selected.size === 0 ? "border-amber-500/30" : "border-slate-800"}">
                ${
                  selected.size > 0
                    ? selectedChips
                    : `<span class="text-[11px] text-amber-400/70 italic px-1">No tags selected. Click an available tag below or type custom tag.</span>`
                }
              </div>

              <!-- Available & custom tag input -->
              <div class="flex flex-wrap items-center gap-1.5 pt-0.5">
                ${availPills}
                <div class="inline-flex items-center gap-1 ml-auto">
                  <input
                    type="text"
                    data-tag-input-for="${escapeAttr(f.name)}"
                    placeholder="+ custom tag"
                    class="h-6 w-24 rounded border border-slate-700 bg-slate-950 px-2 text-[11px] text-slate-200 outline-none focus:border-cyan-500 focus:w-32 transition-all"
                  />
                  <button
                    type="button"
                    data-tag-btn-for="${escapeAttr(f.name)}"
                    class="h-6 px-1.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 text-[11px] font-semibold border border-slate-700"
                    title="Add tag"
                  >
                    +
                  </button>
                </div>
              </div>
            </div>
          `;
        })
        .join("");
    }

    tagSourcesList?.addEventListener("click", (e) => {
      const removeBtn = e.target.closest("[data-remove-tag]");
      if (removeBtn) {
        const col = removeBtn.getAttribute("data-col");
        const tag = removeBtn.getAttribute("data-remove-tag");
        if (col && tag && tagSelections[col]) {
          tagSelections[col].delete(tag);
          renderTagSourcesList();
        }
        return;
      }

      const addBtn = e.target.closest("[data-add-tag]");
      if (addBtn) {
        const col = addBtn.getAttribute("data-col");
        const tag = addBtn.getAttribute("data-add-tag");
        if (col && tag) {
          if (!tagSelections[col]) tagSelections[col] = new Set();
          tagSelections[col].add(tag);
          renderTagSourcesList();
        }
        return;
      }

      const selectAllBtn = e.target.closest("[data-select-all-tags]");
      if (selectAllBtn) {
        const col = selectAllBtn.getAttribute("data-select-all-tags");
        if (col && availableTagsMap[col]) {
          if (!tagSelections[col]) tagSelections[col] = new Set();
          availableTagsMap[col].forEach((t) => tagSelections[col].add(t));
          renderTagSourcesList();
        }
        return;
      }

      const customBtn = e.target.closest("[data-tag-btn-for]");
      if (customBtn) {
        const col = customBtn.getAttribute("data-tag-btn-for");
        const inputs = Array.from(tagSourcesList.querySelectorAll("[data-tag-input-for]"));
        const input = inputs.find((el) => el.getAttribute("data-tag-input-for") === col);
        if (col && input && input.value.trim()) {
          const val = input.value.trim();
          if (!tagSelections[col]) tagSelections[col] = new Set();
          tagSelections[col].add(val);
          if (!availableTagsMap[col]) availableTagsMap[col] = [];
          if (!availableTagsMap[col].includes(val)) availableTagsMap[col].push(val);
          renderTagSourcesList();
        }
        return;
      }
    });

    tagSourcesList?.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && e.target.matches("[data-tag-input-for]")) {
        e.preventDefault();
        const input = e.target;
        const col = input.getAttribute("data-tag-input-for");
        if (col && input.value.trim()) {
          const val = input.value.trim();
          if (!tagSelections[col]) tagSelections[col] = new Set();
          tagSelections[col].add(val);
          if (!availableTagsMap[col]) availableTagsMap[col] = [];
          if (!availableTagsMap[col].includes(val)) availableTagsMap[col].push(val);
          renderTagSourcesList();
        }
      }
    });

    async function fetchMissingTags() {
      const api = getApi();
      if (!api || typeof api.listMembershipTags !== "function") return;

      for (const folder of sourceFolders) {
        if (!availableTagsMap[folder.name] || availableTagsMap[folder.name].length === 0) {
          try {
            const resp = await api.listMembershipTags(folder.name);
            const data = getApiData(resp);
            const tags = Array.isArray(data)
              ? data
              : Array.isArray(data?.tags)
                ? data.tags
                : Array.isArray(data?.items)
                  ? data.items
                  : [];
            const parsed = tags
              .map((t) => (typeof t === "string" ? t : String(t?.name ?? t?.tag ?? "")))
              .map((t) => t.trim())
              .filter(Boolean);
            if (parsed.length > 0) {
              const current = availableTagsMap[folder.name] || [];
              availableTagsMap[folder.name] = [...new Set([...current, ...parsed])];
              if (mergeMode === "tag") {
                renderTagSourcesList();
              }
            }
          } catch (err) {
            console.warn(`Could not load tags for ${folder.name}:`, err);
          }
        }
      }
    }

    function setMode(mode) {
      mergeMode = mode;
      if (mode === "collection") {
        modeBtnCollection.className =
          "flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-white bg-slate-800 shadow-sm border border-slate-700/60";
        modeBtnTag.className =
          "flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-slate-400 hover:text-slate-200 border border-transparent";
        collectionModeSection?.classList.remove("hidden");
        tagModeSection?.classList.add("hidden");
        if (modeHint) modeHint.textContent = "Merge all endpoints from source collections";
      } else {
        modeBtnTag.className =
          "flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-white bg-slate-800 shadow-sm border border-slate-700/60";
        modeBtnCollection.className =
          "flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 font-medium transition text-slate-400 hover:text-slate-200 border border-transparent";
        collectionModeSection?.classList.add("hidden");
        tagModeSection?.classList.remove("hidden");
        if (modeHint) modeHint.textContent = "Merge only endpoints matching selected tags";
        renderTagSourcesList();
        fetchMissingTags();
      }
    }

    modeBtnCollection?.addEventListener("click", () => setMode("collection"));
    modeBtnTag?.addEventListener("click", () => setMode("tag"));

    targetSelect?.addEventListener("change", () => {
      if (targetSelect.value === "__custom__") {
        customInput?.classList.remove("hidden");
        customInput?.focus();
      } else {
        customInput?.classList.add("hidden");
      }
      if (mergeMode === "tag") {
        renderTagSourcesList();
      }
    });

    customInput?.addEventListener("input", () => {
      if (mergeMode === "tag") {
        renderTagSourcesList();
      }
    });

    confirmBtn?.addEventListener("click", async () => {
      let targetName = "";
      if (targetSelect.value === "__custom__") {
        targetName = customInput?.value.trim() || "";
        if (!targetName) {
          showMergeStatus("Target collection name is required.", "error");
          return;
        }
      } else {
        targetName = targetSelect.value;
      }

      const activeSources = sourceFolders
        .map((f) => f.name)
        .filter((name) => name !== targetName);

      if (activeSources.length === 0) {
        showMergeStatus("At least one distinct source collection is required to merge.", "error");
        return;
      }

      let payload;
      if (mergeMode === "collection") {
        payload = { sources: activeSources };
      } else {
        const missingTags = activeSources.filter(
          (name) => !tagSelections[name] || tagSelections[name].size === 0,
        );

        if (missingTags.length > 0) {
          showMergeStatus(
            `Please select or enter at least one tag for "${missingTags[0]}".`,
            "error",
          );
          return;
        }

        payload = {
          sources: activeSources.map((name) => ({
            collection: name,
            tags: Array.from(tagSelections[name]),
          })),
        };
      }

      setMergeLoading(true);
      showMergeStatus(`Initiating merge into "${targetName}"...`, "info");

      const api = getApi();
      if (!api || typeof api.mergeDataViewFolder !== "function") {
        showMergeStatus("Merge API is not available.", "error");
        setMergeLoading(false);
        return;
      }

      try {
        const response = await api.mergeDataViewFolder(targetName, payload);

        if (response?.ok === false && response?.status !== 202) {
          const httpCode = response?.status ?? "unknown";
          const errMsg =
            response?.data?.error ||
            response?.data?.message ||
            `Merge failed (HTTP ${httpCode}).`;
          showMergeStatus(`[${httpCode}] ${errMsg}`, "error");
          setMergeLoading(false);
          return;
        }

        // Response is 202 Accepted
        clearSelection();

        // Extract the job_id returned by the server so we can match the
        // specific ViewIoDone event rather than accepting any merge event.
        const jobId = response?.data?.job_id || response?.data?.id || null;

        showMergeStatus(
          `Merge accepted (job ${jobId ?? "—"}). Waiting for completion...`,
          "success",
        );

        let wsFinished = false;

        const onMergeComplete = async (eventData) => {
          if (wsFinished) return;
          wsFinished = true;
          listenerHandle?.cancel?.();

          clearSelection();
          closeModal();
          await loadFolders();
          renderSidebar();
          applyFilters();

          const res = eventData?.result || eventData?.data || eventData;
          let successMsg = `Successfully merged collections into "${targetName}".`;
          if (res && typeof res === "object") {
            const added = res.added ?? res.endpoints_added ?? res.added_count;
            const created = res.created;
            const after = res.after ?? res.endpoints_after;

            const details = [];
            if (added !== undefined) {
              details.push(`${added} endpoint${added === 1 ? "" : "s"} added`);
            }
            if (created) {
              details.push("created new collection");
            } else if (after !== undefined) {
              details.push(`${after} total endpoints`);
            }

            if (details.length > 0) {
              successMsg = `Merge into "${targetName}" complete (${details.join(", ")}).`;
            }
          }
          showAlert(successMsg);
        };

        const onMergeFailed = (eventData) => {
          if (wsFinished) return;
          wsFinished = true;
          listenerHandle?.cancel?.();

          const reason =
            eventData?.error ||
            eventData?.message ||
            eventData?.data?.error ||
            "The merge job reported a failure.";
          showMergeStatus(`Merge failed: ${reason}`, "error");
          setMergeLoading(false);
        };

        // Use job-scoped listener when we have a job_id; fall back to the
        // legacy folder-name listener otherwise.
        let listenerHandle = null;
        if (jobId && typeof api.listenForMergeJob === "function") {
          listenerHandle = api.listenForMergeJob(jobId, onMergeComplete, onMergeFailed);
        } else if (typeof api.listenForMergeNotification === "function") {
          // Legacy path: fires on any ViewIoDone — acceptable when no job_id.
          const cleanup = api.listenForMergeNotification(targetName, onMergeComplete, 6000);
          listenerHandle = { cancel: cleanup };
        }

        // Fallback timer in case the WebSocket never fires (e.g. WS blocked).
        setTimeout(async () => {
          if (!wsFinished) {
            wsFinished = true;
            listenerHandle?.cancel?.();
            clearSelection();
            closeModal();
            await loadFolders();
            renderSidebar();
            applyFilters();
            showAlert(`Collections merge into "${targetName}" completed.`);
          }
        }, 6500);
      } catch (error) {
        console.error("Failed to merge collections:", error);
        showMergeStatus(`Merge failed: ${error.message || "An unexpected error occurred."}`, "error");
        setMergeLoading(false);
      }
    });

    function setMergeLoading(loading) {
      if (confirmBtn) {
        confirmBtn.disabled = loading;
        if (loading) {
          confirmBtn.classList.add("opacity-60", "cursor-not-allowed");
          spinner?.classList.remove("hidden");
          if (btnText) btnText.textContent = "Merging...";
        } else {
          confirmBtn.classList.remove("opacity-60", "cursor-not-allowed");
          spinner?.classList.add("hidden");
          if (btnText) btnText.textContent = "Merge Collections";
        }
      }
      if (cancelBtn) cancelBtn.disabled = loading;
      if (targetSelect) targetSelect.disabled = loading;
      if (customInput) customInput.disabled = loading;
      if (modeBtnCollection) modeBtnCollection.disabled = loading;
      if (modeBtnTag) modeBtnTag.disabled = loading;
    }

    function showMergeStatus(msg, type) {
      if (!statusArea) return;
      statusArea.classList.remove(
        "hidden",
        "border-rose-500/40",
        "bg-rose-500/10",
        "text-rose-300",
        "border-cyan-500/40",
        "bg-cyan-500/10",
        "text-cyan-300",
        "border-amber-500/40",
        "bg-amber-500/10",
        "text-amber-300",
      );

      if (type === "error") {
        statusArea.classList.add("border-rose-500/40", "bg-rose-500/10", "text-rose-300");
      } else if (type === "success") {
        statusArea.classList.add("border-cyan-500/40", "bg-cyan-500/10", "text-cyan-300");
      } else {
        statusArea.classList.add("border-amber-500/40", "bg-amber-500/10", "text-amber-300");
      }

      statusArea.textContent = msg;
    }
  }

  // ============================================================
  // TEST VIEW
  // ============================================================

  function openTestView(id) {
    const folder = getFolder(id);

    if (!folder) {
      return;
    }

    window.open(
      `./testview.html?collection=${encodeURIComponent(folder.name)}`,
      "_blank",
    );
  }



  // ============================================================
  // DATA VIEW
  // ============================================================

  function openDataView(id) {
    const folder = getFolder(id);

    if (!folder) {
      return;
    }

    window.open(
      `./collection_dataview.html?folder=${encodeURIComponent(folder.name)}`,
      "_blank",
    );
  }

  // ============================================================
  // REPO VIEW
  // ============================================================

  function openRepoViewModal(id) {
    const folder = getFolder(id);

    if (!folder) {
      return;
    }

    openModal(`
        <h2
            class=" text-sm font-semibold
                   text-slate-900 dark:text-white"
        >
            Create Repo View
        </h2>
        <h3 class="text-sm dark:text-blue-400 text-blue-600 mb-3 font-bold">
          ${folder.name}
        </h3>

        <label
            for="repoViewNameInput"
            class="mb-1 block
                   text-[11px] text-slate-500"
        >
            RepoView Name
        </label>

        <input
            id="repoViewNameInput"
            type="text"
            autocomplete="off"
            class="mb-4 h-9 w-full
                   rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3
                   text-xs text-slate-900 dark:text-slate-100
                   placeholder:text-slate-400 dark:placeholder:text-slate-600
                   outline-none
                   focus:border-cyan-500"
            placeholder="RepoView Name"
            value="${folder.name}-repoview"
        >

        <div
            class="flex justify-end
                   gap-2"
        >
            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-300 dark:border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-600 dark:text-slate-400
                       hover:bg-slate-100 dark:hover:bg-slate-800"
            >
                Cancel
            </button>

            <button
                id="confirmCreateRepoView"
                type="button"
                class="rounded-md
                       bg-slate-900 dark:bg-cyan-600
                       px-3 py-1.5
                       text-xs font-medium
                       text-white
                       hover:bg-slate-800 dark:hover:bg-cyan-500"
            >
                Create
            </button>
        </div>
    `);

    const input = document.getElementById("repoViewNameInput");
    const createBtn = document.getElementById("confirmCreateRepoView");

    input?.focus();

    async function handleCreate() {
      const name = input?.value?.trim() || "";

      if (!name) {
        showAlert("RepoView name cannot be empty.");
        input?.focus();
        return;
      }

      const api = getApi();
      if (!api || typeof api.createRepoview !== "function") {
        showAlert("Repo View API is not available.");
        return;
      }

      createBtn.disabled = true;
      const originalText = createBtn.textContent;
      createBtn.textContent = "Creating…";

      try {
        if (typeof api.listRepoviews === "function") {
          try {
            const listRes = await api.listRepoviews();
            const items = Array.isArray(listRes?.data?.items)
              ? listRes.data.items
              : Array.isArray(listRes?.data)
              ? listRes.data
              : [];
            if (
              items.some(
                (repo) => (repo.name || "").toLowerCase() === name.toLowerCase()
              )
            ) {
              showAlert(`A Repo View named "${name}" already exists.`);
              createBtn.disabled = false;
              createBtn.textContent = originalText;
              input?.focus();
              return;
            }
          } catch (e) {
            // Server check will still enforce uniqueness
          }
        }

        const res = await api.createRepoview({
          name,
          source_collection: folder.name,
        });

        if (!res || !res.ok) {
          const errMsg =
            res?.data?.error ||
            res?.data?.message ||
            (typeof res?.data === "string" ? res.data : "") ||
            `Failed to create Repo View (HTTP ${res?.status ?? "unknown"})`;
          showAlert(errMsg);
          createBtn.disabled = false;
          createBtn.textContent = originalText;
          input?.focus();
          return;
        }

        closeModal();
        showAlert(`Repo View "${name}" created successfully.`);

        if (window.RepoView && typeof window.RepoView.loadData === "function") {
          try {
            await window.RepoView.loadData();
            if (typeof window.RepoView.renderSidebar === "function") {
              window.RepoView.renderSidebar();
            }
            if (typeof window.RepoView.applyFilters === "function") {
              window.RepoView.applyFilters();
            }
          } catch (e) {
            console.warn("Failed to refresh Repo View listing:", e);
          }
        }

        if (typeof loadFolders === "function") {
          try {
            await loadFolders();
            if (typeof renderSidebar === "function") {
              renderSidebar();
            }
            if (typeof applyFilters === "function") {
              applyFilters();
            }
          } catch (e) {
            console.warn("Failed to refresh collections:", e);
          }
        }
      } catch (error) {
        console.error("Failed to create Repo View:", error);
        showAlert(`Error creating repo view: ${error.message || error}`);
        createBtn.disabled = false;
        createBtn.textContent = originalText;
      }
    }

    createBtn?.addEventListener("click", handleCreate);

    input?.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleCreate();
      }
    });
  }

  // ============================================================
  // NEW FOLDER
  // ============================================================

  function openNewFolderModal() {
    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-slate-900 dark:text-white"
        >
            New Empty Folder
        </h2>


        <label
            class="mb-1 block
                   text-[11px] text-slate-500"
        >
            Folder name
        </label>

        <input
            id="newFolderName"
            class="mb-3 h-9 w-full
                   rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3
                   text-xs text-slate-900 dark:text-slate-100
                   placeholder:text-slate-400 dark:placeholder:text-slate-600
                   outline-none
                   focus:border-cyan-500"
            placeholder="My New Collection"
        >


        <label
            class="mb-1 block
                   text-[11px] text-slate-500"
        >
            Purpose
        </label>

        <select
            id="newFolderPurpose"
            class="mb-3 h-9 w-full
                   rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3
                   text-xs text-slate-900 dark:text-slate-100
                   outline-none
                   focus:border-cyan-500"
        >
            <option value="importable">
                Importable
            </option>

            <option value="exportable">
                Exportable
            </option>
        </select>


        <label
            class="mb-1 block
                   text-[11px] text-slate-500"
        >
            Tags
        </label>

        <input
            id="newFolderTags"
            class="mb-3 h-9 w-full
                   rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3
                   text-xs text-slate-900 dark:text-slate-100
                   placeholder:text-slate-400 dark:placeholder:text-slate-600
                   outline-none
                   focus:border-cyan-500"
            placeholder="personal, draft"
        >


        <label
            class="mb-1 block
                   text-[11px] text-slate-500"
        >
            Annotation
        </label>

        <textarea
            id="newFolderAnnotation"
            class="mb-4 h-20 w-full
                   resize-none rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3 py-2
                   text-xs text-slate-900 dark:text-slate-100
                   placeholder:text-slate-400 dark:placeholder:text-slate-600
                   outline-none
                   focus:border-cyan-500"
        ></textarea>


        <div
            class="flex justify-end
                   gap-2"
        >

            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-300 dark:border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-600 dark:text-slate-400
                       hover:bg-slate-100 dark:hover:bg-slate-800"
            >
                Cancel
            </button>


            <button
                id="confirmNewFolder"
                type="button"
                class="rounded-md
                       bg-slate-900 dark:bg-cyan-600
                       px-3 py-1.5
                       text-xs font-medium
                       text-white
                       hover:bg-slate-800 dark:hover:bg-cyan-500"
            >
                Create
            </button>

        </div>

    `);

    document
      .getElementById("confirmNewFolder")
      ?.addEventListener("click", async () => {
        const button = document.getElementById("confirmNewFolder");

        button.disabled = true;

        button.textContent = "Creating…";

        await createFolder(
          document.getElementById("newFolderName").value,

          document.getElementById("newFolderPurpose").value,

          document.getElementById("newFolderTags").value,

          document.getElementById("newFolderAnnotation").value,
        );

        closeModal();
      });
  }

  // ============================================================
  // RENAME
  // ============================================================

  function openRenameModal(id) {
    const folder = getFolder(id);

    if (!folder) return;

    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-slate-900 dark:text-white"
        >
            Rename Collection
        </h2>


        <input
            id="renameInput"
            value="${escapeAttr(folder.name)}"
            class="mb-4 h-9 w-full
                   rounded-md
                   border border-slate-300 dark:border-slate-700
                   bg-white dark:bg-slate-950 px-3
                   text-xs text-slate-900 dark:text-slate-100
                   outline-none
                   focus:border-cyan-500"
        >


        <div
            class="flex justify-end
                   gap-2"
        >

            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-300 dark:border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-600 dark:text-slate-400
                       hover:bg-slate-100 dark:hover:bg-slate-800"
            >
                Cancel
            </button>


            <button
                id="confirmRename"
                type="button"
                class="rounded-md
                       bg-slate-900 dark:bg-cyan-600
                       px-3 py-1.5
                       text-xs font-medium
                       text-white
                       hover:bg-slate-800 dark:hover:bg-cyan-500"
            >
                Save
            </button>

        </div>

    `);

    document
      .getElementById("confirmRename")
      ?.addEventListener("click", async () => {
        const name = document.getElementById("renameInput").value.trim();

        if (!name) {
          showAlert("Collection name cannot be empty.");

          return;
        }

        const api = getApi();

        if (!api || typeof api.renameCollection !== "function") {
          showAlert("Collection API is not available.");

          return;
        }

        try {
          const response = await api.renameCollection(folder.name, name);

          if (response?.ok === false) {
            throw new Error(`HTTP ${response.status ?? "unknown"}`);
          }

          closeModal();

          await loadFolders();

          renderSidebar();

          applyFilters();
        } catch (error) {
          console.error("Failed to rename collection:", error);

          showAlert(`Failed to rename collection: ${error.message}`);
        }
      });
  }

  // ============================================================
  // TAG EDITOR
  // ============================================================

  function openTagEditor(id) {
    const folder = getFolder(id);

    if (!folder) return;

    const currentTags = Array.isArray(folder.tags)
      ? folder.tags.join(", ")
      : "";
    const currentAnnotation = folder.annotation || "";

    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-slate-900 dark:text-white"
        >
            Collection — ${escapeHtml(folder.name)}
        </h2>


        <div class="space-y-4">
            <div>
                <label class="mb-1 block text-xs text-slate-500">Tags</label>
                <input
                    id="collectionTagInput"
                    value="${escapeAttr(currentTags)}"
                    class="h-9 w-full
                           rounded-md
           border border-slate-300 dark:border-slate-700
           bg-white dark:bg-slate-950 px-3
           text-xs text-slate-900 dark:text-slate-100
           outline-none
           focus:border-cyan-500"
                >
                <p
                    class="mt-1 text-[10px]
                           text-slate-500 dark:text-slate-600"
                >
                    Separate tags with commas.
                </p>
            </div>

            <div>
                <label class="mb-1 block text-xs text-slate-500">Annotation</label>
                <textarea
                    id="collectionAnnotationInput"
                    class="h-20 w-full rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 p-2 text-xs text-slate-900 dark:text-slate-100 outline-none focus:border-cyan-500"
                    placeholder="Optional notes..."
                >${escapeHtml(currentAnnotation)}</textarea>
            </div>
        </div>


        <div
            class="mt-4 flex justify-end
                   gap-2"
        >

            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-300 dark:border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-600 dark:text-slate-400
                       hover:bg-slate-100 dark:hover:bg-slate-800"
            >
                Cancel
            </button>


            <button
                id="saveCollectionTags"
                type="button"
                class="rounded-md
                       bg-cyan-600
                       px-3 py-1.5
                       text-xs font-medium
                       text-white
                       hover:bg-cyan-500"
            >
                Save
            </button>

        </div>

    `);

    document
      .getElementById("saveCollectionTags")
      ?.addEventListener("click", async () => {
        const api = getApi();

        if (
          !api ||
          typeof api.addMembershipTag !== "function" ||
          typeof api.removeMembershipTag !== "function"
        ) {
          showAlert("Membership tag API is not available.");

          return;
        }

        const input = document.getElementById("collectionTagInput").value;
        const annotationInput = document.getElementById("collectionAnnotationInput").value;

        const newTags = [
          ...new Set(
            input
              .split(",")
              .map((tag) => tag.trim())
              .filter(Boolean),
          ),
        ];

        const oldTags = Array.isArray(folder.tags) ? [...folder.tags] : [];

        try {
          const toRemove = oldTags.filter((tag) => !newTags.includes(tag));

          const toAdd = newTags.filter((tag) => !oldTags.includes(tag));

          for (const tag of toRemove) {
            await api.removeMembershipTag(folder.name, tag);
          }

          for (const tag of toAdd) {
            await api.addMembershipTag(folder.name, tag);
          }

          if (annotationInput !== currentAnnotation && typeof api.setCollectionAnnotation === "function") {
            await api.setCollectionAnnotation(folder.name, annotationInput);
            folder.annotation = annotationInput;
          } else if (annotationInput !== currentAnnotation) {
            // Fallback for UI if API is missing (matches previous logic)
            folder.annotation = annotationInput;
          }

          closeModal();

          await loadFolders();

          renderSidebar();

          applyFilters();
        } catch (error) {
          console.error("Failed to update collection:", error);

          showAlert(`Failed to update collection: ${error.message}`);
        }
      });
  }

  // ============================================================
  // ANNOTATION EDITOR
  // ============================================================

  function openAnnotationEditor(id) {
    const folder = getFolder(id);

    if (!folder) return;

    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-white"
        >
            Edit Annotation
        </h2>


        <textarea
            id="collectionAnnotationInput"
            class="h-28 w-full resize-y
                   rounded-md
                   border border-slate-700
                   bg-slate-950 p-3
                   text-xs leading-5
                   text-slate-800 dark:text-slate-300
                   outline-none
                   focus:border-cyan-500"
        >${escapeHtml(folder.annotation || "")}</textarea>


        <div
            class="mt-4 flex justify-end
                   gap-2"
        >

            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-400
                       hover:bg-slate-800"
            >
                Cancel
            </button>


            <button
                id="saveCollectionAnnotation"
                type="button"
                class="rounded-md
                       bg-cyan-600
                       px-3 py-1.5
                       text-xs font-medium
                       text-white
                       hover:bg-cyan-500"
            >
                Save
            </button>

        </div>

    `);

    document
      .getElementById("saveCollectionAnnotation")
      ?.addEventListener("click", () => {
        /*
         * Annotation has no backend API, so do not pretend
         * this value is persisted. Keep the existing local
         * UI behaviour only.
         */
        folder.annotation = document
          .getElementById("collectionAnnotationInput")
          .value.trim();

        closeModal();

        render();
      });
  }

  // ============================================================
  // DELETE
  // ============================================================

  function openDeleteModal(ids) {
    const folders = ids.map(getFolder).filter(Boolean);

    if (!folders.length) return;

    openModal(`

        <div
            class="flex items-start
                   gap-3 rounded-lg
                   border
                   border-rose-500/20
                   bg-rose-500/5 p-3"
        >

            <div
                class="mt-0.5
                       text-rose-400"
            >
                ${iconTrash()}
            </div>


            <div>

                <h2
                    class="text-sm
                           font-semibold
                           text-rose-300"
                >
                    Delete
                    ${folders.length > 1 ? "Collections" : "Collection"}
                </h2>

                <p
                    class="mt-1 text-[11px]
                           text-slate-500"
                >
                    This removes the selected
                    collection(s) from the backend.
                </p>

            </div>

        </div>


        <div
            class="my-4 max-h-48
                   overflow-y-auto
                   rounded-lg
                   border border-slate-200 dark:border-slate-800"
        >

            ${folders
        .map(
          (folder) => `

                            <div
                                class="border-b
                                       border-slate-200 dark:border-slate-800
                                       px-3 py-2
                                       last:border-0"
                            >

                                <p
                                    class="font-mono
                                           text-xs
                                           text-slate-800 dark:text-slate-300"
                                >
                                    ${escapeHtml(folder.name)}
                                </p>

                                <p
                                    class="mt-0.5
                                           text-[10px]
                                           text-slate-600"
                                >
                                    ${Array.isArray(folder.endpoints)
              ? folder.endpoints.length
              : 0
            }
                                    endpoints
                                </p>

                            </div>

                        `,
        )
        .join("")}

        </div>


        <div
            class="flex justify-end
                   gap-2"
        >

            <button
                type="button"
                data-modal-close
                class="rounded-md
                       border border-slate-700
                       px-3 py-1.5
                       text-xs text-slate-400
                       hover:bg-slate-800"
            >
                Cancel
            </button>


            <button
                id="confirmDelete"
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

    `);

    document
      .getElementById("confirmDelete")
      ?.addEventListener("click", async (event) => {
        const button = event.currentTarget;

        button.disabled = true;

        button.textContent = "Deleting…";

        await deleteFolders(ids);

        closeModal();
      });
  }

  // ============================================================
  // IMPORT / EXPORT
  // ============================================================

  function openImportModal() {
    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-white"
        >
            Import Collection
        </h2>


        <p
            class="text-xs leading-5
                 text-slate-800 dark:text-slate-300"
        >
            Import is not currently provided
            by the backend collection API.
        </p>


        <div
            class="mt-4 rounded-lg
                  border border-slate-200 dark:border-slate-800
bg-slate-100 dark:bg-slate-950/60 p-3
                   text-xs  text-slate-800 dark:text-slate-300"
        >
            ${state.folders.length}
            collections are currently loaded
            from the backend.
        </div>


        <div
            class="mt-4 flex justify-end"
        >

           <button
    type="button"
    data-modal-close
    class="rounded-md
           border border-slate-300 dark:border-slate-700
           px-3 py-1.5
           text-xs
           text-slate-800 dark:text-slate-300
           hover:bg-slate-100 dark:hover:bg-slate-800
           hover:border-slate-400 dark:hover:border-slate-600"
>
    Close
</button>

        </div>

    `);
  }

  function openExportModal() {
    const selected = getSelectedFolders();

    const active = getFolder(state.activeFolderId);

    const targets = selected.length ? selected : active ? [active] : [];

    openModal(`

        <h2
            class="mb-4 text-sm font-semibold
                   text-white"
        >
            Export Collection
        </h2>


        <p
            class="text-xs leading-5
                   text-slate-800 dark:text-slate-300"
        >
            Export is not currently provided
            by the backend collection API.
        </p>


        <div
            class="my-4 rounded-lg
                   border border-slate-200 dark:border-slate-800
bg-slate-100 dark:bg-slate-950/60 p-3"
        >

            ${targets.length
        ? targets
          .map(
            (folder) => `

                                <div
                                    class="flex
                                           items-center
                                           justify-between
                                           border-b
                                           border-slate-200 dark:border-slate-800
                                           py-2 last:border-0"
                                >

                                    <span
                                        class="truncate
                                               text-xs
                                              text-slate-800 dark:text-slate-300"
                                    >
                                        ${escapeHtml(folder.name)}
                                    </span>

                                    <span
                                        class="ml-3
                                               shrink-0
                                               text-[10px]
                                               text-slate-600"
                                    >
                                        ${Array.isArray(folder.endpoints)
                ? folder.endpoints.length
                : 0
              }
                                        endpoints
                                    </span>

                                </div>

                            `,
          )
          .join("")
        : `
                        <span
                            class="text-xs
                                   text-slate-600"
                        >
                            Select a collection first.
                        </span>
                      `
      }

        </div>


        <div
            class="flex justify-end"
        >

           <button
    type="button"
    data-modal-close
    class="rounded-md
           border border-slate-300 dark:border-slate-700
           px-3 py-1.5
           text-xs
           text-slate-800 dark:text-slate-300
           hover:bg-slate-100 dark:hover:bg-slate-800
           hover:border-slate-400 dark:hover:border-slate-600"
>
    Close
</button>

        </div>

    `);
  }

  // ============================================================
  // MODAL
  // ============================================================

  function wireModal() {
    const backdrop = document.getElementById("modalBackdrop");

    if (!backdrop) return;

    backdrop.addEventListener("click", (event) => {
      if (event.target === backdrop) {
        closeModal();
      }

      if (event.target.closest("[data-modal-close]")) {
        closeModal();
      }
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

    if (!backdrop || !content) {
      return;
    }

    content.innerHTML = html;

    backdrop.classList.remove("hidden");

    backdrop.classList.add("flex");
  }

  function closeModal() {
    const backdrop = document.getElementById("modalBackdrop");

    const content = document.getElementById("modalContent");

    backdrop?.classList.add("hidden");

    backdrop?.classList.remove("flex");

    if (content) {
      content.innerHTML = "";
    }
  }

  // ============================================================
  // UTILITIES
  // ============================================================

  function calculateSize(folder) {
    /*
     * Size is not provided by the backend API.
     * Do not fabricate a value.
     */
    return null;
  }

  function capitalize(value) {
    const text = String(value || "");

    return text ? text.charAt(0).toUpperCase() + text.slice(1) : "";
  }

  function methodClass(method) {
    return (
      {
        GET: "text-emerald-400",

        POST: "text-sky-400",

        PUT: "text-amber-400",

        PATCH: "text-violet-400",

        DELETE: "text-rose-400",
      }[String(method || "").toUpperCase()] || "text-white"
    );
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

  // ============================================================
  // ICONS
  // ============================================================

  function iconClose() {
    return `

        <svg
            class="h-4 w-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
        >
            <path d="M6 6l12 12"/>
            <path d="M18 6 6 18"/>
        </svg>

    `;
  }

  function iconTrash() {
    return `

        <svg
            class="h-4 w-4"
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

  // ============================================================
  // PUBLIC API
  // ============================================================

  window.CollectionView = {
    getState: () => state,

    getFolder,

    openTestView,



    duplicateFolder,

    openRenameModal,

    openTagEditor,

    openAnnotationEditor,

    openDeleteModal,

    openMergeModal,

    openNewFolderModal,

    openDataView,

    openRepoViewModal,

    applyFilters,

    resetFilters,

    loadFolders,
  };
})();
