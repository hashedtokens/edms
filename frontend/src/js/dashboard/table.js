function createStatsTable({
    containerId,
    title,
    entries,
    rows
}) {
    const container = document.getElementById(containerId);

    if (!container) {
        console.warn(`Container not found: ${containerId}`);
        return;
    }

    // HTTP METHODS
    const methods = [
        {
            name: "GET",
            color: "text-emerald-600 dark:text-emerald-500"
        },
        {
            name: "POST",
            color: "text-yellow-600 dark:text-yellow-500"
        },
        {
            name: "PUT",
            color: "text-blue-600 dark:text-blue-500"
        },
        {
            name: "PATCH",
            color: "text-purple-600 dark:text-purple-500"
        },
        {
            name: "DELETE",
            color: "text-red-600 dark:text-red-500"
        }
    ];
    // TOTAL ENDPOINTS
    const endpointRow = rows.find(
        row => row.name === "Endpoint Segments"
    );

    const endpoints = endpointRow
        ? methods.reduce((total, method) => {
            return total +
                Number(endpointRow.methods?.[method.name]?.count || 0);
        }, 0)
        : 0;
    // METHOD HEADERS
    let methodHeaders = "";

    methods.forEach(method => {

        methodHeaders += `
            <th
                colspan="2"
                class="
                    border-l border-slate-300 dark:border-slate-800
                    px-3 py-2
                    text-center
                    font-semibold
                    ${method.color}
                "
            >
                ${method.name}
            </th>
        `;

    });
    // SUB HEADERS

    let subHeaders = "";

    methods.forEach(() => {

        subHeaders += `
            <th
                class="
                    border-l border-slate-300 dark:border-slate-800
                    px-2 py-1
                    text-center
                "
            >
                %
            </th>

            <th
                class="
                    px-2 py-1
                    text-center
                "
            >
                Count
            </th>
        `;

    });
    // TABLE ROWS

    let tableRows = "";

    rows.forEach(row => {

        let cells = "";

        methods.forEach(method => {

            const data =
                row.methods?.[method.name] || {
                    percentage: "—",
                    count: "—"
                };
            // Percentage
            cells += `
                <td
                    class="
                        border-l border-slate-300 dark:border-slate-800
                        px-3 py-2.5
                        text-center
                        font-semibold
                        ${method.color}
                    "
                >
                    ${data.percentage}
                </td>
            `;
            // Count
            cells += `
                <td
                    class="
                        px-3 py-2.5
                        text-center
                        text-slate-500 dark:text-slate-400
                    "
                >
                    ${data.count}
                </td>
            `;

        });
        tableRows += `
            <tr
                class="
                    border-b border-slate-300 dark:border-slate-800
                    last:border-b-0
                "
            >

                <td
                    class="
                        px-3 py-2.5
                        font-medium
                        text-slate-600 dark:text-slate-300
                    "
                >
                    ${row.name}
                </td>

                ${cells}

            </tr>
        `;

    });
    // TABLE HTML

    container.innerHTML = `

        <section
            class="
                mt-3
                overflow-hidden
                rounded-lg
                border border-slate-300 dark:border-slate-800
                bg-slate-100 dark:bg-slate-900
            "
        >

            <div
                class="
                    flex
                    items-center
                    justify-between
                    border-b border-slate-300 dark:border-slate-800
                    px-2.5 py-1.5
                "     >
                <h3
                    class="
                        text-sm
                        font-semibold
                        text-slate-700 dark:text-slate-200
                    "
                >
                    ${title}
                </h3>

            </div>

            <div class="overflow-x-auto">

                <table
                    class="
                        w-full
                        min-w-[850px]
                        text-xs
                    "
                >

                    <thead>

                        <!-- HTTP METHOD HEADER -->

                        <tr
                            class="
                                border-b
                                border-slate-300 dark:border-slate-800
                            "
                        >

                            <!-- Row name column: now holds Endpoints / Entries -->

                            <th
                                class="
                                    w-40
                                    px-3 py-2
                                    text-left
                                "
                            >
                                <div class="flex flex-col gap-0.5">
                                    <span class="text-[10px] font-semibold text-slate-600 dark:text-slate-300">
                                        Endpoints ${endpoints}
                                    </span>
                                    <span class="text-[10px] font-semibold text-slate-600 dark:text-slate-300">
                                        Entries ${entries}
                                    </span>
                                </div>
                            </th>
                           ${methodHeaders}

                        </tr>


                        <!-- % / COUNT HEADER -->

                        <tr
                            class="
                                border-b
                                border-slate-300 dark:border-slate-800
                                text-[10px]
                                text-slate-400 dark:text-slate-500
                            "
                        >

                            <th></th>

                            ${subHeaders}

                        </tr>

                    </thead>


                    <!-- DATA -->

                    <tbody>

                        ${tableRows}

                    </tbody>

                </table>

            </div>

        </section>
    `;
}