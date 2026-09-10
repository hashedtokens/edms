// ============================================================
// EDMS REPO VIEW
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
            'repoTableBody'
        );


    if (
        !menu ||
        !tbody ||
        !window.RepoView
    ) {

        console.error(
            'Repo View context menu could not initialize.'
        );

        return;

    }


    let targetIds = [];


    // ========================================================
    // RIGHT CLICK ON REPO ROW
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
                Number(
                    row.dataset.id
                );


            const selected =
                window.RepoView
                    .getState()
                    .selected;


            /*
             * If the clicked repo is already part of a
             * multi-selection, operate on the entire selection.
             *
             * Otherwise operate only on the clicked repo.
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
            <!-- EDIT -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'edit',
                        iconEdit(),
                        'Edit Repo View'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- ANNOTATION -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'annotation',
                        iconEdit(),
                        'Edit Annotation'
                    )
                    : ''
            }


            <!-- ============================================ -->
            <!-- DATA TAGS -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'datatags',
                        iconTags(),
                        'View Data Tags'
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
            <!-- CREATE -->
            <!-- ============================================ -->

            ${
                isSingle
                    ? menuItem(
                        'create',
                        iconPlus(),
                        'Create Repo View'
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
                    ? `Delete ${count} Repo Views`
                    : 'Delete Repo View',
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
        event => {

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
                // EDIT REPO
                // --------------------------------------------

                case 'edit':

                    if (
                        targetIds.length === 1
                    ) {

                        window.RepoView
                            .openEditRepoModal(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // ANNOTATION
                // --------------------------------------------

                case 'annotation':

                    if (
                        targetIds.length === 1
                    ) {

                        window.RepoView
                            .openAnnotationModal(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // DATA TAGS
                // --------------------------------------------

                case 'datatags':

                    if (
                        targetIds.length === 1
                    ) {

                        window.RepoView
                            .openDataTagsModal(
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

                        window.RepoView
                            .duplicateRepo(
                                targetIds[0]
                            );

                    }

                    break;


                // --------------------------------------------
                // CREATE
                // --------------------------------------------

                case 'create':

                    window.RepoView
                        .openCreateModal();

                    break;


                // --------------------------------------------
                // DELETE
                // --------------------------------------------

                case 'delete':

                    window.RepoView
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
         * The menu must already be display:block here so
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
    // ICON BASE
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


    // ========================================================
    // EDIT ICON
    // ========================================================

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


    // ========================================================
    // COPY ICON
    // ========================================================

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


    // ========================================================
    // TAGS ICON
    // ========================================================

    function iconTags() {

        return iconBase(`

            <path
                d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3.4 13.4a2 2 0 0 1 0-2.8V5a2 2 0 0 1 2-2h5.6a2 2 0 0 1 1.4.6l8.2 8.2a2 2 0 0 1 0 2.8Z"
            />

            <circle
                cx="7.5"
                cy="7.5"
                r="1.2"
            />

        `);

    }


    // ========================================================
    // PLUS ICON
    // ========================================================

    function iconPlus() {

        return iconBase(`

            <path
                d="M12 5v14"
            />

            <path
                d="M5 12h14"
            />

        `);

    }


    // ========================================================
    // TRASH ICON
    // ========================================================

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