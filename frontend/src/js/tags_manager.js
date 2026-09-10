// ============================================================
// EDMS UNIVERSAL TAG MANAGER
// ============================================================
//
// This module is intentionally independent from List View.
//
// Each view initializes it with an adapter:
//
// TagManager.init({
//     view: "list",
//     getItems: () => endpoints,
//     getTags: item => item.tags,
//     setTags: (item, tags) => {
//         item.tags = tags;
//     },
//     onChange: () => {
//         renderTags();
//         updateStats();
//         applyFilters();
//     }
// });
//
// Supported operations:
// - View all tags
// - Select tags with checkboxes
// - Rename tag
// - Merge multiple tags
// - Delete tag
//
// ============================================================


const TagManager = (() => {

    // ========================================================
    // STATE
    // ========================================================

    let config = null;

    let selectedTags = new Set();

    let currentTags = [];

    let managerOpen = false;


    // ========================================================
    // DEFAULT CONFIG
    // ========================================================

    const defaultConfig = {

        view: "default",

        getItems: () => [],

        getTags: item => {

            return Array.isArray(item?.tags)
                ? item.tags
                : [];

        },

        setTags: (item, tags) => {

            item.tags = tags;

        },

        onChange: null,

        title: "Tag Manager"

    };


    // ========================================================
    // INITIALIZE
    // ========================================================

    function init(options = {}) {

        config = {

            ...defaultConfig,

            ...options

        };


        if (
            typeof config.getItems !== "function"
        ) {

            throw new Error(
                "TagManager: getItems must be a function."
            );

        }


        if (
            typeof config.getTags !== "function"
        ) {

            throw new Error(
                "TagManager: getTags must be a function."
            );

        }


        if (
            typeof config.setTags !== "function"
        ) {

            throw new Error(
                "TagManager: setTags must be a function."
            );

        }


        setupButton();


        return TagManager;

    }


    // ========================================================
    // SETUP MANAGER BUTTON
    // ========================================================

    function setupButton() {

        const button =
            document.getElementById(
                "tagsManager"
            );


        if (!button) {

            return;

        }


        // Prevent duplicate listeners when a view
        // is initialized more than once.

        if (
            button.dataset.tagManagerBound === "true"
        ) {

            return;

        }


        button.dataset.tagManagerBound =
            "true";


        button.addEventListener(
            "click",
            open
        );

    }


    // ========================================================
    // GET ITEMS
    // ========================================================

    function getItems() {

        if (!config) {

            return [];

        }


        try {

            const items =
                config.getItems();


            return Array.isArray(items)
                ? items
                : [];

        } catch (error) {

            console.error(
                "TagManager: failed to get items.",
                error
            );


            return [];

        }

    }


    // ========================================================
    // GET ALL TAGS
    // ========================================================

    function getAllTags() {

        const tagSet =
            new Set();


        getItems().forEach(
            item => {

                let tags = [];


                try {

                    tags =
                        config.getTags(item);

                } catch (error) {

                    console.error(
                        "TagManager: failed to read tags.",
                        error
                    );

                    return;

                }


                if (
                    !Array.isArray(tags)
                ) {

                    return;

                }


                tags.forEach(
                    tag => {

                        const normalized =
                            normalizeTag(tag);


                        if (normalized) {

                            tagSet.add(
                                normalized
                            );

                        }

                    }
                );

            }
        );


        return [...tagSet].sort(
            (a, b) =>
                a.localeCompare(
                    b,
                    undefined,
                    {
                        sensitivity: "base"
                    }
                )
        );

    }


    // ========================================================
    // GET TAG COUNTS
    // ========================================================

    function getTagCounts() {

        const counts = {};


        getItems().forEach(
            item => {

                let tags = [];


                try {

                    tags =
                        config.getTags(item);

                } catch {

                    return;

                }


                if (
                    !Array.isArray(tags)
                ) {

                    return;

                }


                const uniqueTags =
                    new Set();


                tags.forEach(
                    tag => {

                        const normalized =
                            normalizeTag(tag);


                        if (
                            normalized &&
                            !uniqueTags.has(
                                normalized
                            )
                        ) {

                            uniqueTags.add(
                                normalized
                            );


                            counts[normalized] =
                                (
                                    counts[normalized] ||
                                    0
                                ) + 1;

                        }

                    }
                );

            }
        );


        return counts;

    }


    // ========================================================
    // OPEN
    // ========================================================

    function open() {

        if (!config) {

            console.error(
                "TagManager: initialize the manager first."
            );

            return;

        }


        managerOpen = true;

        selectedTags.clear();

        refreshTags();


        renderManager();

    }


    // ========================================================
    // CLOSE
    // ========================================================

    function close() {

        managerOpen = false;

        selectedTags.clear();


        if (
            typeof window.closeModal ===
            "function"
        ) {

            window.closeModal();

        }

    }


    // ========================================================
    // REFRESH
    // ========================================================

    function refreshTags() {

        currentTags =
            getAllTags();


        // Remove selections that no longer exist.

        selectedTags =
            new Set(
                [...selectedTags].filter(
                    tag =>
                        currentTags.includes(
                            tag
                        )
                )
            );

    }


    // ========================================================
    // RENDER MANAGER
    // ========================================================

    function renderManager() {

        const counts =
            getTagCounts();


        const content = `

            <div
                class="space-y-4 "
            >

                <!-- =========================================
                     HEADER / INFO
                ========================================== -->

                <div
                    class="flex items-center
                           justify-between "
                >

                    <div>

                        <p
                            class="text-xs
                                   text-slate-400"
                        >
                            ${currentTags.length}
                            tag${currentTags.length === 1 ? "" : "s"}
                        </p>

                        <p
                            class="mt-0.5
                                   text-[10px]
                                   text-slate-600"
                        >
                            ${getItems().length}
                            items in this view
                        </p>

                    </div>


                    <button
                        type="button"
                        id="tmRefresh"
                        class="rounded-md
                               border
                               border-slate-700
                               bg-slate-800
                               px-2.5 py-1.5
                               text-[11px]
                               text-slate-400
                               transition
                               hover:bg-slate-700
                               hover:text-white"
                    >
                        Refresh
                    </button>

                </div>


                <!-- =========================================
                     SELECTED ACTION BAR
                ========================================== -->

                <div
                    id="tmActionBar"
                    class="
                        hidden
                        rounded-lg
                        border
                        border-cyan-500/20
                        bg-cyan-500/5
                        p-3
                    "
                >

                    <div
                        class="flex items-center
                               justify-between
                               gap-3"
                    >

                        <span
                            id="tmSelectedCount"
                            class="text-xs
                                   font-medium
                                   text-cyan-300"
                        >
                            0 selected
                        </span>


                        <button
                            type="button"
                            id="tmClearSelection"
                            class="text-[11px]
                                   text-slate-500
                                   hover:text-white"
                        >
                            Clear
                        </button>

                    </div>


                    <div
                        class="mt-3
                               flex flex-wrap
                               gap-2"
                    >

                        <button
                            type="button"
                            id="tmRename"
                            class="
                                rounded-md
                                border
                                border-slate-700
                                bg-slate-800
                                px-2.5 py-1.5
                                text-[11px]
                                text-slate-300
                                transition
                                hover:bg-slate-700
                            "
                        >
                            Rename
                        </button>


                        <button
                            type="button"
                            id="tmMerge"
                            class="
                                rounded-md
                                border
                                border-slate-700
                                bg-slate-800
                                px-2.5 py-1.5
                                text-[11px]
                                text-slate-300
                                transition
                                hover:bg-slate-700
                            "
                        >
                            Merge
                        </button>


                        <button
                            type="button"
                            id="tmDelete"
                            class="
                                rounded-md
                                border
                                border-rose-500/20
                                bg-rose-500/10
                                px-2.5 py-1.5
                                text-[11px]
                                text-rose-400
                                transition
                                hover:bg-rose-500/15
                            "
                        >
                            Delete
                        </button>

                    </div>

                </div>


                <!-- =========================================
                     TAG LIST
                ========================================== -->

                <div>

                    <div
                        class="mb-2
                               flex items-center
                               justify-between"
                    >

                        <h4
                            class="text-[11px]
                                   font-medium
                                   text-slate-500"
                        >
                            Tags
                        </h4>


                        <span
                            class="text-[10px]
                                   text-slate-600"
                        >
                            Select tags to manage
                        </span>

                    </div>


                    <div
                        id="tmTagList"
                        class="
                            max-h-[45vh]
                            overflow-y-auto
                            
                            rounded-lg
                            border
                            border-slate-800
                        "
                    >

                        ${renderTagList(counts)}

                    </div>

                </div>


                <!-- =========================================
                     FOOTER
                ========================================== -->

                <div
                    class="flex justify-end
                           border-t
                           border-slate-800
                           pt-3"
                >

                    <button
                        type="button"
                        data-modal-close
                        class="
                            rounded-md
                            border
                            border-slate-700
                            px-3 py-1.5
                            text-xs
                            text-slate-400
                            hover:bg-slate-800
                        "
                    >
                        Close
                    </button>

                </div>

            </div>

        `;


        if (
            typeof window.openModal ===
            "function"
        ) {

            window.openModal(
                `${config.title} — ${capitalize(config.view)}`,
                content
            );

        } else {

            fallbackModal(
                content
            );

        }


        bindManagerEvents();

        updateActionBar();

    }


    // ========================================================
    // RENDER TAG LIST
    // ========================================================

    function renderTagList(counts) {

        if (
            currentTags.length === 0
        ) {

            return `

                <div
                    class="px-4 py-8
                           text-center"
                >

                    <p
                        class="text-xs
                               text-slate-500"
                    >
                        No tags found
                    </p>

                    <p
                        class="mt-1
                               text-[10px]
                               text-slate-700"
                    >
                        Tags will appear here when
                        items contain tags.
                    </p>

                </div>

            `;

        }


        return currentTags
            .map(
                tag => {

                    const checked =
                        selectedTags.has(
                            tag
                        );


                    const count =
                        counts[tag] || 0;


                    return `

                        <label
                            class="
                                flex
                                cursor-pointer
                                items-center
                                gap-3
                                border-b
                                border-slate-800
                                px-3 py-2
                                last:border-0
                                transition
                                hover:bg-slate-800/50
                            "
                        >

                            <input
                                type="checkbox"
                                class="
                                    tm-tag-checkbox
                                    h-3.5 w-3.5
                                    accent-cyan-400
                                "
                                data-tag="${escapeAttr(tag)}"
                                ${checked ? "checked" : ""}
                            >


                            <span
                                class="
                                    min-w-0
                                    flex-1
                                    truncate
                                    text-xs
                                    text-slate-300
                                "
                            >
                                ${escape(tag)}
                            </span>


                            <span
                                class="
                                    shrink-0
                                    rounded-md
                                    bg-slate-800
                                    px-1.5 py-0.5
                                    text-[10px]
                                    text-slate-500
                                "
                            >
                                ${count}
                            </span>

                        </label>

                    `;

                }
            )
            .join("");

    }


    // ========================================================
    // BIND MANAGER EVENTS
    // ========================================================

    function bindManagerEvents() {

        // ----------------------------------------------------
        // Refresh
        // ----------------------------------------------------

        document
            .getElementById(
                "tmRefresh"
            )
            ?.addEventListener(
                "click",
                () => {

                    refreshTags();

                    renderManager();

                }
            );


        // ----------------------------------------------------
        // Checkboxes
        // ----------------------------------------------------

        document
            .querySelectorAll(
                ".tm-tag-checkbox"
            )
            .forEach(
                checkbox => {

                    checkbox.addEventListener(
                        "change",
                        () => {

                            const tag =
                                checkbox.dataset.tag;


                            if (
                                checkbox.checked
                            ) {

                                selectedTags.add(
                                    tag
                                );

                            } else {

                                selectedTags.delete(
                                    tag
                                );

                            }


                            updateActionBar();

                        }
                    );

                }
            );


        // ----------------------------------------------------
        // Clear
        // ----------------------------------------------------

        document
            .getElementById(
                "tmClearSelection"
            )
            ?.addEventListener(
                "click",
                () => {

                    selectedTags.clear();

                    document
                        .querySelectorAll(
                            ".tm-tag-checkbox"
                        )
                        .forEach(
                            checkbox => {

                                checkbox.checked =
                                    false;

                            }
                        );


                    updateActionBar();

                }
            );


        // ----------------------------------------------------
        // Rename
        // ----------------------------------------------------

        document
            .getElementById(
                "tmRename"
            )
            ?.addEventListener(
                "click",
                openRenameModal
            );


        // ----------------------------------------------------
        // Merge
        // ----------------------------------------------------

        document
            .getElementById(
                "tmMerge"
            )
            ?.addEventListener(
                "click",
                openMergeModal
            );


        // ----------------------------------------------------
        // Delete
        // ----------------------------------------------------

        document
            .getElementById(
                "tmDelete"
            )
            ?.addEventListener(
                "click",
                openDeleteModal
            );

    }


    // ========================================================
    // UPDATE ACTION BAR
    // ========================================================

    function updateActionBar() {

        const bar =
            document.getElementById(
                "tmActionBar"
            );


        const count =
            document.getElementById(
                "tmSelectedCount"
            );


        const selectedCount =
            selectedTags.size;


        if (selectedCount > 0) {

            bar?.classList.remove(
                "hidden"
            );

        } else {

            bar?.classList.add(
                "hidden"
            );

        }


        if (count) {

            count.textContent =
                `${selectedCount} selected`;

        }


        const renameButton =
            document.getElementById(
                "tmRename"
            );


        const mergeButton =
            document.getElementById(
                "tmMerge"
            );


        const deleteButton =
            document.getElementById(
                "tmDelete"
            );


        if (renameButton) {

            renameButton.disabled =
                selectedCount !== 1;

            updateDisabledStyle(
                renameButton
            );

        }


        if (mergeButton) {

            mergeButton.disabled =
                selectedCount < 2;

            updateDisabledStyle(
                mergeButton
            );

        }


        if (deleteButton) {

            deleteButton.disabled =
                selectedCount === 0;

            updateDisabledStyle(
                deleteButton
            );

        }

    }


    // ========================================================
    // RENAME TAG
    // ========================================================

    function openRenameModal() {

        if (
            selectedTags.size !== 1
        ) {

            return;

        }


        const oldTag =
            [...selectedTags][0];


        const content = `

            <div
                class="space-y-4"
            >

                <div>

                    <label
                        class="mb-1 block
                               text-xs
                               text-slate-500"
                    >
                        New tag name
                    </label>


                    <input
                        id="tmRenameInput"
                        value="${escapeAttr(oldTag)}"
                        class="
                            h-9 w-full
                            rounded-md
                            border
                            border-slate-700
                            bg-slate-950
                            px-3
                            text-xs
                            text-slate-300
                            outline-none
                            focus:border-cyan-500
                        "
                    >

                </div>


                <div
                    class="
                        rounded-lg
                        border
                        border-slate-800
                        bg-slate-950/40
                        p-3
                    "
                >

                    <p
                        class="text-[11px]
                               text-slate-500"
                    >
                        This will rename
                        <span
                            class="font-medium
                                   text-slate-300"
                        >
                            ${escape(oldTag)}
                        </span>
                        everywhere in the current view.
                    </p>

                </div>


                <div
                    class="flex justify-end
                           gap-2"
                >

                    <button
                        type="button"
                        data-modal-close
                        class="
                            rounded-md
                            border
                            border-slate-700
                            px-3 py-1.5
                            text-xs
                            text-slate-400
                            hover:bg-slate-800
                        "
                    >
                        Cancel
                    </button>


                    <button
                        type="button"
                        id="tmConfirmRename"
                        class="
                            rounded-md
                            bg-cyan-600
                            px-3 py-1.5
                            text-xs
                            font-medium
                            text-white
                            hover:bg-cyan-500
                        "
                    >
                        Rename
                    </button>

                </div>

            </div>

        `;


        openInternalModal(
            "Rename Tag",
            content
        );


        document
            .getElementById(
                "tmConfirmRename"
            )
            ?.addEventListener(
                "click",
                () => {

                    const input =
                        document.getElementById(
                            "tmRenameInput"
                        );


                    const newName =
                        input?.value.trim();


                    if (!newName) {

                        notify(
                            "Enter a tag name.",
                            "error"
                        );

                        return;

                    }


                    renameTag(
                        oldTag,
                        newName
                    );

                }
            );

    }


    // ========================================================
    // RENAME IMPLEMENTATION
    // ========================================================

    function renameTag(
        oldTag,
        newTag
    ) {

        const normalizedNewTag =
            normalizeTag(newTag);


        if (!normalizedNewTag) {

            return;

        }


        const existingTag =
            currentTags.find(
                tag =>
                    tag.toLowerCase() ===
                    normalizedNewTag.toLowerCase()
            );


        if (
            existingTag &&
            existingTag !== oldTag
        ) {

            notify(
                `"${normalizedNewTag}" already exists. Use Merge instead.`,
                "error"
            );

            return;

        }


        let changed = 0;


        getItems().forEach(
            item => {

                let tags;


                try {

                    tags =
                        config.getTags(item);

                } catch {

                    return;

                }


                if (
                    !Array.isArray(tags)
                ) {

                    return;

                }


                let modified =
                    false;


                const newTags =
                    tags.map(
                        tag => {

                            if (
                                normalizeTag(tag) ===
                                oldTag
                            ) {

                                modified = true;

                                changed++;

                                return normalizedNewTag;

                            }


                            return tag;

                        }
                    );


                if (modified) {

                    config.setTags(
                        item,
                        uniqueTags(
                            newTags
                        )
                    );

                }

            }
        );


        selectedTags.clear();


        closeInternalModal();


        notify(
            changed === 0
                ? "No items were changed."
                : `"${oldTag}" renamed to "${normalizedNewTag}".`,
            changed === 0
                ? "info"
                : "success"
        );


        notifyChange();


        if (managerOpen) {

            refreshTags();

            renderManager();

        }

    }


    // ========================================================
    // MERGE TAGS
    // ========================================================

    function openMergeModal() {

        if (
            selectedTags.size < 2
        ) {

            return;

        }


        const tags =
            [...selectedTags];


        const tagList =
            tags
                .map(
                    tag => `

                        <span
                            class="
                                rounded-md
                                bg-slate-800
                                px-2 py-1
                                text-[10px]
                                text-slate-300
                            "
                        >
                            ${escape(tag)}
                        </span>

                    `
                )
                .join("");


        const content = `

            <div
                class="space-y-4"
            >

                <div>

                    <p
                        class="mb-2
                               text-xs
                               text-slate-500"
                    >
                        Tags to merge
                    </p>


                    <div
                        class="flex
                               flex-wrap
                               gap-1.5"
                    >
                        ${tagList}
                    </div>

                </div>


                <div>

                    <label
                        class="mb-1 block
                               text-xs
                               text-slate-500"
                    >
                        New tag name
                    </label>


                    <input
                        id="tmMergeInput"
                        class="
                            h-9 w-full
                            rounded-md
                            border
                            border-slate-700
                            bg-slate-950
                            px-3
                            text-xs
                            text-slate-300
                            outline-none
                            focus:border-cyan-500
                        "
                        placeholder="Enter merged tag name"
                    >

                </div>


                <div
                    class="
                        rounded-lg
                        border
                        border-amber-500/20
                        bg-amber-500/5
                        p-3
                    "
                >

                    <p
                        class="text-[11px]
                               text-amber-300"
                    >
                        All selected tags will be replaced
                        by the new tag across the current
                        view.
                    </p>

                </div>


                <div
                    class="flex justify-end
                           gap-2"
                >

                    <button
                        type="button"
                        data-modal-close
                        class="
                            rounded-md
                            border
                            border-slate-700
                            px-3 py-1.5
                            text-xs
                            text-slate-400
                            hover:bg-slate-800
                        "
                    >
                        Cancel
                    </button>


                    <button
                        type="button"
                        id="tmConfirmMerge"
                        class="
                            rounded-md
                            bg-cyan-600
                            px-3 py-1.5
                            text-xs
                            font-medium
                            text-white
                            hover:bg-cyan-500
                        "
                    >
                        Merge
                    </button>

                </div>

            </div>

        `;


        openInternalModal(
            "Merge Tags",
            content
        );


        document
            .getElementById(
                "tmConfirmMerge"
            )
            ?.addEventListener(
                "click",
                () => {

                    const input =
                        document.getElementById(
                            "tmMergeInput"
                        );


                    const newName =
                        input?.value.trim();


                    if (!newName) {

                        notify(
                            "Enter a tag name.",
                            "error"
                        );

                        return;

                    }


                    mergeTags(
                        tags,
                        newName
                    );

                }
            );

    }


    // ========================================================
    // MERGE IMPLEMENTATION
    // ========================================================

    function mergeTags(
        tagsToMerge,
        newTag
    ) {

        const normalizedNewTag =
            normalizeTag(newTag);


        if (!normalizedNewTag) {

            return;

        }


        let changed = 0;


        getItems().forEach(
            item => {

                let tags;


                try {

                    tags =
                        config.getTags(item);

                } catch {

                    return;

                }


                if (
                    !Array.isArray(tags)
                ) {

                    return;

                }


                let modified =
                    false;


                const newTags = [];


                tags.forEach(
                    tag => {

                        const normalized =
                            normalizeTag(tag);


                        if (
                            tagsToMerge.some(
                                selected =>
                                    selected.toLowerCase() ===
                                    normalized.toLowerCase()
                            )
                        ) {

                            modified = true;

                            changed++;

                            return;

                        }


                        newTags.push(tag);

                    }
                );


                if (modified) {

                    newTags.push(
                        normalizedNewTag
                    );


                    config.setTags(
                        item,
                        uniqueTags(
                            newTags
                        )
                    );

                }

            }
        );


        selectedTags.clear();


        closeInternalModal();


        notify(
            changed === 0
                ? "No items were changed."
                : `Merged ${tagsToMerge.length} tags into "${normalizedNewTag}".`,
            changed === 0
                ? "info"
                : "success"
        );


        notifyChange();


        if (managerOpen) {

            refreshTags();

            renderManager();

        }

    }


    // ========================================================
    // DELETE TAG
    // ========================================================

    function openDeleteModal() {

        if (
            selectedTags.size === 0
        ) {

            return;

        }


        const tags =
            [...selectedTags];


        const tagList =
            tags
                .map(
                    tag => `

                        <span
                            class="
                                rounded-md
                                bg-slate-800
                                px-2 py-1
                                text-[10px]
                                text-slate-300
                            "
                        >
                            ${escape(tag)}
                        </span>

                    `
                )
                .join("");


        const content = `

            <div
                class="space-y-4"
            >

                <div>

                    <p
                        class="mb-2
                               text-xs
                               text-slate-500"
                    >
                        Tags to delete
                    </p>


                    <div
                        class="flex flex-wrap
                               gap-1.5"
                    >
                        ${tagList}
                    </div>

                </div>


                <div
                    class="
                        rounded-lg
                        border
                        border-rose-500/20
                        bg-rose-500/5
                        p-3
                    "
                >

                    <p
                        class="text-[11px]
                               text-rose-300"
                    >
                        These tags will be removed
                        from every item in the current
                        view.
                    </p>

                </div>


                <div
                    class="flex justify-end
                           gap-2"
                >

                    <button
                        type="button"
                        data-modal-close
                        class="
                            rounded-md
                            border
                            border-slate-700
                            px-3 py-1.5
                            text-xs
                            text-slate-400
                            hover:bg-slate-800
                        "
                    >
                        Cancel
                    </button>


                    <button
                        type="button"
                        id="tmConfirmDelete"
                        class="
                            rounded-md
                            bg-rose-600
                            px-3 py-1.5
                            text-xs
                            font-medium
                            text-white
                            hover:bg-rose-500
                        "
                    >
                        Delete
                    </button>

                </div>

            </div>

        `;


        openInternalModal(
            "Delete Tags",
            content
        );


        document
            .getElementById(
                "tmConfirmDelete"
            )
            ?.addEventListener(
                "click",
                () => {

                    deleteTags(
                        tags
                    );

                }
            );

    }


    // ========================================================
    // DELETE IMPLEMENTATION
    // ========================================================

    function deleteTags(
        tagsToDelete
    ) {

        let changed = 0;


        getItems().forEach(
            item => {

                let tags;


                try {

                    tags =
                        config.getTags(item);

                } catch {

                    return;

                }


                if (
                    !Array.isArray(tags)
                ) {

                    return;

                }


                const newTags =
                    tags.filter(
                        tag => {

                            const normalized =
                                normalizeTag(tag);


                            const shouldDelete =
                                tagsToDelete.some(
                                    selected =>
                                        selected.toLowerCase() ===
                                        normalized.toLowerCase()
                                );


                            if (
                                shouldDelete
                            ) {

                                changed++;

                            }


                            return !shouldDelete;

                        }
                    );


                if (
                    newTags.length !==
                    tags.length
                ) {

                    config.setTags(
                        item,
                        uniqueTags(
                            newTags
                        )
                    );

                }

            }
        );


        selectedTags.clear();


        closeInternalModal();


        notify(
            changed === 0
                ? "No items were changed."
                : `${tagsToDelete.length} tag${tagsToDelete.length === 1 ? "" : "s"} deleted.`,
            changed === 0
                ? "info"
                : "success"
        );


        notifyChange();


        if (managerOpen) {

            refreshTags();

            renderManager();

        }

    }


    // ========================================================
    // NOTIFY VIEW
    // ========================================================

    function notifyChange() {

        if (
            typeof config?.onChange ===
            "function"
        ) {

            try {

                config.onChange();

            } catch (error) {

                console.error(
                    "TagManager: onChange failed.",
                    error
                );

            }

        }

    }


    // ========================================================
    // INTERNAL MODAL
    // ========================================================

    function openInternalModal(
        title,
        content
    ) {

        if (
            typeof window.openModal ===
            "function"
        ) {

            window.openModal(
                title,
                content
            );

            return;

        }


        fallbackModal(
            content
        );

    }


    // ========================================================
    // CLOSE INTERNAL MODAL
    // ========================================================

    function closeInternalModal() {

        if (
            typeof window.closeModal ===
            "function"
        ) {

            window.closeModal();

        }

    }


    // ========================================================
    // FALLBACK MODAL
    // ========================================================

    function fallbackModal(
        content
    ) {

        const existing =
            document.getElementById(
                "tagManagerFallback"
            );


        existing?.remove();


        const wrapper =
            document.createElement(
                "div"
            );


        wrapper.id =
            "tagManagerFallback";


        wrapper.className = `
            fixed inset-0 z-[100]
            flex items-center justify-center
            bg-slate-950/70 p-4
            backdrop-blur-sm
        `;


        wrapper.innerHTML = `

            <div
                class="
                    w-full max-w-lg
                    overflow-hidden
                    rounded-xl
                    border
                    border-slate-700
                    bg-slate-900
                    shadow-2xl
                "
            >

                <div
                    class="
                        flex items-center
                        justify-between
                        border-b
                        border-slate-800
                        px-4 py-3
                    "
                >

                    <h3
                        class="
                            text-sm
                            font-semibold
                            text-white
                        "
                    >
                        ${escape(
                            config?.title ||
                            "Tag Manager"
                        )}
                    </h3>


                    <button
                        type="button"
                        id="tmFallbackClose"
                        class="
                            rounded-md
                            p-1
                            text-slate-500
                            hover:bg-slate-800
                            hover:text-white
                        "
                    >
                        ×
                    </button>

                </div>


                <div
                    class="
                        max-h-[75vh]
                        overflow-y-auto
                        p-4
                        
                    "
                >
                    ${content}
                </div>

            </div>

        `;


        document.body.appendChild(
            wrapper
        );


        wrapper
            .querySelector(
                "#tmFallbackClose"
            )
            ?.addEventListener(
                "click",
                () => {

                    wrapper.remove();

                }
            );


        wrapper.addEventListener(
            "click",
            event => {

                if (
                    event.target === wrapper
                ) {

                    wrapper.remove();

                }

            }
        );

    }


    // ========================================================
    // DISABLED STYLE
    // ========================================================

    function updateDisabledStyle(
        button
    ) {

        if (
            button.disabled
        ) {

            button.classList.add(
                "cursor-not-allowed",
                "opacity-30"
            );

        } else {

            button.classList.remove(
                "cursor-not-allowed",
                "opacity-30"
            );

        }

    }


    // ========================================================
    // NORMALIZE TAG
    // ========================================================

    function normalizeTag(
        tag
    ) {

        return String(
            tag ?? ""
        ).trim();

    }


    // ========================================================
    // UNIQUE TAGS
    // ========================================================

    function uniqueTags(
        tags
    ) {

        const seen =
            new Set();


        const result = [];


        tags.forEach(
            tag => {

                const normalized =
                    normalizeTag(tag);


                if (!normalized) {

                    return;

                }


                const key =
                    normalized.toLowerCase();


                if (
                    seen.has(key)
                ) {

                    return;

                }


                seen.add(key);

                result.push(
                    normalized
                );

            }
        );


        return result;

    }


    // ========================================================
    // CAPITALIZE
    // ========================================================

    function capitalize(
        value
    ) {

        const text =
            String(
                value || ""
            );


        return text
            ? text.charAt(0).toUpperCase() +
              text.slice(1)
            : "";

    }


    // ========================================================
    // ESCAPE HTML
    // ========================================================

    function escape(
        value
    ) {

        if (
            typeof window.escapeHTML ===
            "function"
        ) {

            return window.escapeHTML(
                value
            );

        }


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


    // ========================================================
    // ESCAPE ATTRIBUTE
    // ========================================================

    function escapeAttr(
        value
    ) {

        if (
            typeof window.escapeAttribute ===
            "function"
        ) {

            return window.escapeAttribute(
                value
            );

        }


        return escape(
            value
        );

    }


    // ========================================================
    // TOAST
    // ========================================================

    function notify(
        message,
        type = "info"
    ) {

        if (
            typeof window.showToast ===
            "function"
        ) {

            window.showToast(
                message,
                type
            );

            return;

        }


        console.log(
            `[TagManager] ${message}`
        );

    }


    // ========================================================
    // PUBLIC API
    // ========================================================

    return {

        init,

        open,

        close,

        refresh: () => {

            refreshTags();

        },

        getTags: () => {

            return [
                ...getAllTags()
            ];

        },

        getSelectedTags: () => {

            return [
                ...selectedTags
            ];

        },

        rename: renameTag,

        merge: mergeTags,

        delete: deleteTags

    };

})();