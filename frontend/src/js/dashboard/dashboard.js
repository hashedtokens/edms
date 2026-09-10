const dates = [
    "Last 24 hrs",
    "11-08-2026",
    "10-08-2026",
    "09-08-2026",
    "08-08-2026",
    "07-08-2026",
    "06-08-2026",
    "05-08-2026",
    "04-08-2026",
    "03-08-2026",
    "02-08-2026"
];

let selectedDate = "Last 24 hrs";

const historyDateList = document.getElementById("historyDateList");

function renderHistory() {
    historyDateList.innerHTML = "";

    dates.forEach((date) => {

        const isActive = date === selectedDate;

        const button = document.createElement("button");

        button.type = "button";

        button.className = `
            flex w-full items-center
            border-b border-slate-800
            px-3 py-2.5
            text-left text-xs
            transition
            ${isActive
                ? "bg-slate-800 font-semibold text-white"
                : "text-slate-400 hover:bg-slate-800 hover:text-white"
            }
        `;

        if (isActive) {
            button.innerHTML = `
                <span class="mr-2 text-cyan-400">●</span>
                <span>${date}</span>
            `;
        } else {
            button.innerHTML = `
                <span class="ml-4">${date}</span>
            `;
        }

        button.addEventListener("click", () => {
            selectedDate = date;
            renderHistory();
        });

        historyDateList.appendChild(button);
    });
}

renderHistory();
document.addEventListener("DOMContentLoaded", () => {

    loadTags();

});


async function loadTags() {

    const tagsContainer =
        document.getElementById("tagsContainer");

    if (!tagsContainer) return;

    try {

        const response =
            await fetch("../data/collections.json");

        const data =
            await response.json();

        const tagCounts = {};

        // Count tags from collections
        data.folders.forEach(folder => {

            if (Array.isArray(folder.tags)) {

                folder.tags.forEach(tag => {

                    tagCounts[tag] =
                        (tagCounts[tag] || 0) + 1;

                });

            }

            // Count tags from endpoints
            if (Array.isArray(folder.endpoints)) {

                folder.endpoints.forEach(endpoint => {

                    if (Array.isArray(endpoint.tags)) {

                        endpoint.tags.forEach(tag => {

                            tagCounts[tag] =
                                (tagCounts[tag] || 0) + 1;

                        });

                    }

                });

            }

        });


        // Convert to array and sort
        const tags =
            Object.entries(tagCounts)
                .map(([name, count]) => ({
                    name,
                    count
                }))
                .sort((a, b) => b.count - a.count);



        tags.slice(0, 13).forEach(tag => {

            const item =
                document.createElement("div");

            item.className =
                "flex items-center gap-1.5 rounded-md " +
                "border border-slate-700 bg-slate-800 " +
                "px-2 py-1";

            item.innerHTML = `
                <span class="rounded border border-slate-600
                             bg-slate-700 px-2 py-0.5
                             text-xs text-slate-200">
                    ${tag.name}
                </span>

                <span class="text-xs text-slate-400">
                    ${tag.count}
                </span>
            `;

            tagsContainer.appendChild(item);

        });


        // More tags indicator
        if (tags.length > 6) {

            const more =
                document.createElement("span");

            more.className =
                "px-2 py-1 text-xs text-slate-400";

            more.textContent =
                `... more (+${tags.length - 6})`;

            tagsContainer.appendChild(more);

        }

    } catch (error) {

        console.error(
            "Failed to load tags:",
            error
        );

    }

}
document.addEventListener("DOMContentLoaded", () => {
    // COMMON DATA

    const tableRows = [

        {
            name: "Endpoint Segments",

            methods: {

                GET: {
                    percentage: "63%",
                    count: 41
                },

                POST: {
                    percentage: "58%",
                    count: 18
                },

                PUT: {
                    percentage: "45%",
                    count: 9
                },

                PATCH: {
                    percentage: "35%",
                    count: 6
                },

                DELETE: {
                    percentage: "12%",
                    count: 2
                }

            }
        },


        {
            name: "Tags",

            methods: {

                GET: {
                    percentage: "18%",
                    count: 12
                },

                POST: {
                    percentage: "25%",
                    count: 8
                },

                PUT: {
                    percentage: "10%",
                    count: 4
                },

                PATCH: {
                    percentage: "15%",
                    count: 5
                },

                DELETE: {
                    percentage: "4%",
                    count: 1
                }

            }
        },


        {
            name: "Bookmarks",

            methods: {

                GET: {
                    percentage: "48%",
                    count: 20
                },

                POST: {
                    percentage: "30%",
                    count: 12
                },

                PUT: {
                    percentage: "10%",
                    count: 3
                },

                PATCH: {
                    percentage: "5%",
                    count: 2
                },

                DELETE: {
                    percentage: "5%",
                    count: 1
                }

            }
        },


        {
            name: "History",

            methods: {

                GET: {
                    percentage: "38%",
                    count: 15
                },

                POST: {
                    percentage: "35%",
                    count: 14
                },

                PUT: {
                    percentage: "15%",
                    count: 6
                },

                PATCH: {
                    percentage: "5%",
                    count: 2
                },

                DELETE: {
                    percentage: "3%",
                    count: 1
                }

            }
        },


        {
            name: "QP Pairs",

            methods: {

                GET: {
                    percentage: "69%",
                    count: 54
                },

                POST: {
                    percentage: "15%",
                    count: 10
                },

                PUT: {
                    percentage: "8%",
                    count: 5
                },

                PATCH: {
                    percentage: "4%",
                    count: 3
                },

                DELETE: {
                    percentage: "2%",
                    count: 1
                }

            }
        }

    ];


    // TEST VIEW

    createStatsTable({

        containerId: "testTable",

        title: "Test View",

        entries: 520,

        rows: tableRows

    });

    // REPO VIEW

    createStatsTable({

        containerId: "repoTable",

        title: "Repo View",

        entries: 520,

        rows: tableRows

    });


    // COLLECTIONS VIEW

    createStatsTable({

        containerId: "collectionTable",

        title: "Collections View",

        entries: 520,

        rows: tableRows

    });


    // BOOKMARK VIEW

    createStatsTable({

        containerId: "bookmarkTable",

        title: "Bookmark View",

        entries: 520,

        rows: tableRows

    });

});
// CALCULATE CHANGES MODAL - DEMO

document.addEventListener("DOMContentLoaded", () => {

    const calculateChangesButton =
        document.getElementById("calculateChanges");

    const changesModal =
        document.getElementById("changesModal");

    const closeChangesModal =
        document.getElementById("closeChangesModal");

    const cancelChanges =
        document.getElementById("cancelChanges");


    // Open modal
    calculateChangesButton.addEventListener("click", () => {

        changesModal.classList.remove("hidden");
        changesModal.classList.add("flex");

    });


    // Close using X
    closeChangesModal.addEventListener("click", () => {

        changesModal.classList.add("hidden");
        changesModal.classList.remove("flex");

    });


    // Close using Cancel
    cancelChanges.addEventListener("click", () => {

        changesModal.classList.add("hidden");
        changesModal.classList.remove("flex");

    });

});