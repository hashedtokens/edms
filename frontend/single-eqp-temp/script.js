
// Find the theme toggle button in HTML
const themeToggle = document.getElementById("theme-toggle");

// Listen for clicks on the button
themeToggle.addEventListener("click", () => {
  // Read the currently active theme
  const currentTheme = document.body.dataset.theme;

  // Switch to the opposite theme
  const newTheme = currentTheme === "light" ? "dark" : "light";

  // Update the theme attribute
  document.body.dataset.theme = newTheme;

  // Update the button label
  themeToggle.textContent = newTheme === "light" ? "Dark Mode" : "Light Mode";
});

// Set the initial button label
themeToggle.textContent = "Light Mode";

/* ========================================
   2. Embedded EDMS data
======================================== */

const eqpData = {
  endpoint: "/api/list/users/admin",

  operations: [
    {
      eid: "EID-001",
      method: "DELETE",
      annotation: "analytics API endpoint used for admin",
      status: "inactive",

      tags: ["support", "analytics", "reports"],

      qps: [
        {
          id: 1,

          request: {
            request_id: "EID-001-request1",
            meta_data: {
              method: "DELETE",
              content_type: "text/plain",
              authorization: "API Key",
              request_size: "10 KB",
            },
            body: {
              headers: {
                Authorization: "Bearer sample_token",
                "Content-Type": "text/plain",
              },
              data: {
                task: "update_profile",
                priority: "high",
                username: "emma_brown",
                email: "emma_brown@example.com",
              },
            },
          },

          // Response data will be added after
          // inspecting response-1.json

          response: {
            response_id: "EID-001-response1",
            endpoint_id: "EID-001",

            meta_data: {
              status_code: 200,
              content_type: "text/plain",
              response_size: "10 KB",
              response_time: "788 ms",
            },

            body: {
              status: "success",
              code: 200,
              message: "Request completed successfully",

              data: {
                user_id: 8911,
                username: "emma_brown",
                email: "emma_brown@example.com",
                role: "user",
                created_at: "2025-07-25T15:32:02Z",
              },
            },
          },
          headers: {
            request_headers: {
              Authorization: "Bearer sample_token",
              "Content-Type": "application/json",
            },
            response_headers: {
              "Content-Type": "application/json",
            },
          },
        },

        {
          id: 2,

          request: {
            request_id: "EID-001-request2",
            meta_data: {
              method: "DELETE",
              content_type: "application/json",
              authorization: "Bearer Token",
              request_size: "1 KB",
            },
            body: {
              headers: {
                Authorization: "Bearer sample_token",
                "Content-Type": "application/json",
              },
              data: {
                task: "check_status",
                priority: "high",
                username: "emma_brown",
                email: "emma_brown@example.com",
              },
            },
          },

          response: {
            response_id: "EID-001-response2",
            endpoint_id: "EID-001",

            meta_data: {
              status_code: 500,
              content_type: "application/json",
              response_size: "15 KB",
              response_time: "383 ms",
            },

            body: {
              status: "error",
              code: 500,
              message: "Internal server error",

              data: {
                user_id: 7184,
                username: "emma_brown",
                email: "emma_brown@example.com",
                role: "support",
                created_at: "2026-09-13T18:04:16Z",
              },
            },
          },
          headers: {
            request_headers: {
              Authorization: "Bearer sample_token",
              "Content-Type": "application/json",
            },
            response_headers: {
              "Content-Type": "application/json",
            },
          },
        },
      ],
    },
  ],
};

/* ========================================
   3. Render endpoint information
======================================== */

function renderEndpoint() {
  const endpointPath = document.getElementById("endpoint-path");

  const endpointId = document.getElementById("endpoint-id");

  endpointPath.textContent = eqpData.endpoint;

  endpointId.textContent = eqpData.operations
    .map((operation) => operation.eid)
    .join(", ");
}

// Initial rendering
renderEndpoint();

/* ========================================
   4. Render global statistics
======================================== */

function renderStats() {
  const operations = eqpData.operations;

  const totalCruds = operations.length;

  const totalQps = operations.reduce(
    (total, operation) => total + operation.qps.length,
    0,
  );

  const uniqueTags = new Set(operations.flatMap((operation) => operation.tags));

  const dataSize = new Blob([JSON.stringify(eqpData)]).size;

  const sizeLabel =
    dataSize >= 1024 * 1024
      ? `${(dataSize / (1024 * 1024)).toFixed(2)} MB`
      : `${(dataSize / 1024).toFixed(2)} KB`;

  document.getElementById("total-cruds").textContent = totalCruds;

  document.getElementById("total-qps").textContent = totalQps;

  document.getElementById("total-tags").textContent = uniqueTags.size;

  document.getElementById("data-size").textContent = sizeLabel;
}

renderStats();

/* ========================================
   5. Render CRUD operations table
======================================== */

function renderCrudTable() {
  const tableBody = document.getElementById("crud-table-body");

  // Remove previously rendered rows
  tableBody.replaceChildren();

  // Loop through every CRUD operation
  eqpData.operations.forEach((operation) => {
    // Create a table row
    const row = document.createElement("tr");

    // Create the table cells
    const values = [
      operation.eid,
      operation.method,
      operation.qps.length,
      operation.tags.join(", "),
      operation.annotation,
    ];

    values.forEach((value, index) => {
      const cell = document.createElement("td");

      if (index === 1) {
        const badge = document.createElement("span");

        badge.className = `method-badge method-${operation.method.toLowerCase()}`;

        badge.textContent = value;

        cell.appendChild(badge);
      } else {
        cell.textContent = value;
      }

      row.appendChild(cell);
    });

    // Store the EID on the row
    row.dataset.eid = operation.eid;

    row.tabIndex = 0;

    row.addEventListener("click", () => {
      selectOperation(operation.eid);
    });

    row.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        selectOperation(operation.eid);
      }
    });

    // Add the completed row to the table
    tableBody.appendChild(row);
  });
}

// Initial rendering
renderCrudTable();

/* ========================================
   6. Selection state
======================================== */

let selectedOperation = null;
let selectedQP = null;
let activeRequestTab = "body";
let activeResponseTab = "body";

/* ========================================
   7. Render tags
======================================== */

function renderTags() {
  const localTags = document.getElementById("local-tags");
  const globalTags = document.getElementById("global-tags");

  localTags.replaceChildren();
  globalTags.replaceChildren();

  if (selectedOperation) {
    selectedOperation.tags.forEach((tag) => {
      const badge = document.createElement("span");

      badge.className = "tag-badge";
      badge.textContent = tag;

      localTags.appendChild(badge);
    });
  } else {
    localTags.textContent = "Select a CRUD operation";
  }

  const allTags = [
    ...new Set(eqpData.operations.flatMap((operation) => operation.tags)),
  ];

  allTags.forEach((tag) => {
    const badge = document.createElement("span");

    badge.className = "tag-badge";
    badge.textContent = tag;

    globalTags.appendChild(badge);
  });
}

/* ========================================
   8. Render QP selector
======================================== */

function renderQPList() {
  const qpList = document.getElementById("qp-list");

  qpList.replaceChildren();

  if (!selectedOperation) {
    qpList.textContent = "Select a CRUD operation";
    return;
  }

  selectedOperation.qps.forEach((qp) => {
    const button = document.createElement("button");

    button.type = "button";
    button.textContent = `QP ${qp.id}`;

    if (selectedQP === qp) {
      button.classList.add("active");
    }

    button.addEventListener("click", () => {
      selectQP(qp.id);
    });

    qpList.appendChild(button);
  });
}

function selectQP(qpId) {
  if (!selectedOperation) return;

  selectedQP = selectedOperation.qps.find((qp) => qp.id === qpId) ?? null;

  renderQPList();
  renderRequestResponse();

  console.log("Selected QP:", selectedQP);
}

/* ========================================
   10. Render Request and Response
======================================== */


function renderRequestResponse() {
  const requestBody = document.getElementById("request-body");

  const responseBody = document.getElementById("response-body");

  const requestMeta = document.getElementById("request-meta");

  const responseMeta = document.getElementById("response-meta");

  const responseStatus = document.getElementById("response-status");

  const copyRequest = document.getElementById("copy-request");

  const copyResponse = document.getElementById("copy-response");

  updateTabButtons();

  if (!selectedQP) {
    requestBody.textContent = "Select a QP";
    responseBody.textContent = "Select a QP";

    requestMeta.textContent = "";
    responseMeta.textContent = "";
    responseStatus.textContent = "Status";

    responseStatus.classList.remove("is-success", "is-error");

    copyRequest.disabled = true;
    copyResponse.disabled = true;
    return;
  }

  const request = selectedQP.request;
  const response = selectedQP.response;

  // These headers should come from the saved
  // header JSON embedded in the page.
  const requestHeaders =
    selectedQP.headers?.request_headers ?? request?.body?.headers ?? {};

  const responseHeaders = selectedQP.headers?.response_headers ?? {};

  const requestContent =
    activeRequestTab === "body" ? request?.body : requestHeaders;

  const responseContent =
    activeResponseTab === "body" ? response?.body : responseHeaders;

  requestBody.textContent = formatJSON(requestContent);
  responseBody.textContent = formatJSON(responseContent);

  requestMeta.textContent =
    `Method: ${request?.meta_data?.method ?? "N/A"} | ` +
    `Size: ${request?.meta_data?.request_size ?? "N/A"}`;

  responseMeta.textContent =
    `Size: ${response?.meta_data?.response_size ?? "N/A"} | ` +
    `Time: ${response?.meta_data?.response_time ?? "N/A"}`;

  const statusCode = response?.meta_data?.status_code;

  responseStatus.textContent = statusCode ?? "N/A";

  responseStatus.classList.toggle(
    "is-success",
    statusCode >= 200 && statusCode < 400,
  );

  responseStatus.classList.toggle("is-error", statusCode >= 400);

  copyRequest.disabled = false;
  copyResponse.disabled = false;
}

//copy content

async function copyPanelContent(elementId, buttonId) {
  const contentElement = document.getElementById(elementId);
  const button = document.getElementById(buttonId);

  if (!contentElement || !button) return;

  const content = contentElement.textContent;

  try {
    await navigator.clipboard.writeText(content);

    button.textContent = "Copied!";

    setTimeout(() => {
      button.textContent = "Copy";
    }, 1500);
  } catch (error) {
    console.error("Clipboard error:", error);
    button.textContent = "Copy failed";

    setTimeout(() => {
      button.textContent = "Copy";
    }, 1500);
  }
}



function updateTabButtons() {
  const tabs = [
    ["request-body-tab", activeRequestTab === "body"],
    ["request-headers-tab", activeRequestTab === "headers"],
    ["response-body-tab", activeResponseTab === "body"],
    ["response-headers-tab", activeResponseTab === "headers"],
  ];

  tabs.forEach(([id, active]) => {
    const button = document.getElementById(id);

    if (!button) return;

    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  });
}

function formatJSON(value) {
  return JSON.stringify(value ?? {}, null, 2);
}


/* ========================================
   9. Select CRUD operation
======================================== */

function selectOperation(eid) {
  selectedOperation =
    eqpData.operations.find((operation) => operation.eid === eid) ?? null;

  // Reset the QP when switching CRUD operations
  selectedQP = null;

  // Highlight the selected table row
  document.querySelectorAll("#crud-table-body tr").forEach((row) => {
    row.classList.toggle("active", row.dataset.eid === eid);
  });

  renderTags();
  renderQPList();
  renderRequestResponse();
}

renderTags();
renderQPList();


document.getElementById("request-body-tab").addEventListener("click", () => {
  activeRequestTab = "body";
  renderRequestResponse();
});

document.getElementById("request-headers-tab").addEventListener("click", () => {
  activeRequestTab = "headers";
  renderRequestResponse();
});

document.getElementById("response-body-tab").addEventListener("click", () => {
  activeResponseTab = "body";
  renderRequestResponse();
});

document
  .getElementById("response-headers-tab")
  .addEventListener("click", () => {
    activeResponseTab = "headers";
    renderRequestResponse();
  });

document.getElementById("copy-request").addEventListener("click", () => {
  copyPanelContent("request-body", "copy-request");
});
document.getElementById("copy-response").addEventListener("click", () => {
  copyPanelContent("response-body", "copy-response");
});