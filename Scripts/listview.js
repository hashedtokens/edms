const endpointDetailsCache = new Map();

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
	const details = await Promise.all(
		indexArray.map((it) => fetchEndpointDetail(it.eid)),
	);
	return details;
};

const loadendpointsIndex = async () => {
	const endpointsIndex = await fetch("./data/EndpointIndex.json").then((res) =>
		res.json(),
	);
	allEndpointsIndex = endpointsIndex;
	visibleEndpointsIndex = [...endpointsIndex];
	updateCategoryTabStyles();
	void updateCategoryCounts(visibleEndpointsIndex);
	renderCurrentPage();
};

console.log("loading meta Data ...");

const endpointPP = 500;
let totalPages = 0; // will be computed from meta-data
let currentPage = 1;
let visibleEndpointsIndex = [];
let allEndpointsIndex = [];
let selectedPathFilters = [];
let selectedCategory = "ALL";
let renderToken = 0;

const getSearchInputValue = () => {
	const input = document.querySelector('main input[type="text"]');
	return (input && input.value ? input.value : "").trim().toLowerCase();
};

const updateCategoryTabStyles = () => {
	document.querySelectorAll(".search-category-tab").forEach((tab) => {
		const isActive = tab.dataset.category === selectedCategory;
		tab.classList.toggle("bg-white", isActive);
		tab.classList.toggle("text-gray-900", isActive);
		tab.classList.toggle("shadow-sm", isActive);
		tab.classList.toggle("text-gray-700", !isActive);
		tab.classList.toggle("hover:bg-gray-200", !isActive);
	});
};

const updateCategoryCounts = async (indexList = visibleEndpointsIndex) => {
	const counts = { ALL: 0, GET: 0, POST: 0, PUT: 0, DELETE: 0 };
	const sourceList =
		allEndpointsIndex && allEndpointsIndex.length
			? allEndpointsIndex
			: indexList;
	if (!sourceList || sourceList.length === 0) {
		const allBtn = document.getElementById("count-all");
		const getBtn = document.getElementById("count-get");
		const postBtn = document.getElementById("count-post");
		const putBtn = document.getElementById("count-put");
		const deleteBtn = document.getElementById("count-delete");
		if (allBtn) allBtn.innerText = "0";
		if (getBtn) getBtn.innerText = "0";
		if (postBtn) postBtn.innerText = "0";
		if (putBtn) putBtn.innerText = "0";
		if (deleteBtn) deleteBtn.innerText = "0";
		return;
	}

	const details = await fetchEndpointsByIndex(sourceList);
	details.forEach((endpoint) => {
		const method = ((endpoint && endpoint.method) || "").toUpperCase();
		if (counts[method] !== undefined) {
			counts[method]++;
		}
	});
	counts.ALL = sourceList.length;

	const allBtn = document.getElementById("count-all");
	const getBtn = document.getElementById("count-get");
	const postBtn = document.getElementById("count-post");
	const putBtn = document.getElementById("count-put");
	const deleteBtn = document.getElementById("count-delete");
	if (allBtn) allBtn.innerText = counts.ALL;
	if (getBtn) getBtn.innerText = counts.GET;
	if (postBtn) postBtn.innerText = counts.POST;
	if (putBtn) putBtn.innerText = counts.PUT;
	if (deleteBtn) deleteBtn.innerText = counts.DELETE;
};

const applyAllFilters = async () => {
	let filteredIndex = [...allEndpointsIndex];

	if (selectedPathFilters.length > 0) {
		const segmentsData = await fetch("./data/EndpointSegments_EID.json").then(
			(res) => res.json(),
		);
		const filteredEids = new Set();
		selectedPathFilters.forEach((filter) => {
			const matchingEids = segmentsData[filter] || [];
			matchingEids.forEach((eid) => filteredEids.add(eid));
		});
		filteredIndex = filteredIndex.filter((entry) =>
			filteredEids.has(entry.eid),
		);
	}

	const query = getSearchInputValue();
	const needsDetailMatch = selectedCategory !== "ALL" || query;
	if (needsDetailMatch && filteredIndex.length > 0) {
		const details = await fetchEndpointsByIndex(filteredIndex);
		const refinedIndex = [];
		for (let i = 0; i < details.length; i++) {
			const endpoint = details[i];
			if (!endpoint) continue;

			const method = (endpoint.method || "").toUpperCase();
			if (selectedCategory !== "ALL" && method !== selectedCategory) {
				continue;
			}

			if (query) {
				const haystack = [
					endpoint.eid || "",
					endpoint.method || "",
					endpoint.path || "",
					endpoint.annotation || "",
					(endpoint.tags || []).join(" "),
				]
					.join(" ")
					.toLowerCase();
				if (haystack.indexOf(query) === -1) {
					continue;
				}
			}

			refinedIndex.push(filteredIndex[i]);
		}
		filteredIndex = refinedIndex;
	}

	visibleEndpointsIndex = filteredIndex;
	currentPage = 1;
	updateCategoryTabStyles();
	void updateCategoryCounts(filteredIndex);
	renderCurrentPage();
};

const getMethodBadgeClasses = (method) => {
	switch (method) {
		case "GET":
			return "bg-emerald-100 text-emerald-700 ";
		case "POST":
			return "bg-amber-100 text-amber-700 ";
		case "PUT":
			return "bg-sky-100 text-sky-700 ";
		case "DELETE":
			return "bg-rose-100 text-rose-700 ";
		default:
			return "bg-gray-100 text-gray-700 ";
	}
};

const renderEndpoints = async (endpointsList) => {
	const tableBody = document.getElementById("endpointsTableBody");
	tableBody.innerHTML = "";
	const activeToken = renderToken;

	const start = (currentPage - 1) * endpointPP;
	const end = start + endpointPP;

	console.time("rendering_time");

	// get current page EIDs from visible endpoints
	const currentEndpoints = (endpointsList || visibleEndpointsIndex).slice(
		start,
		end,
	);

	// fetch ALL in parallel using cache
	const endpointData = await fetchEndpointsByIndex(currentEndpoints);
	if (activeToken !== renderToken) return;

	// use fragment for faster DOM rendering
	const fragment = document.createDocumentFragment();

	endpointData.forEach((endpoint) => {
		if (!endpoint) return;
		const annotation = endpoint.annotation || "";
		const words = annotation.trim().split(/\s+/).filter(Boolean);
		const shortAnnotation =
			words.length > 4 ? words.slice(0, 6).join(" ") + "..." : words.join(" ");
		const row = document.createElement("tr");
		row.className = "hover:bg-gray-50 cursor-pointer";

		const eidCell = document.createElement("td");
		eidCell.className =
			"text-gray-500 hover:text-yellow-800 hover:bg-yellow-100";
		eidCell.textContent = endpoint.eid || "";

		const methodCell = document.createElement("td");
		methodCell.className = "px-4 py-2";
		const methodBadge = document.createElement("span");
		methodBadge.className = `inline-block px-2 py-1 text-md font-semibold rounded-full ${getMethodBadgeClasses(endpoint.method)}`;
		methodBadge.textContent = endpoint.method || "";
		methodCell.appendChild(methodBadge);

		const pathCell = document.createElement("td");
		pathCell.textContent = endpoint.path || "";

		const annotationCell = document.createElement("td");
		annotationCell.className = "text-wrap";
		annotationCell.textContent = shortAnnotation;

		const qpCell = document.createElement("td");
		qpCell.textContent = String(endpoint.qpPairs ?? "");

		const tagsCell = document.createElement("td");
		tagsCell.textContent = Array.isArray(endpoint.tags)
			? endpoint.tags.join(", ")
			: "";

		const statusCell = document.createElement("td");
		statusCell.textContent = String(endpoint.status ?? "");

		row.append(
			eidCell,
			methodCell,
			pathCell,
			annotationCell,
			qpCell,
			tagsCell,
			statusCell,
		);
		fragment.appendChild(row);
		row.addEventListener("click", () => {
			openSQP(endpoint.eid);
		});
	});

	if (activeToken !== renderToken) return;
	tableBody.appendChild(fragment);
	console.timeEnd("rendering_time");
};

const openSQP = (eid) => {
	window.open(`./Single-EQP.html?eid=${eid}`, "_blank");
};

const syncPaginationState = () => {
	totalPages = Math.max(
		1,
		Math.ceil(visibleEndpointsIndex.length / endpointPP),
	);
	currentPage = Math.min(Math.max(currentPage, 1), totalPages);
};

const renderCurrentPage = async () => {
	renderToken += 1;
	syncPaginationState();
	await renderEndpoints(visibleEndpointsIndex);
	updatePaginationDisplay();
	// update the max-page display
	const max_page = document.querySelector("#max-page");
	if (max_page) {
		max_page.textContent = totalPages;
	}
};

const updatePaginationDisplay = () => {
	const start = (currentPage - 1) * endpointPP;
	const end = Math.min(start + endpointPP, visibleEndpointsIndex.length);
	const paginationContainer = document.querySelector(".pagination");
	if (paginationContainer) {
		paginationContainer.textContent = `showing ${visibleEndpointsIndex.length > 0 ? start + 1 : 0} to ${end} of ${visibleEndpointsIndex.length} results`;
	}
};

const pagination = (metaData) => {
	// compute total pages from meta-data
	totalPages = Math.ceil(
		metaData && metaData.endpoints ? metaData.endpoints / endpointPP : 0,
	);

	const prevButton = document.querySelector("#prev-button");
	prevButton.onclick = () => {
		if (currentPage > 1) {
			currentPage--;
			renderCurrentPage();
		}
	};

	const nextButton = document.querySelector("#next-button");
	nextButton.onclick = () => {
		if (currentPage < totalPages) {
			currentPage++;
			renderCurrentPage();
		}
	};

	const page_input = document.querySelector("#page-input");
	page_input.value = currentPage;
	page_input.onchange = (e) => {
		const page = parseInt(e.target.value);
		if (page >= 1 && page <= totalPages) {
			currentPage = page;
			renderCurrentPage();
		}
	};

	const max_page = document.querySelector("#max-page");
	max_page.textContent = totalPages;
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

loadMetaData().then((metaData) => {
	pagination(metaData);
	loadendpointsIndex();
});

const toggleView = () => {
	const sidebar = document.querySelector(".endpointsTags");
	const leftPanel = document.querySelector(".left");
	const rightPanel = document.querySelector(".right");

	if (!sidebar || !leftPanel || !rightPanel) {
		return;
	}

	const isCollapsed = sidebar.classList.toggle("hidden");
	sidebar.classList.toggle("w-0", isCollapsed);
	sidebar.classList.toggle("w-full", !isCollapsed);
	sidebar.classList.toggle("overflow-hidden", isCollapsed);

	leftPanel.classList.toggle("w-1/4", !isCollapsed);
	leftPanel.classList.toggle("w-0", isCollapsed);
	leftPanel.classList.toggle("px-2", !isCollapsed);
	leftPanel.classList.toggle("px-0", isCollapsed);
	leftPanel.classList.toggle("min-w-0", isCollapsed);
	leftPanel.classList.toggle("overflow-hidden", isCollapsed);
	leftPanel.classList.toggle("hidden", isCollapsed);

	rightPanel.classList.toggle("w-3/4", !isCollapsed);
	rightPanel.classList.toggle("w-full", isCollapsed);

	const floatingToggleButton = document.querySelector(".toggle-view.absolute");
	if (floatingToggleButton) {
		floatingToggleButton.classList.toggle("hidden", !isCollapsed);
	}
};

// search box handler: search within the currently visible endpoints
const searchEndpoints = async () => {
	await applyAllFilters();
};

const clearSidebarPathFilters = () => {
	selectedPathFilters = [];
	document
		.querySelectorAll('.left .endpointsTags .content input[type="checkbox"]')
		.forEach((checkbox) => {
			checkbox.checked = false;
		});
};

// clear filters and search
const clearAllFilters = () => {
	selectedPathFilters = [];
	selectedCategory = "ALL";
	clearSidebarPathFilters();
	updateCategoryTabStyles();
	const input = document.querySelector('main input[type="text"]');
	if (input) input.value = "";
	visibleEndpointsIndex = [...allEndpointsIndex];
	currentPage = 1;
	void updateCategoryCounts(visibleEndpointsIndex);
	renderCurrentPage();
};

const searchCategoryChange = async (category) => {
	selectedCategory = category;
	await applyAllFilters();
};

const applyPathFilters = async () => {
	await applyAllFilters();
};

// const loadSegments = async () => {
// 	const sideSearchBar = document.querySelector(".left .endpointsTags .content");
// 	if (sideSearchBar) {
// 		sideSearchBar.innerHTML = "";
// 		paths = await fetch("./data/AllEndpointSegments.json").then((res) =>
// 			res.json(),
// 		);
// 		paths.forEach((path) => {
// 			let pathElement = document.createElement("div");
// 			pathElement.className =
// 				"inputGR flex items-center justify-between gap-4 w-full hover:bg-white/30 rounded-lg px-3 py-2 cursor-pointer";
// 			const checkboxId = `path-filter-${path}`;
// 			pathElement.innerHTML = `
// 										<div class="flex items-center gap-4">
// 							<input class= "accent-zinc-900 w-4 h-4" type="checkbox" name="path-filter" id="${checkboxId}" /> ${path}
// 							</div>
// 							<span class=""></span>`;

// 			const checkbox = pathElement.querySelector('input[type="checkbox"]');
// 			checkbox.addEventListener("click", (event) => {
// 				event.stopPropagation();
// 			});
// 			checkbox.addEventListener("change", () => {
// 				toggleSidebarPathFilter(path, checkbox.checked);
// 			});

// 			pathElement.addEventListener("click", () => {
// 				checkbox.checked = !checkbox.checked;
// 				toggleSidebarPathFilter(path, checkbox.checked);
// 			});

// 			sideSearchBar.appendChild(pathElement);
// 		});
// 	}
// };
const loadSegments = async () => {
	const sideSearchBar = document.querySelector(".left .endpointsTags .content");

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
						pathElement.classList.toggle("active", checkbox.checked);
					});

					sideSearchBar.appendChild(pathElement);
				}
			}
		} catch (error) {
			console.error("Error loading segments:", error);
		}
	}
};
// const loadAllTags = async () => {
// 	const sideSearchBar = document.querySelector(".left .endpointsTags .content");
// 	if (sideSearchBar) {
// 		sideSearchBar.innerHTML = "";
// 		paths = await fetch("./data/AllTags.json").then((res) => res.json());
// 		paths.forEach((path) => {
// 			let pathElement = document.createElement("div");
// 			pathElement.className =
// 				"inputGR flex items-center justify-between gap-4 w-full hover:bg-white/30 rounded-lg px-3 py-2 cursor-pointer";
// 			const checkboxId = `path-filter-${path}`;
// 			pathElement.innerHTML = `
// 										<div class="flex items-center gap-4">
// 							<input class= "accent-zinc-900 w-4 h-4" type="checkbox" name="path-filter" id="${checkboxId}" /> ${path}
// 							</div>
// 							<span class=""></span>`;

// 			const checkbox = pathElement.querySelector('input[type="checkbox"]');
// 			checkbox.addEventListener("click", (event) => {
// 				event.stopPropagation();
// 			});
// 			checkbox.addEventListener("change", () => {
// 				toggleSidebarPathFilter(path, checkbox.checked);
// 			});

// 			pathElement.addEventListener("click", () => {
// 				checkbox.checked = !checkbox.checked;
// 				toggleSidebarPathFilter(path, checkbox.checked);
// 			});

// 			sideSearchBar.appendChild(pathElement);
// 		});
// 	}
// };
const loadAllTags = async () => {
	const sideSearchBar = document.querySelector(".left .endpointsTags .content");
	if (sideSearchBar) {
		sideSearchBar.innerHTML = "";
		try {
			const tags = await fetch("./data/Tags_EID.json").then((res) =>
				res.json(),
			);
			for (const tag in tags) {
				let pathElement = document.createElement("div");
				pathElement.className =
					"inputGR flex items-center justify-between gap-4 w-full hover:bg-white/30 rounded-lg px-3 py-2 cursor-pointer";
				const checkboxId = `path-filter-${tag}`;
				pathElement.innerHTML = `
									<div class="flex items-center gap-4">
						<input class= "accent-zinc-900 w-4 h-4" type="checkbox" name="path-filter" id="${checkboxId}" /> ${tag}
						</div>
						<span class="text-xs text-muted-foreground">${tags[tag].length} </span>`;

				const checkbox = pathElement.querySelector('input[type="checkbox"]');
				checkbox.addEventListener("click", (event) => {
					event.stopPropagation();
				});
				checkbox.addEventListener("change", () => {
					toggleSidebarPathFilter(tag, checkbox.checked);
				});

				pathElement.addEventListener("click", () => {
					checkbox.checked = !checkbox.checked;
					console.log(
						"toggled tag filter for",
						tag,
						"checked:",
						checkbox.checked,
					);
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

const toggleSidebarPathFilter = (path, isChecked) => {
	if (isChecked) {
		if (!selectedPathFilters.includes(path)) {
			selectedPathFilters.push(path);
		}
	} else {
		selectedPathFilters = selectedPathFilters.filter((p) => p !== path);
	}
	void applyAllFilters();
};
