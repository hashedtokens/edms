const endpointDetailsCache = new Map();
let selectedPathFilters = [];

const fetchEndpointDetail = async (eid) => {
	if (endpointDetailsCache.has(eid)) return endpointDetailsCache.get(eid);
	try {
		const res = await fetch(`./data/endpoints/${eid}.json`);
		const data = await res.json();
		endpointDetailsCache.set(eid, data);
		return data;
	} catch (err) {
		console.error("failed to load", eid, err);
		return null;
	}
};

const fetchEndpointsByIndex = async (indexArray) => {
	// indexArray is array of objects like {eid: 'E0001-BG'}
	const details = await Promise.all(
		indexArray.map((it) => fetchEndpointDetail(it.eid)),
	);
	return details;
};

const loadendpointsIndex = async () => {
	// load the eid index (list of {eid}) and keep per-eid files as source of truth
	const endpointsIndex = await fetch("./data/EndpointIndex.json").then((r) =>
		r.json(),
	);
	allEndpointsIndex = endpointsIndex;
	visibleEndpointsIndex = [...endpointsIndex];
	// warm counts (will fetch per-eid files as needed)
	await updateMethodCounts();
	renderCurrentPage();
};

console.log("loading meta Data ...");

const endpointPerPage = 500;
let totalPages = 0; // will be computed from meta-data
let currentPage = 1;
let visibleEndpointsIndex = [];
let allEndpointsIndex = [];
let selectedCategory = "ALL";
// filtering state
// const selectedPathFilters = new Set();

const updateCategoryTabStyles = () => {
	const tabs = document.querySelectorAll(".search-category-tab");
	tabs.forEach((tab) => {
		const isActive = tab.dataset.category === selectedCategory;
		tab.classList.toggle("bg-white", isActive);
		tab.classList.toggle("text-gray-900", isActive);
		tab.classList.toggle("shadow-sm", isActive);
		tab.classList.toggle("text-gray-700", !isActive);
		tab.classList.toggle("hover:bg-gray-200", !isActive);
	});
};

const updateMethodCounts = async () => {
	const counts = { GET: 0, POST: 0, PUT: 0, DELETE: 0, OTHER: 0 };
	if (!allEndpointsIndex) return;
	const start = (currentPage - 1) * endpointPerPage;
	const end = start + endpointPerPage;
	currentPageEid = allEndpointsIndex.slice(start, end);
	const details = await fetchEndpointsByIndex(currentPageEid);
	details.forEach((ep) => {
		const m = ((ep && ep.method) || "").toUpperCase();
		if (counts[m] !== undefined) counts[m]++;
		else counts.OTHER++;
	});
	const allBtn = document.getElementById("count-all");
	const getBtn = document.getElementById("count-get");
	const postBtn = document.getElementById("count-post");
	const putBtn = document.getElementById("count-put");
	const deleteBtn = document.getElementById("count-delete");
	if (allBtn)
		allBtn.innerText =
			counts.GET + counts.POST + counts.PUT + counts.DELETE + counts.OTHER;
	if (getBtn) getBtn.innerText = counts.GET;
	if (postBtn) postBtn.innerText = counts.POST;
	if (putBtn) putBtn.innerText = counts.PUT;
	if (deleteBtn) deleteBtn.innerText = counts.DELETE;
};

const searchCategoryChange = (category) => {
	selectedCategory = category;
	updateCategoryTabStyles();
	applyCategoryFilter();
};

const applyCategoryFilter = async () => {
	if (selectedCategory === "ALL") {
		visibleEndpointsIndex = [...allEndpointsIndex];
	} else {
		// filter by reading per-eid files
		const start = (currentPage - 1) * endpointPerPage;
		const end = start + endpointPerPage;
		currentPageEid = allEndpointsIndex.slice(start, end);
		const details = await fetchEndpointsByIndex(currentPageEid);
		// const details = await fetchEndpointsByIndex(allEndpointsIndex);
		const filtered = [];
		details.forEach((d, idx) => {
			if (d && (d.method || "").toUpperCase() === selectedCategory) {
				filtered.push(allEndpointsIndex[idx]);
			}
		});
		visibleEndpointsIndex = filtered;
	}
	currentPage = 1;
	renderCurrentPage();
};

const getMethodBadgeClasses = (method) => {
	switch (method) {
		case "GET":
			return "bg-emerald-100 text-emerald-700 border border-emerald-200";
		case "POST":
			return "bg-amber-100 text-amber-700 border border-amber-200";
		case "PUT":
			return "bg-sky-100 text-sky-700 border border-sky-200";
		case "DELETE":
			return "bg-rose-100 text-rose-700 border border-rose-200";
		default:
			return "bg-gray-100 text-gray-700 border border-gray-200";
	}
};
let selectedEndpointEid = "";
// backup of visible endpoints before a live text search is applied
let _visibleEndpointsBeforeSearch = null;

const loadRQno = async (endpoint) => {
	let RQnoContainer = document.getElementById("rq-number");
	// RQnoContainer.classList.toggle("active", endpoint.qpPairs > 0);
	RQnoContainer.innerHTML = "";
	let RQnoTitle = document.createElement("h2");
	RQnoTitle.className =
		"text-lg font-bold text-gray-800 m-10 rotate-90 whitespace-nowrap";
	RQnoTitle.innerText = "QP PAIRS";
	RQnoContainer.appendChild(RQnoTitle);
	for (let i = 0; i < endpoint.qpPairs; i++) {
		let RQno = document.createElement("label");
		RQno.onclick = () => {
			document.querySelectorAll("#rq-number .rqno").forEach((item) => {
				item.classList.remove("active");
			});
			RQno.classList.add("active");
			loadreqPanel(endpoint.eid, i);
		};
		RQno.className =
			"p-2 rounded-full border-2 border-gray-300 text-black font-semibold text-lg	bg-white cursor-pointer  transition rqno";
		RQno.innerText = `QP ${i + 1}`;
		RQnoContainer.appendChild(RQno);
	}
};

const loadAnnotations = async (endpoint) => {
	const method = endpoint.method.toUpperCase();
	const methodBadgeClasses = getMethodBadgeClasses(method);

	const eidBox = document.getElementById("eid-box");
	const crudBox = document.getElementById("crud-box");
	const pathBox = document.getElementById("path-box");
	const annotationBox = document.getElementById("annotation-box");

	showDetailPlaceholder(false);
	if (eidBox) eidBox.innerText = endpoint.eid || "";
	if (crudBox)
		crudBox.innerHTML = `<span class="inline-block ${methodBadgeClasses} px-3 py-1 rounded uppercase font-bold">${method}</span>`;
	if (pathBox) pathBox.innerText = endpoint.path || "";
	if (annotationBox) annotationBox.innerText = endpoint.annotation || "";
};

const loadTags = async (endpoint) => {
	const tagsList = document.getElementById("tags-list");
	if (!tagsList) return;
	tagsList.innerHTML = "";

	(endpoint.tags || []).forEach((tag) => {
		const tagElement = document.createElement("span");
		tagElement.className =
			"inline-block bg-blue-200 text-blue-800 text-sm px-2 py-1 rounded-lg mr-2";
		tagElement.innerText = tag;
		tagsList.appendChild(tagElement);
	});
};

const loadreq = async (eid) => {
	try {
		const endpointData = await fetch(`./data/endpoints/${eid}.json`);
		const endpoint = await endpointData.json();

		selectedEndpointEid = eid;
		loadRQno(endpoint);
		loadAnnotations(endpoint);
		loadTags(endpoint);
		syncSelectedEndpointStyles();
	} catch (error) {
		console.error(`Error loading endpoint ${eid}:`, error);
		selectedEndpointEid = "";
		showDetailPlaceholder(true);
		syncSelectedEndpointStyles();
	}
};

// search box handler: search within the currently visible endpoints
const searchEndpoints = async () => {
	const input = document.querySelector('.search_box input[type="text"]');
	if (!input) return;
	const q = (input.value || "").trim().toLowerCase();

	// when starting a new search, keep a backup of the current visible endpoints
	if (q && _visibleEndpointsBeforeSearch === null) {
		_visibleEndpointsBeforeSearch = [...visibleEndpointsIndex];
	}

	// if query is empty, restore previous visibility (if any) and render
	if (!q) {
		if (_visibleEndpointsBeforeSearch !== null) {
			visibleEndpointsIndex = [..._visibleEndpointsBeforeSearch];
			_visibleEndpointsBeforeSearch = null;
		}
		currentPage = 1;
		renderCurrentPage();
		return;
	}

	// perform search across the currently visible endpoints (using cached per-eid details)
	const sourceList =
		_visibleEndpointsBeforeSearch !== null
			? _visibleEndpointsBeforeSearch
			: visibleEndpointsIndex;
	const details = await Promise.all(
		sourceList.map((it) => fetchEndpointDetail(it.eid)),
	);

	const filtered = [];
	for (let i = 0; i < details.length; i++) {
		const d = details[i];
		if (!d) continue;
		const hay = [
			d.eid || "",
			d.method || "",
			d.path || "",
			d.annotation || "",
			(d.tags || []).join(" "),
		]
			.join(" ")
			.toLowerCase();
		if (hay.indexOf(q) !== -1) {
			filtered.push(sourceList[i]);
		}
	}

	visibleEndpointsIndex = filtered;
	currentPage = 1;
	renderCurrentPage();
};
const clearSidebarPathFilters = () => {
	selectedPathFilters = [];
	document
		.querySelectorAll('.endpointsTags .content input[type="checkbox"]')
		.forEach((checkbox) => {
			checkbox.checked = false;
		});
};
// clear filters and search
const clearAllFilters = () => {
	selectedPathFilters = [];
	selectedCategory = "ALL";
	_visibleEndpointsBeforeSearch = null;
	clearSidebarPathFilters();
	updateCategoryTabStyles();
	visibleEndpointsIndex = [...allEndpointsIndex];
	currentPage = 1;
	renderCurrentPage();
};

const addStyleForEndpointcCard = (card, endpointStr, endpointID) => {
	// remove any existing SQP buttons from other cards
	document
		.querySelectorAll(".endpoint-card .sqp-view-btn")
		.forEach((b) => b.remove());

	if (endpointID === selectedEndpointEid) {
		card.classList.add("active");
		// avoid adding duplicate button on this card
		if (!card.querySelector(".sqp-view-btn")) {
			const btn = document.createElement("button");
			btn.className =
				"sqp-view-btn absolute bottom-3 right-3 inline-flex items-center gap-2 rounded-full border border-teal-500/30 bg-gradient-to-r from-teal-600 to-emerald-600 px-3 py-2 text-xs font-semibold text-white shadow-lg shadow-teal-900/10 transition-all duration-200 hover:-translate-y-0.5 hover:from-teal-500 hover:to-emerald-500 hover:shadow-xl focus:outline-none focus:ring-2 focus:ring-teal-400 focus:ring-offset-2 focus:ring-offset-gray-50";
			btn.type = "button";
			btn.setAttribute(
				"aria-label",
				`Open single endpoint view for ${endpointID}`,
			);
			btn.innerHTML = `
				<svg class="h-4 w-4 shrink-0" viewBox="0 0 20 20" fill="none" aria-hidden="true">
					<path d="M7.5 4.5H5A1.5 1.5 0 0 0 3.5 6v9A1.5 1.5 0 0 0 5 16.5h9A1.5 1.5 0 0 0 15.5 15v-2.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
					<path d="M9 11l7-7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
					<path d="M11.5 4H16v4.5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
				</svg>
				<span>SQP view</span>
			`;
			btn.addEventListener("click", () => {
				window.open(`./Single-EQP.html?eid=${endpointID}`, "_blank");
				// window.location.href = `http://127.0.0.1:5500/pid-webview/v1/Single-EQP.html?eid=${endpointID}`;
			});
			card.append(btn);
		}
	} else {
		card.classList.remove("active");
		const existing = card.querySelector(".sqp-view-btn");
		if (existing) existing.remove();
	}
};

const renderEndpoints = async (endpointsList) => {
	let endpointsContainer = document.getElementById("endpoints_scroll");
	endpointsContainer.innerHTML = "";

	console.time("rendering_time");

	const currentEndpoints = endpointsList || [];

	for (let i = 0; i < currentEndpoints.length; i++) {
		let endpointCard = document.createElement("div");
		let endpoint = currentEndpoints[i];

		// truncate annotation to max 4 words
		let words = endpoint.annotation.trim().split(/\s+/);
		let shortAnnotation =
			words.length > 4 ? words.slice(0, 4).join(" ") + "..." : words.join(" ");

		endpointCard.className =
			"endpoint-card w-full p-4 bg-gray-50 shadow-md. border-2 border-gray-200 rounded-lg flex flex-col gap-2 cursor-pointer hover:bg-gray-100 transition";
		endpointCard.dataset.eid = endpoint.eid;
		const method = endpoint.method.toUpperCase();
		const methodBadgeClasses = getMethodBadgeClasses(method);
		endpointCard.classList.toggle(
			"active",
			endpoint.eid === selectedEndpointEid,
		);

		endpointCard.innerHTML = `
			<div class="flex justify-between">
				<h1 class="rounded-sm px-2 py-1 uppercase font-bold ${methodBadgeClasses}">${endpoint.method.toLowerCase()}</h1>
				<h2>ID : ${endpoint.eid}</h2>
			</div>
			<h2 class="text-xl ">${endpoint.path}</h2>
			<h3 class="text-sm text-gray-600">${shortAnnotation}</h3>
		`;

		endpointsContainer.appendChild(endpointCard);

		endpointCard.addEventListener("click", () => {
			selectedEndpointEid = endpoint.eid;
			// syncSelectedEndpointStyles();
			addStyleForEndpointcCard(endpointCard, endpoint.path, endpoint.eid);
			loadreq(endpoint.eid);
		});
		endpointCard.addEventListener("dblclick", () => {
			window.location.href = `http://127.0.0.1:5500/pid-webview/v1/Single-EQP.html?eid=${endpoint.eid}`;
		});
	}
	console.timeEnd("rendering_time");
};

const loadMetaData = async () => {
	const response = await fetch("./data/meta-data.json");
	const metaData = await response.json();
	document.getElementById("total_tags").innerText = metaData.tags || 0;
	document.getElementById("total_segments").innerText = metaData.segments || 0;
	document.getElementById("total_endpoints").innerText =
		metaData.endpoints || 0;
	return metaData;
};

const updatePaginationInput = () => {
	const pageInput = document.getElementById("page-input");
	const maxPage = document.getElementById("max-page");
	pageInput.value = String(currentPage);
	pageInput.max = String(totalPages);
	maxPage.innerText = String(totalPages);
};

const syncPaginationState = () => {
	totalPages = Math.max(
		1,
		Math.ceil(visibleEndpointsIndex.length / endpointPerPage),
	);
	currentPage = Math.min(Math.max(currentPage, 1), totalPages);
};
const syncSelectedEndpointStyles = () => {
	document.querySelectorAll(".endpoint-card").forEach((card) => {
		card.classList.toggle("active", card.dataset.eid === selectedEndpointEid);
	});
};
const renderCurrentPage = async () => {
	syncPaginationState();
	const start = (currentPage - 1) * endpointPerPage;
	const end = start + endpointPerPage;
	const pageIndexEntries = visibleEndpointsIndex.slice(start, end);
	const pageDetails = await fetchEndpointsByIndex(pageIndexEntries);
	updateMethodCounts();
	await renderEndpoints(pageDetails);
	syncSelectedEndpointStyles();
	updatePaginationInput();
};

const goToPage = (requestedPage) => {
	const boundedPage = Math.min(Math.max(requestedPage, 1), totalPages);
	currentPage = boundedPage;
	renderCurrentPage();
};

const pagination = (metaData) => {
	// compute total pages from meta-data
	totalPages = Math.ceil(
		metaData && metaData.endpoints ? metaData.endpoints / endpointPerPage : 0,
	);

	const start = (currentPage - 1) * endpointPerPage;
	const end = start + endpointPerPage;
	const paginationContainer = document.querySelector(".pagination");

	const prevButton = document.getElementById("prev-button");
	const nextButton = document.getElementById("next-button");
	const pageInput = document.getElementById("page-input");

	pageInput.addEventListener("keydown", (event) => {
		if (event.key !== "Enter") {
			return;
		}

		const requestedPage = parseInt(pageInput.value.trim(), 10);

		if (Number.isNaN(requestedPage)) {
			updatePaginationInput();
			return;
		}

		goToPage(requestedPage);
	});

	pageInput.addEventListener("blur", () => {
		updatePaginationInput();
	});

	prevButton.addEventListener("click", () => {
		if (currentPage > 1) {
			goToPage(currentPage - 1);
		}
	});

	nextButton.addEventListener("click", () => {
		if (currentPage < totalPages) {
			goToPage(currentPage + 1);
		}
	});

	const max_page = document.querySelector("#max-page");
	max_page.textContent = totalPages;
};

loadMetaData().then((metaData) => {
	pagination(metaData);
	loadendpointsIndex();
});

const showDetailPlaceholder = (show) => {
	const placeholder = document.getElementById("detail-placeholder");
	const content = document.getElementById("detail-content");
	if (placeholder && content) {
		placeholder.classList.toggle("hidden", !show);
		content.classList.toggle("hidden", show);
	}
};

const toggleView = () => {
	const sidebar = document.querySelector(".endpointsTags");
	const mainContent = document.querySelector(".main-content");
	const endpoinDiv = document.getElementById("endpoints_card_container");
	const RRdiv = document.querySelector(".RRpanel");
	const DetailDiv = document.querySelector(".Detail");
	if (!sidebar || !mainContent) {
		return;
	}

	const isCollapsed = sidebar.classList.toggle("hidden");
	sidebar.classList.toggle("col-span-3", !isCollapsed);
	sidebar.classList.toggle("col-span-0", isCollapsed);
	if (sidebar) {
		sidebar.classList.toggle("overflow-hidden", isCollapsed);
	}
	mainContent.classList.toggle("grid-cols-16", !isCollapsed);
	mainContent.classList.toggle("grid-cols-13", isCollapsed);
	endpoinDiv.classList.toggle("col-span-5", !isCollapsed);
	endpoinDiv.classList.toggle("col-span-4", isCollapsed);
	RRdiv.classList.toggle("col-span-7", !isCollapsed);
	RRdiv.classList.toggle("col-span-8", isCollapsed);
	DetailDiv.classList.toggle("col-span-7", !isCollapsed);
	DetailDiv.classList.toggle("col-span-8", isCollapsed);
	const toggleButton = document.querySelector(".toggle-view");
	toggleButton.classList.toggle("hidden", !isCollapsed);
	toggleButton.classList.toggle("", isCollapsed);
};

const sideSearchBar = document.querySelector(".endpointsTags .content");

const loadSegments = async () => {
	const sidebar_headings = document.querySelector(".left-sidebar-headings");
	if (sidebar_headings) {
		sidebar_headings.children[0].classList.add("active");
		sidebar_headings.children[1].classList.remove("active");
	}
	if (sideSearchBar) {
		sideSearchBar.innerHTML = "";
		try {
			const segments = await fetch("./data/EndpointSegments_EID.json").then(
				(res) => res.json(),
			);
			for (const path in segments) {
				{
					let pathElement = document.createElement("div");
					pathElement.className =
						"inputGR flex items-center justify-between gap-4 w-full hover:bg-white/30 rounded-lg px-3 py-2 cursor-pointer mb-2 border-2 border-transparent";
					const checkboxId = `path-filter-${path}`;
					pathElement.innerHTML = `
									<div class="flex items-center gap-4">
						<input class= "accent-zinc-900 w-4 h-4" type="checkbox" name="path-filter" id="${checkboxId}" /> ${path}
						</div>
						<span class="text-xs text-muted-foreground">
							${segments[path].length}
						</span>`;

					const checkbox = pathElement.querySelector('input[type="checkbox"]');
					checkbox.addEventListener("click", (event) => {
						event.stopPropagation();
					});
					checkbox.addEventListener("change", () => {
						toggleSidebarPathFilter(path, checkbox.checked);
					});

					pathElement.addEventListener("click", () => {
						checkbox.checked = !checkbox.checked;
						toggleSidebarPathFilter(path, checkbox.checked);
						if (checkbox.checked) {
							pathElement.classList.add("active");
						} else {
							pathElement.classList.remove("active");
						}
					});

					sideSearchBar.appendChild(pathElement);
				}
			}
		} catch (error) {
			console.error("Error loading segments:", error);
		}
	}
};

const loadAllTags = async () => {
	const sidebar_headings = document.querySelector(".left-sidebar-headings");
	if (sidebar_headings) {
		sidebar_headings.children[1].classList.add("active");
		sidebar_headings.children[0].classList.remove("active");
	}
	if (sideSearchBar) {
		sideSearchBar.innerHTML = "";
		try {
			const tags = await fetch("./data/Tags_EID.json").then((res) =>
				res.json(),
			);
			for (const tag in tags) {
				let pathElement = document.createElement("div");
				pathElement.className =
					"inputGR flex items-center justify-between gap-4 w-full hover:bg-white/30 rounded-lg px-3 py-2 cursor-pointer border-2 border-transparent mb-2";
				const checkboxId = `path-filter-${tag}`;
				pathElement.innerHTML = `
									<div class="flex items-center gap-4">
						<input class= "accent-zinc-900 w-4 h-4" type="checkbox" name="path-filter" id="${checkboxId}" /> ${tag}
						</div>
						<span class="text-xs text-muted-foreground">${tags[tag].length}</span>`;

				const checkbox = pathElement.querySelector('input[type="checkbox"]');
				checkbox.addEventListener("click", (event) => {
					event.stopPropagation();
				});
				checkbox.addEventListener("change", () => {
					toggleSidebarPathFilter(tag, checkbox.checked);
				});

				pathElement.addEventListener("click", () => {
					checkbox.checked = !checkbox.checked;
					toggleSidebarPathFilter(tag, checkbox.checked);
					pathElement.classList.toggle("active", checkbox.checked);
				});

				sideSearchBar.appendChild(pathElement);
			}
		} catch (error) {
			console.error("Error loading tags:", error);
		}
	}
};
loadSegments();

const loadreqPanel = async (eid, rqIndex) => {
	console.log("Loading request panel for EID:", eid, "RQ Index:", rqIndex + 1);
	let responseD;
	try {
		responseD = await fetch(
			`./data/requests/${eid}-request${rqIndex + 1}.json`,
		).then((res) => res.json());
		console.log("Received request data:", responseD);
	} catch (error) {
		responseD = await fetch(`./data/requests/E0006-GL-request1.json`).then(
			(res) => res.json(),
		);
		console.log("loaded default request data due to error:", error);
	}

	const requestPanel = document.getElementById("request-panel");
	if (!requestPanel) {
		console.error("Request panel element not found");
		return;
	}

	requestPanel.innerHTML = `
	 <pre class="text-sm whitespace-pre-wrap break-words">${JSON.stringify(responseD.body, null, 2)}</pre>
	`;

	document.getElementById("size").innerText =
		`${responseD.meta_data.request_size}`;
	loadresPanel(eid, rqIndex);
};
const loadresPanel = async (eid, rqIndex) => {
	console.log("Loading response panel for EID:", eid, "RQ Index:", rqIndex + 1);
	let responseD;
	try {
		responseD = await fetch(
			`./data/responses/${eid}-response${rqIndex + 1}.json`,
		).then((res) => res.json());
		console.log("Received response data:", responseD);
	} catch (error) {
		responseD = await fetch(`./data/responses/E0006-GL-response1.json`).then(
			(res) => res.json(),
		);
		console.log("loaded default response data due to error:", error);
	}

	const responsePanel = document.getElementById("response-panel");
	if (!responsePanel) {
		console.error("Response panel element not found");
		return;
	}

	responsePanel.innerHTML = `
	 <pre class="text-sm whitespace-pre-wrap break-words">${JSON.stringify(responseD.body, null, 2)}</pre>
	`;
};

const toggleSidebarPathFilter = (path, isChecked) => {
	if (isChecked) {
		selectedPathFilters.push(path);
	} else {
		selectedPathFilters = selectedPathFilters.filter((p) => p !== path);
	}
	applyPathFilters();
};

const applyPathFilters = async () => {
	const EidSegmentsData = await fetch("./data/EndpointSegments_EID.json").then(
		(res) => res.json(),
	);
	if (selectedPathFilters.length === 0) {
		visibleEndpointsIndex = [...allEndpointsIndex];
	} else {
		const filteredEids = new Set();
		selectedPathFilters.forEach((filter) => {
			const matchingEids = EidSegmentsData[filter] || [];
			matchingEids.forEach((eid) => filteredEids.add(eid));
		});

		visibleEndpointsIndex = allEndpointsIndex.filter((entry) =>
			filteredEids.has(entry.eid),
		);
	}
	currentPage = 1;
	renderCurrentPage();
};
