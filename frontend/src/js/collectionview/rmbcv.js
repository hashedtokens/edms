// ============================================================
// EDMS COLLECTION VIEW
// RIGHT-CLICK MENU
// ============================================================

(() => {
  "use strict";

  // ============================================================
  // INIT
  // ============================================================

  document.addEventListener("DOMContentLoaded", init);

  function init() {
    const menu = document.getElementById("contextMenu");

    const tbody = document.getElementById("folderTableBody");

    if (!menu || !tbody || !window.CollectionView) {
      console.error("Collection View context menu could not initialize.");

      return;
    }

    let targetIds = [];

    // ========================================================
    // RIGHT CLICK ON FOLDER ROW
    // ========================================================

    tbody.addEventListener("contextmenu", (event) => {
      const row = event.target.closest("tr[data-id]");

      if (!row) {
        return;
      }

      event.preventDefault();

      const id = row.dataset.id;

      const selected = window.CollectionView.getState().selected;

      /*
       * If the clicked folder is already inside a
       * multi-selection, operate on the whole selection.
       *
       * Otherwise only operate on the clicked folder.
       */

      if (selected.has(id) && selected.size > 1) {
        targetIds = [...selected];
      } else {
        targetIds = [id];
      }

      renderMenu();

      /*
       * Make the menu visible before measuring it.
       */

      menu.classList.remove("hidden");

      menu.style.display = "block";

      menu.style.visibility = "hidden";

      positionMenu(event.clientX, event.clientY);

      menu.style.visibility = "visible";
    });

    // ========================================================
    // BUILD MENU
    // ========================================================

    function renderMenu() {
      const count = targetIds.length;

      const isSingle = count === 1;

      const isMultiple = count > 1;

      menu.innerHTML = `

            <!-- ============================================ -->
            <!-- DATA VIEW -->
            <!-- ============================================ -->

            ${isSingle ? menuItem("dataview", iconDatabase(), "Data View") : ""}





            <!-- ============================================ -->
            <!-- WEB VIEW -->
            <!-- ============================================ -->

            ${isSingle ? menuItem("webview", iconGlobe(), "Create Web View") : ""}
            ${isSingle ? menuItem("repoview", iconRepo(), "Create Repo View") : ""}
            
            <!-- ============================================ -->
            <!-- CREATE TEST VIEW -->
            <!-- ============================================ -->

            ${
              isSingle
                ? menuItem("testview", iconFlask(), "Create Test View")
                : ""
            }


            <!-- ============================================ -->
            <!-- RENAME -->
            <!-- ============================================ -->

            ${isSingle ? menuItem("rename", iconEdit(), "Rename") : ""}


            <!-- ============================================ -->
            <!-- DUPLICATE -->
            <!-- ============================================ -->

            ${isSingle ? menuItem("duplicate", iconCopy(), "Duplicate") : ""}


            <!-- ============================================ -->
            <!-- MODIFY (TAGS & ANNOTATION) -->
            <!-- ============================================ -->

            ${isSingle ? menuItem("edittags", iconTag(), "Modify") : ""}


            ${
              isSingle
                ? '<div class="my-1 border-t border-slate-800"></div>'
                : ""
            }


            <!-- ============================================ -->
            <!-- MERGE -->
            <!-- ============================================ -->

            ${
              isMultiple
                ? menuItem("merge", iconMerge(), `Merge Selected (${count})`)
                : ""
            }


            <!-- ============================================ -->
            <!-- NEW FOLDER -->
            <!-- ============================================ -->

            ${
              isSingle
                ? menuItem("newfolder", iconFolderPlus(), "New Empty Folder")
                : ""
            }


            ${
              isMultiple
                ? '<div class="my-1 border-t border-slate-800"></div>'
                : ""
            }


            <!-- ============================================ -->
            <!-- DELETE -->
            <!-- ============================================ -->

            ${menuItem(
              "delete",
              iconTrash(),
              isMultiple ? `Delete ${count} Collections` : "Delete Collection",
              "text-rose-400 hover:bg-rose-500/10",
            )}

        `;
    }

    // ========================================================
    // MENU ITEM
    // ========================================================

    function menuItem(action, icon, label, extraClass = "") {
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
                    text-slate-700 dark:text-slate-300
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

    menu.addEventListener("click", async (event) => {
      const button = event.target.closest("[data-action]");

      if (!button) {
        return;
      }

      const action = button.dataset.action;

      if (targetIds.length === 0) {
        closeMenu();

        return;
      }

      switch (action) {
        // --------------------------------------------
        // DATA VIEW
        // --------------------------------------------

        case "dataview":
          if (targetIds.length === 1) {
            window.CollectionView.openDataView(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // CREATE TEST VIEW
        // --------------------------------------------

        case "testview":
          if (targetIds.length === 1) {
            /*
             * Open the Test View page in a new tab,
             * pre-scoped to the right-clicked collection.
             */
            window.CollectionView.openTestView(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // REPO VIEW
        // --------------------------------------------

        case "repoview":
          if (targetIds.length === 1) {
            window.CollectionView.openRepoViewModal(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // RENAME
        // --------------------------------------------

        case "rename":
          if (targetIds.length === 1) {
            window.CollectionView.openRenameModal(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // EDIT TAGS
        // --------------------------------------------

        case "edittags":
          if (targetIds.length === 1) {
            window.CollectionView.openTagEditor(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // DUPLICATE
        // --------------------------------------------

        case "duplicate":
          if (targetIds.length === 1) {
            window.CollectionView.duplicateFolder(targetIds[0]);
          }

          break;

        // --------------------------------------------
        // MERGE
        // --------------------------------------------

        case "merge":
          if (targetIds.length >= 2) {
            window.CollectionView.openMergeModal(targetIds);
          }

          break;

        // --------------------------------------------
        // NEW FOLDER
        // --------------------------------------------

        case "newfolder":
          window.CollectionView.openNewFolderModal();

          break;

        // --------------------------------------------
        // DELETE
        // --------------------------------------------

        case "delete":
          window.CollectionView.openDeleteModal(targetIds);

          break;
      }

      closeMenu();
    });

    // ========================================================
    // CLOSE ON NORMAL CLICK
    // ========================================================

    document.addEventListener("click", (event) => {
      if (!menu.contains(event.target)) {
        closeMenu();
      }
    });

    // ========================================================
    // CLOSE ON ESCAPE
    // ========================================================

    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        closeMenu();
      }
    });

    // ========================================================
    // CLOSE ON RESIZE
    // ========================================================

    window.addEventListener("resize", closeMenu);

    // ========================================================
    // CLOSE ON SCROLL
    // ========================================================

    window.addEventListener("scroll", closeMenu, true);

    // ========================================================
    // CLOSE MENU
    // ========================================================

    function closeMenu() {
      menu.classList.add("hidden");

      menu.style.display = "none";

      menu.style.visibility = "hidden";
    }

    // ========================================================
    // POSITION MENU
    // ========================================================

    function positionMenu(x, y) {
      /*
       * Menu must already be display:block here so that
       * offsetWidth / offsetHeight are available.
       */

      const width = menu.offsetWidth || 224;

      const height = menu.offsetHeight || 240;

      const margin = 8;

      let left = x;

      let top = y;

      // --------------------------------------------
      // Horizontal boundary
      // --------------------------------------------

      if (left + width > window.innerWidth - margin) {
        left = window.innerWidth - width - margin;
      }

      // --------------------------------------------
      // Vertical boundary
      // --------------------------------------------

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

    function iconFlask() {
      return iconBase(`

            <path
                d="M9 3h6"
            />

            <path
                d="M9 3v7L5 20h14L15 10V3"
            />

            <circle
                cx="9"
                cy="16"
                r="1"
                fill="currentColor"
                stroke="none"
            />

            <circle
                cx="13"
                cy="14"
                r="0.8"
                fill="currentColor"
                stroke="none"
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
    function iconRepo() {
      return iconBase(`
        <path
            d="M4 4.5A1.5 1.5 0 0 1 5.5 3h13A1.5 1.5 0 0 1 20 4.5v15a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 19.5z"
        />

        <path d="M8 3v18" />

        <circle cx="8" cy="8" r="1" />
        <circle cx="8" cy="12" r="1" />
        <circle cx="8" cy="16" r="1" />

        <path d="M12 8h4" />
        <path d="M12 12h4" />
        <path d="M12 16h4" />
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

    function iconEye() {
      return iconBase(`

            <path
                d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"
            />

            <circle
                cx="12"
                cy="12"
                r="3"
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

    function iconTag() {
      return iconBase(`
            <path d="M20.5 13.5 13.5 20.5a2 2 0 0 1-2.8 0L4 13.8V4h9.8l6.7 6.7a2 2 0 0 1 0 2.8Z" />
            <circle cx="8.5" cy="8.5" r="1" />
        `);
    }
  }
})();
