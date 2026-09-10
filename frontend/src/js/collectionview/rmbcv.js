// ============================================================
// EDMS COLLECTION VIEW
// RIGHT-CLICK MENU
// ============================================================

(() => {
'use strict';


// ============================================================
// INIT
// ============================================================

document.addEventListener(
    'DOMContentLoaded',
    init
);


function init() {

    const menu =
        document.getElementById(
            'contextMenu'
        );

    const tbody =
        document.getElementById(
            'folderTableBody'
        );


    if (
        !menu ||
        !tbody ||
        !window.CollectionView
    ) {

        console.error(
            'Collection View context menu could not initialize.'
        );

        return;

    }


    let targetIds = [];


    // ========================================================
    // RIGHT CLICK ON FOLDER ROW
    // ========================================================

    tbody.addEventListener(
        'contextmenu',
        event => {

            const row =
                event.target.closest(
                    'tr[data-id]'
                );


            if (!row) {
                return;
            }


            event.preventDefault();


            const id =
                row.dataset.id;


            const selected =
                window.CollectionView
                    .getState()
                    .selected;


            /*
             * If the clicked folder is already inside a
             * multi-selection, operate on the whole selection.
             *
             * Otherwise only operate on the clicked folder.
             */

            if (
                selected.has(id) &&
                selected.size > 1
            ) {

                targetIds =
                    [...selected];

            } else {

                targetIds =
                    [id];

            }


            renderMenu();


            /*
             * Make the menu visible before measuring it.
             */

            menu.classList.remove(
                'hidden'
            );


            menu.style.display =
                'block';

            menu.style.visibility =
                'hidden';


            positionMenu(
                event.clientX,
                event.clientY
            );


            menu.style.visibility =
                'visible';

        }
    );


    // ========================================================
    // BUILD MENU
    // ========================================================

    function renderMenu() {

        const count =
            targetIds.length;


        const isSingle =
            count === 1;

        const isMultiple =
            count > 1;


        menu.innerHTML = `

            <!-- ============================================ -->
            <!-- DATA VIEW -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'dataview',
                        iconDatabase(),
                        'Data View'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- WEB VIEW -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'webview',
                        iconGlobe(),
                        'Web View'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- SET ACTIVE -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'active',
                        iconPin(),
                        'Set Active Folder'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- RENAME -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'rename',
                        iconEdit(),
                        'Rename'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- DUPLICATE -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'duplicate',
                        iconCopy(),
                        'Duplicate'
                    )
                    : ''
            }


            ${
                isSingle
                    ? '<div class="my-1 border-t border-slate-800"></div>'
                    : ''
            }


            <!-- ============================================ -->
            <!-- MERGE -->
            <!-- ============================================ -->

            ${
                isMultiple
                    ? menuItem(
                        'merge',
                        iconMerge(),
                        `Merge Selected (${count})`
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- NEW FOLDER -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'newfolder',
                        iconFolderPlus(),
                        'New Empty Folder'
                    )
                    : ''
            }


            ${
                isMultiple
                    ? '<div class="my-1 border-t border-slate-800"></div>'
                    : ''
            }


            <!-- ============================================ -->
            <!-- DELETE -->
            <!-- ============================================ -->

            ${menuItem(
                'delete',
                iconTrash(),
                isMultiple
                    ? `Delete ${count} Collections`
                    : 'Delete Collection',
                'text-rose-400 hover:bg-rose-500/10'
            )}

        `;

    }


    // ========================================================
    // MENU ITEM
    // ========================================================

    function menuItem(
        action,
        icon,
        label,
        extraClass = ''
    ) {

        return `

            <button
                type="button"
                data-action="${action}"
                class="
                    flex
                    w-full
                    items-center
                    gap-2.5
                    px-3
                    py-2
                    text-left
                    text-xs
                    text-slate-300
                    transition-colors
                    hover:bg-slate-800
                    hover:text-white
                    ${extraClass}
                "
            >

                ${icon}

                <span class="truncate">
                    ${label}
                </span>

            </button>

        `;

    }


    // ========================================================
    // MENU ACTIONS
    // ========================================================

    menu.addEventListener(
        'click',
        async event => {

            const button =
                event.target.closest(
                    '[data-action]'
                );


            if (!button) {
                return;
            }


            const action =
                button.dataset.action;


            if (
                targetIds.length === 0
            ) {

                closeMenu();

                return;

            }


            switch (action) {


                // --------------------------------------------
                // DATA VIEW
                // --------------------------------------------

                case 'dataview':

                    if (
                        targetIds.length === 1
                    ) {

                        window.CollectionView
                            .openDataView(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // SET ACTIVE
                // --------------------------------------------

                case 'active':

                    if (
                        targetIds.length === 1
                    ) {

                        /*
                         * Load the selected collection into
                         * the backend Active Collection workspace.
                         *
                         * This does NOT navigate to Bookmark View.
                         */
                        await window.CollectionView
                            .loadCollectionIntoActive(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // RENAME
                // --------------------------------------------

                case 'rename':

                    if (
                        targetIds.length === 1
                    ) {

                        window.CollectionView
                            .openRenameModal(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // DUPLICATE
                // --------------------------------------------

                case 'duplicate':

                    if (
                        targetIds.length === 1
                    ) {

                        window.CollectionView
                            .duplicateFolder(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // MERGE
                // --------------------------------------------

                case 'merge':

                    if (
                        targetIds.length >= 2
                    ) {

                        window.CollectionView
                            .openMergeModal(
                                targetIds
                            );

                    }

                    break;


                // --------------------------------------------
                // NEW FOLDER
                // --------------------------------------------

                case 'newfolder':

                    window.CollectionView
                        .openNewFolderModal();

                    break;


                // --------------------------------------------
                // DELETE
                // --------------------------------------------

                case 'delete':

                    window.CollectionView
                        .openDeleteModal(
                            targetIds
                        );

                    break;

            }


            closeMenu();

        }
    );


    // ========================================================
    // CLOSE ON NORMAL CLICK
    // ========================================================

    document.addEventListener(
        'click',
        event => {

            if (
                !menu.contains(
                    event.target
                )
            ) {

                closeMenu();

            }

        }
    );


    // ========================================================
    // CLOSE ON ESCAPE
    // ========================================================

    document.addEventListener(
        'keydown',
        event => {

            if (
                event.key === 'Escape'
            ) {

                closeMenu();

            }

        }
    );


    // ========================================================
    // CLOSE ON RESIZE
    // ========================================================

    window.addEventListener(
        'resize',
        closeMenu
    );


    // ========================================================
    // CLOSE ON SCROLL
    // ========================================================

    window.addEventListener(
        'scroll',
        closeMenu,
        true
    );


    // ========================================================
    // CLOSE MENU
    // ========================================================

    function closeMenu() {

        menu.classList.add(
            'hidden'
        );


        menu.style.display =
            'none';

        menu.style.visibility =
            'hidden';

    }


    // ========================================================
    // POSITION MENU
    // ========================================================

    function positionMenu(
        x,
        y
    ) {

        /*
         * Menu must already be display:block here so that
         * offsetWidth / offsetHeight are available.
         */

        const width =
            menu.offsetWidth || 224;

        const height =
            menu.offsetHeight || 240;

        const margin =
            8;


        let left =
            x;

        let top =
            y;


        // --------------------------------------------
        // Horizontal boundary
        // --------------------------------------------

        if (
            left + width >
            window.innerWidth - margin
        ) {

            left =
                window.innerWidth -
                width -
                margin;

        }


        // --------------------------------------------
        // Vertical boundary
        // --------------------------------------------

        if (
            top + height >
            window.innerHeight - margin
        ) {

            top =
                window.innerHeight -
                height -
                margin;

        }


        left =
            Math.max(
                margin,
                left
            );


        top =
            Math.max(
                margin,
                top
            );


        menu.style.left =
            `${left}px`;

        menu.style.top =
            `${top}px`;

    }


    // ========================================================
    // ICONS
    // ========================================================

    function iconBase(
        content
    ) {

        return `

            <svg
                class="h-4 w-4 shrink-0"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.7"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
            >
                ${content}
            </svg>

        `;

    }


    function iconDatabase() {

        return iconBase(`

            <ellipse
                cx="12"
                cy="5"
                rx="7"
                ry="3"
            />

            <path
                d="M5 5v7c0 1.7 3.1 3 7 3s7-1.3 7-3V5"
            />

            <path
                d="M5 12v7c0 1.7 3.1 3 7 3s7-1.3 7-3v-7"
            />

        `);

    }


    function iconPin() {

        return iconBase(`

            <path
                d="M12 17v5"
            />

            <path
                d="M7 4h10"
            />

            <path
                d="M8 4v6l-3 4h14l-3-4V4"
            />

        `);

    }


    function iconEdit() {

        return iconBase(`

            <path
                d="M12 20h9"
            />

            <path
                d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4Z"
            />

        `);

    }


    function iconCopy() {

        return iconBase(`

            <rect
                x="9"
                y="9"
                width="11"
                height="11"
                rx="2"
            />

            <path
                d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
            />

        `);

    }


    function iconGlobe() {

        return iconBase(`

            <circle
                cx="12"
                cy="12"
                r="9"
            />

            <path
                d="M3 12h18"
            />

            <path
                d="M12 3c2.5 2.5 3.5 5.5 3.5 9s-1 6.5-3.5 9"
            />

            <path
                d="M12 3c-2.5 2.5-3.5 5.5-3.5 9s1 6.5 3.5 9"
            />

        `);

    }


    function iconMerge() {

        return iconBase(`

            <path
                d="M6 4v5"
            />

            <path
                d="M18 4v5"
            />

            <path
                d="M6 9c0 4 3 6 6 6"
            />

            <path
                d="M18 9c0 4-3 6-6 6"
            />

            <path
                d="M12 15v5"
            />

        `);

    }


    function iconFolderPlus() {

        return iconBase(`

            <path
                d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z"
            />

            <path
                d="M12 11v6"
            />

            <path
                d="M9 14h6"
            />

        `);

    }


    function iconTrash() {

        return iconBase(`

            <path
                d="M4 7h16"
            />

            <path
                d="M10 11v6"
            />

            <path
                d="M14 11v6"
            />

            <path
                d="M6 7l1 13h10l1-13"
            />

            <path
                d="M9 7V4h6v3"
            />

        `);

    }

}

})();