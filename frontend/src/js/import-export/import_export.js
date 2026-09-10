document.addEventListener("DOMContentLoaded", () => {
    // IMPORT / EXPORT DATA
    let importExportData = [];

    let currentView = "compressed";

    let selectedItem = null;
    // LOAD DATA
    function loadImportExportData() {

        fetch("../../data/import_export.json")

            .then(response => {

                if (!response.ok) {

                    throw new Error(
                        "Failed to load import_export.json"
                    );
                }
                return response.json();
            })
            .then(data => {
                importExportData = data;
                createImportExportTable();

            })
            .catch(error => {
                console.error(
                    "Error loading Import / Export data:",
                    error
                );
            });
    }
    // RENDER TABLE
    function createImportExportTable() {
        const table =
            document.getElementById(
                "importExportTable"
            );

        if (!table) return;
        table.innerHTML = "";
        // Search
        const searchInput =
            document.getElementById(
                "importExportSearch"
            );
        const search =
            searchInput
                ? searchInput.value.toLowerCase()
                : "";
        // Type filter
        const typeFilter =
            document.getElementById(
                "typeFilter"
            );

        const type =
            typeFilter
                ? typeFilter.value
                : "all";
        // Purpose filter
        const purposeFilter =
            document.getElementById(
                "purposeFilter"
            );

        const purpose =
            purposeFilter
                ? purposeFilter.value
                : "all";
        // CREATE ROWS
        importExportData.forEach((item, index) => {
            // View filter
            if (
                item.view &&
                item.view !== currentView
            ) {
                return;
            }
            // Search filter
            if (
                search &&
                !item.name
                    .toLowerCase()
                    .includes(search)
            ) {

                return;

            }
            // Type filter
            if (
                type !== "all" &&
                item.type !== type
            ) {

                return;
            }
            // Purpose filter
            if (
                purpose !== "all" &&
                item.purpose !== purpose
            ) {

                return;
            }
            // CREATE ROW

            const row =
                document.createElement("tr");
            row.className =
                "transition hover:bg-slate-800/40";

            row.innerHTML = `

                <!-- Folder Name -->

                <td class="px-5 py-3
                           font-medium
                           text-slate-200">

                    ${item.name}

                </td>
                <!-- Purpose -->

                <td class="px-5 py-3
                           text-slate-400">

                    ${item.purpose}

                </td>
                <!-- Size -->

                <td class="px-5 py-3
                           text-slate-400">

                    ${item.size}

                </td>
                <!-- Format Check -->

                <td class="px-5 py-3">

                    ${getFormatCheckBadge(
                item.formatCheck
            )}

                </td>

            `;
            // RIGHT CLICK
            row.addEventListener(
                "contextmenu",
                event => {

                    event.preventDefault();

                    selectedItem = item;

                    showContextMenu(
                        event.clientX,
                        event.clientY
                    );

                }
            );
            table.appendChild(row);

        });
    }
    // FORMAT CHECK BADGE
    function getFormatCheckBadge(result) {

        if (result === "success") {

            return `

                <span class="
                    rounded-full
                    bg-emerald-500/10
                    px-2 py-1
                    text-xs
                    text-emerald-400">

                    Success

                </span>
            `;
        }

        if (result === "fail") {
            return `
                <span class="
                    rounded-full
                    bg-red-500/10
                    px-2 py-1
                    text-xs
                    text-red-400">

                    Fail
                </span>

            `;
        }
        return `
            <span class="
                rounded-full
                bg-slate-800
                px-2 py-1
                text-xs
                text-slate-400">

                Not checked
            </span>

        `;
    }
    // SEARCH

    const searchInput =
        document.getElementById(
            "importExportSearch"
        );
    if (searchInput) {

        searchInput.addEventListener(
            "input",
            () => {

                createImportExportTable();

            }
        );

    }
    // TYPE FILTER
    const typeFilter =
        document.getElementById(
            "typeFilter"
        );
    if (typeFilter) {

        typeFilter.addEventListener(
            "change",
            () => {

                createImportExportTable();

            }
        );
    }
    // PURPOSE FILTER
    const purposeFilter =
        document.getElementById(
            "purposeFilter"
        );
    if (purposeFilter) {

        purposeFilter.addEventListener(
            "change",
            () => {

                createImportExportTable();

            }
        );

    }
    // RESET

    const resetFilters =
        document.getElementById(
            "resetFilters"
        );

    if (resetFilters) {

        resetFilters.addEventListener(
            "click",
            () => {

                if (searchInput) {

                    searchInput.value = "";

                }
                if (typeFilter) {

                    typeFilter.value = "all";

                }
                if (purposeFilter) {

                    purposeFilter.value = "all";

                }
                createImportExportTable();
            }
        );
    }
    // COMPRESSED
    const compressedTab =
        document.getElementById(
            "compressedTab"
        );
    const uncompressedTab =
        document.getElementById(
            "uncompressedTab"
        );

    if (compressedTab) {

        compressedTab.addEventListener(
            "click",
            () => {

                currentView = "compressed";

                compressedTab.classList.remove(
                    "bg-slate-800",
                    "text-slate-300"
                );

                compressedTab.classList.add(
                    "bg-cyan-500",
                    "text-slate-950"
                );


                if (uncompressedTab) {

                    uncompressedTab.classList.remove(
                        "bg-cyan-500",
                        "text-slate-950"
                    );

                    uncompressedTab.classList.add(
                        "bg-slate-800",
                        "text-slate-300"
                    );

                }
                createImportExportTable();
            }
        );
    }
    // UNCOMPRESSED

    if (uncompressedTab) {

        uncompressedTab.addEventListener(
            "click",
            () => {

                currentView = "uncompressed";

                uncompressedTab.classList.remove(
                    "bg-slate-800",
                    "text-slate-300"
                );

                uncompressedTab.classList.add(
                    "bg-cyan-500",
                    "text-slate-950"
                );


                if (compressedTab) {

                    compressedTab.classList.remove(
                        "bg-cyan-500",
                        "text-slate-950"
                    );

                    compressedTab.classList.add(
                        "bg-slate-800",
                        "text-slate-300"
                    );

                }
                createImportExportTable();
            }
        );

    }
    // REFRESH
    const refreshBtn =
        document.getElementById(
            "refreshBtn"
        );

    if (refreshBtn) {

        refreshBtn.addEventListener(
            "click",
            () => {
                loadImportExportData();
            }
        );

    }
    // IMPORT

    const importBtn =
        document.getElementById(
            "importBtn"
        );


    if (importBtn) {

        importBtn.addEventListener(
            "click",
            () => {
                alert(
                    "Import operation requested."
                );

            }
        );

    }
    // EXPORT MODAL

    const exportBtn =
        document.getElementById("exportBtn");

    const exportModal =
        document.getElementById("exportModal");

    const cancelExport =
        document.getElementById("cancelExport");

    const confirmExport =
        document.getElementById("confirmExport");

    // OPEN EXPORT MODAL

    if (exportBtn && exportModal) {

        exportBtn.addEventListener(
            "click",
            () => {

                exportModal.classList.remove(
                    "hidden"
                );

                exportModal.classList.add(
                    "flex"
                );

            }
        );

    }

    // CLOSE EXPORT MODAL

    if (cancelExport && exportModal) {

        cancelExport.addEventListener(
            "click",
            () => {

                exportModal.classList.add(
                    "hidden"
                );

                exportModal.classList.remove(
                    "flex"
                );

            }
        );

    }
    // CONFIRM EXPORT

    if (confirmExport && exportModal) {

        confirmExport.addEventListener(
            "click",
            () => {

                const selectedPurpose =
                    document.querySelector(
                        'input[name="exportPurpose"]:checked'
                    );


                if (!selectedPurpose) {

                    alert(
                        "Please select an export purpose."
                    );

                    return;
                }
                const purpose =
                    selectedPurpose.value;


                exportModal.classList.add(
                    "hidden"
                );

                exportModal.classList.remove(
                    "flex"
                );


                alert(
                    "Export requested: " +
                    purpose
                );

            }
        );

    }
    // CONTEXT MENU
    const contextMenu =
        document.getElementById(
            "contextMenu"
        );

    function showContextMenu(x, y) {

        if (!contextMenu) return;


        contextMenu.style.position =
            "fixed";

        contextMenu.style.left =
            x + "px";

        contextMenu.style.top =
            y + "px";


        contextMenu.classList.remove(
            "hidden"
        );

    }


    function hideContextMenu() {

        if (!contextMenu) return;


        contextMenu.classList.add(
            "hidden"
        );

    }
    // Hide context menu when clicking elsewhere
    document.addEventListener(
        "click",
        event => {

            if (
                contextMenu &&
                !contextMenu.contains(event.target)
            ) {

                hideContextMenu();

            }

        }
    );
    // FORMAT CHECK
    const formatCheckBtn =
        document.getElementById(
            "formatCheckBtn"
        );


    if (formatCheckBtn) {

        formatCheckBtn.addEventListener(
            "click",
            () => {

                hideContextMenu();


                if (!selectedItem) return;
                alert(
                    "Format Check requested for: " +
                    selectedItem.name
                );

            }
        );

    }
    // MOVE
    const moveBtn =
        document.getElementById(
            "moveBtn"
        );
    if (moveBtn) {

        moveBtn.addEventListener(
            "click",
            () => {

                hideContextMenu();


                if (!selectedItem) return;


                alert(
                    "Move requested for " +
                    selectedItem.name +
                    " → " +
                    selectedItem.purpose
                );

            }
        );

    }
    // TAKEOUT

    const takeoutBtn =
        document.getElementById(
            "takeoutBtn"
        );

    const takeoutModal =
        document.getElementById(
            "takeoutModal"
        );

    if (takeoutBtn) {

        takeoutBtn.addEventListener(
            "click",
            () => {
                hideContextMenu();
                if (!selectedItem) return;
                if (!takeoutModal) return;

                takeoutModal.classList.remove(
                    "hidden"
                );
                takeoutModal.classList.add(
                    "flex"
                );
                const folderName =
                    document.getElementById(
                        "takeoutFolderName"
                    );


                if (folderName) {

                    folderName.value =
                        selectedItem.name;

                }

            }
        );

    }
    // CANCEL TAKEOUT

    const cancelTakeout =
        document.getElementById(
            "cancelTakeout"
        );
    if (cancelTakeout) {

        cancelTakeout.addEventListener(
            "click",
            () => {

                if (!takeoutModal) return;
                takeoutModal.classList.add(
                    "hidden"
                );
                takeoutModal.classList.remove(
                    "flex"
                );

            }
        );
    }    // APPLY TAKEOUT

    const applyTakeout =
        document.getElementById(
            "applyTakeout"
        );

    if (applyTakeout) {

        applyTakeout.addEventListener(
            "click",
            () => {

                const folderName =
                    document.getElementById(
                        "takeoutFolderName"
                    );
                const action =
                    document.querySelector(
                        'input[name="takeoutAction"]:checked'
                    );
                if (
                    !folderName ||
                    !folderName.value.trim()
                ) {

                    alert(
                        "Please enter a folder name."
                    );

                    return;

                }
                const actionValue =
                    action
                        ? action.value
                        : "rename";

                alert(
                    "Takeout requested:\n\n" +
                    "Folder: " +
                    folderName.value +
                    "\nAction: " +
                    actionValue
                );


                if (takeoutModal) {

                    takeoutModal.classList.add(
                        "hidden"
                    );

                    takeoutModal.classList.remove(
                        "flex"
                    );

                }

            }
        );
    }
    // INITIALIZE

    loadImportExportData();

});