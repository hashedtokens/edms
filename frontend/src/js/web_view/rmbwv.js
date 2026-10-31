// ============================================================
// EDMS WEB VIEW
// RIGHT-CLICK MENU (RMBWV)
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
        const menu = document.getElementById('contextMenu');
        const tbody = document.getElementById('webTableBody');

        if (!menu || !tbody) {
            return;
        }

        let targetIds = [];

        // ========================================================
        // RIGHT CLICK ON WEB VIEW ROW
        // ========================================================

        tbody.addEventListener('contextmenu', event => {
            const row = event.target.closest('tr[data-id]');
            if (!row) return;

            event.preventDefault();

            const id = row.dataset.id;
            const selected = window.WebView?.getState()?.selected || new Set();

            if (selected.has(id) && selected.size > 1) {
                targetIds = [...selected];
            } else {
                targetIds = [id];
            }

            renderMenu();

            menu.classList.remove('hidden');
            menu.style.display = 'block';
            menu.style.visibility = 'hidden';

            positionMenu(event.clientX, event.clientY);

            menu.style.visibility = 'visible';
        });

        // ========================================================
        // BUILD MENU
        // ========================================================

        function renderMenu() {
            const count = targetIds.length;
            const isSingle = count === 1;
            const isMultiple = count > 1;

            menu.innerHTML = `
                <!-- EDIT FRONT PAGE -->
                ${isSingle
                    ? menuItem('frontpage', iconEdit(), 'Edit Front Page')
                    : ''
                }

                <!-- RENAME -->
                ${isSingle
                    ? menuItem('edit', iconEdit(), 'Rename')
                    : ''
                }

                <!-- ANNOTATION -->
                ${isSingle
                    ? menuItem('annotation', iconFileText(), 'Edit Annotation')
                    : ''
                }

                <!-- DATA TAGS -->
                ${isSingle
                    ? menuItem('datatags', iconTags(), 'View EQP Tags')
                    : ''
                }

                <!-- DUPLICATE -->
                ${isSingle
                    ? menuItem('duplicate', iconCopy(), 'Duplicate')
                    : ''
                }

                <!-- CONVERT TO COLLECTION -->
                ${isSingle
                    ? menuItem('convert', iconFolderSwitch(), 'Convert to Collection')
                    : ''
                }

                <!-- TAKEOUT -->
                ${isSingle
                    ? menuItem('takeout', iconDownload(), 'Takeout')
                    : ''
                }

                <!-- COMBINE -->
                ${isMultiple
                    ? menuItem('combine', iconMerge(), `Combine Selected (${count})`)
                    : ''
                }

                <div class="my-1 border-t border-slate-200 dark:border-slate-800"></div>

                <!-- CREATE -->
                ${isSingle
                    ? menuItem('create', iconPlus(), 'Create Web View')
                    : ''
                }

                <!-- DELETE -->
                ${menuItem(
                    'delete',
                    iconTrash(),
                    isMultiple
                        ? `Delete ${count} Web Views`
                        : 'Delete Web View',
                    'text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 dark:hover:bg-rose-500/10'
                )}
            `;
        }

        // ========================================================
        // MENU ITEM
        // ========================================================

        function menuItem(action, icon, label, extraClass = '') {
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
                        text-slate-800
                        dark:text-slate-300
                        transition-colors
                        hover:bg-slate-100
                        dark:hover:bg-slate-800
                        hover:text-slate-900
                        dark:hover:text-white
                        ${extraClass}
                    "
                >
                    ${icon}
                    <span class="truncate">${label}</span>
                </button>
            `;
        }

        // ========================================================
        // MENU ACTIONS
        // ========================================================

        menu.addEventListener('click', event => {
            const button = event.target.closest('[data-action]');
            if (!button) return;

            const action = button.dataset.action;
            if (targetIds.length === 0) {
                closeMenu();
                return;
            }

            const view = window.WebView;
            if (!view) {
                closeMenu();
                return;
            }

            switch (action) {
                case 'frontpage':
                    if (targetIds.length === 1 && typeof window.openEditWebFrontPageModal === 'function') {
                        window.openEditWebFrontPageModal(targetIds[0]);
                    }
                    break;

                case 'edit':
                    if (targetIds.length === 1) {
                        view.openEditWebModal(targetIds[0]);
                    }
                    break;

                case 'annotation':
                    if (targetIds.length === 1) {
                        view.openAnnotationModal(targetIds[0]);
                    }
                    break;

                case 'datatags':
                    if (targetIds.length === 1) {
                        view.openDataTagsModal(targetIds[0]);
                    }
                    break;

                case 'duplicate':
                    if (targetIds.length === 1) {
                        view.duplicateWeb(targetIds[0]);
                    }
                    break;

                case 'convert':
                    if (targetIds.length === 1 && typeof view.convertToCollection === 'function') {
                        view.convertToCollection(targetIds[0]);
                    }
                    break;

                case 'takeout':
                    if (targetIds.length === 1 && typeof view.takeoutWeb === 'function') {
                        view.takeoutWeb(targetIds[0]);
                    }
                    break;

                case 'combine':
                    if (targetIds.length >= 2 && typeof view.combineWebs === 'function') {
                        view.combineWebs(targetIds);
                    }
                    break;

                case 'create':
                    view.openCreateModal();
                    break;

                case 'delete':
                    view.openDeleteModal(targetIds);
                    break;
            }

            closeMenu();
        });

        // ========================================================
        // CLOSE LISTENERS
        // ========================================================

        document.addEventListener('click', event => {
            if (!menu.contains(event.target)) {
                closeMenu();
            }
        });

        document.addEventListener('keydown', event => {
            if (event.key === 'Escape') {
                closeMenu();
            }
        });

        window.addEventListener('resize', closeMenu);
        window.addEventListener('scroll', closeMenu, true);

        function closeMenu() {
            menu.classList.add('hidden');
            menu.style.display = 'none';
            menu.style.visibility = 'hidden';
        }

        // ========================================================
        // POSITION MENU
        // ========================================================

        function positionMenu(x, y) {
            const width = menu.offsetWidth || 224;
            const height = menu.offsetHeight || 260;
            const margin = 8;

            let left = x;
            let top = y;

            if (left + width > window.innerWidth - margin) {
                left = window.innerWidth - width - margin;
            }

            if (top + height > window.innerHeight - margin) {
                top = window.innerHeight - height - margin;
            }

            left = Math.max(margin, left);
            top = Math.max(margin, top);

            menu.style.left = `${left}px`;
            menu.style.top = `${top}px`;
        }

        // ========================================================
        // ICONS
        // ========================================================

        function iconBase(content) {
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

        function iconEdit() {
            return iconBase(`
                <path d="M12 20h9"/>
                <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4Z"/>
            `);
        }

        function iconFileText() {
            return iconBase(`
                <path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/>
                <polyline points="14 2 14 8 20 8"/>
                <line x1="16" y1="13" x2="8" y2="13"/>
                <line x1="16" y1="17" x2="8" y2="17"/>
                <line x1="10" y1="9" x2="8" y2="9"/>
            `);
        }

        function iconCopy() {
            return iconBase(`
                <rect x="9" y="9" width="11" height="11" rx="2"/>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
            `);
        }

        function iconTags() {
            return iconBase(`
                <path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3.4 13.4a2 2 0 0 1 0-2.8V5a2 2 0 0 1 2-2h5.6a2 2 0 0 1 1.4.6l8.2 8.2a2 2 0 0 1 0 2.8Z"/>
                <circle cx="7.5" cy="7.5" r="1.2"/>
            `);
        }

        function iconFolderSwitch() {
            return iconBase(`
                <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.93a2 2 0 0 1-1.66-.9l-.82-1.2A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z"/>
                <path d="m11 10 3 3-3 3"/>
                <path d="M8 13h6"/>
            `);
        }

        function iconDownload() {
            return iconBase(`
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                <polyline points="7 10 12 15 17 10"/>
                <line x1="12" y1="15" x2="12" y2="3"/>
            `);
        }

        function iconMerge() {
            return iconBase(`
                <path d="M6 4v5"/>
                <path d="M18 4v5"/>
                <path d="M6 9c0 4 3 6 6 6"/>
                <path d="M18 9c0 4-3 6-6 6"/>
                <path d="M12 15v5"/>
            `);
        }

        function iconPlus() {
            return iconBase(`
                <path d="M12 5v14"/>
                <path d="M5 12h14"/>
            `);
        }

        function iconTrash() {
            return iconBase(`
                <path d="M4 7h16"/>
                <path d="M10 11v6"/>
                <path d="M14 11v6"/>
                <path d="M6 7l1 13h10l1-13"/>
                <path d="M9 7V4h6v3"/>
            `);
        }
    }
})();