let data = [];

const loadData = async () => {
	data = await fetch("./data/endpoints.json").then((response) =>
		response.json(),
	);
};

const qpPairsCounts = {
	TOTAL: 0,
	GET: 0,
	POST: 0,
	DELETE: 0,
	PUT: 0,
};

const EndpointCounts = {
	TOTAL: 0,
	GET: 0,
	POST: 0,
	DELETE: 0,
	PUT: 0,
};
const TagsCounts = {
	TOTAL: 0,
	GET: 0,
	POST: 0,
	DELETE: 0,
	PUT: 0,
};

const renderEnpointsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");

	data.forEach((endpoint) => {
		EndpointCounts.TOTAL += 1;
		if (endpoint.method in EndpointCounts) {
			EndpointCounts[endpoint.method] += 1;
		}
	});
	tr.addEventListener("click", () => {
		// console.log(EndpointCounts);
		renderChart(EndpointCounts);
	});
	tr.className =
		"hover:bg-gray-100 transition-colors border-t border-black/40 cursor-pointer";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">Endpoints</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									${EndpointCounts.TOTAL.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((EndpointCounts.GET / EndpointCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${EndpointCounts.GET.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((EndpointCounts.POST / EndpointCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${EndpointCounts.POST.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((EndpointCounts.DELETE / EndpointCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${EndpointCounts.DELETE.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((EndpointCounts.PUT / EndpointCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${EndpointCounts.PUT.toLocaleString()}
								</td>
    `;
	tableBody.appendChild(tr);
};

const renderEnpointsSegmentsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");
	const segments = await fetch("./data/AllEndpointSegments.json").then(
		(response) => response.json(),
	);
	console.log(segments);
	tr.className =
		"hover:bg-surface-container-low/50 transition-colors border-t border-black/40";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">Endpoints Segments</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									49
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									49
								</td>
    `;
	tableBody.appendChild(tr);
};
const renderEnpointsQPpairsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");
	tr.onclick = () => {
		console.log(qpPairsCounts);
		renderChart(qpPairsCounts);
	};
	const segments = await fetch("./data/AllEndpointSegments.json").then(
		(response) => response.json(),
	);
	console.log(segments);

	await data.forEach((endpoint) => {
		qpPairsCounts.TOTAL += endpoint.qpPairs;
		if (endpoint.method in qpPairsCounts) {
			qpPairsCounts[endpoint.method] += endpoint.qpPairs;
		}
	});

	// console.log(qpPairsCounts);
	tr.className =
		"hover:bg-gray-100 transition-colors border-t border-black/40 cursor-pointer";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">QP Pairs</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									${qpPairsCounts.TOTAL.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((qpPairsCounts.GET / qpPairsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${qpPairsCounts.GET}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((qpPairsCounts.POST / qpPairsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${qpPairsCounts.POST}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((qpPairsCounts.DELETE / qpPairsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${qpPairsCounts.DELETE}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((qpPairsCounts.PUT / qpPairsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${qpPairsCounts.PUT}
								</td>
    `;
	tableBody.appendChild(tr);
};
const renderEnpointstagsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");
	tr.onclick = () => {
		renderChart(TagsCounts);
	};
	await data.forEach((endpoint) => {
		TagsCounts.TOTAL += endpoint.tags.length;
		if (endpoint.method in TagsCounts) {
			TagsCounts[endpoint.method] += endpoint.tags.length;
		}
	});

	// console.log(TagsCounts);
	tr.className =
		"hover:bg-gray-100 transition-colors border-t border-black/40 cursor-pointer";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">Tags</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									${TagsCounts.TOTAL.toLocaleString()}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((TagsCounts.GET / TagsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${TagsCounts.GET}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((TagsCounts.POST / TagsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${TagsCounts.POST}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((TagsCounts.DELETE / TagsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${TagsCounts.DELETE}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									${((TagsCounts.PUT / TagsCounts.TOTAL) * 100).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${TagsCounts.PUT}
								</td>
    `;
	tableBody.appendChild(tr);
};
const renderRatioEnpointsQPpairsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");

	// console.log(qpPairsCounts);
	tr.className =
		"hover:bg-surface-container-low/50 transition-colors border-t border-black/40";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">QP Pairs / Endpoints</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									${(qpPairsCounts.TOTAL / EndpointCounts.TOTAL).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(qpPairsCounts.GET / EndpointCounts.GET).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(qpPairsCounts.POST / EndpointCounts.POST).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(qpPairsCounts.DELETE / EndpointCounts.DELETE).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(qpPairsCounts.PUT / EndpointCounts.PUT).toFixed(2)}
								</td>
    `;
	tableBody.appendChild(tr);
};
const renderRatioEnpointsTagsData = async () => {
	const tableBody = document.querySelector("tbody");
	const tr = document.createElement("tr");

	// console.log(qpPairsCounts);
	tr.className =
		"hover:bg-surface-container-low/50 transition-colors border-t border-black/40";
	tr.innerHTML = `
								<td class="py-3 px-4 font-bold text-on-surface" colspan="2">Tags / Endpoints</td>
								<td
									class="py-3 px-4 font-mono font-bold text-right text-primary"
                                    colspan="2"
								>
									${(TagsCounts.TOTAL / EndpointCounts.TOTAL).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(TagsCounts.GET / EndpointCounts.GET).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(TagsCounts.POST / EndpointCounts.POST).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(TagsCounts.DELETE / EndpointCounts.DELETE).toFixed(2)}
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right border-l border-outline-variant/10"
								>
									-
								</td>
								<td
									class="py-3 px-4 font-mono text-on-surface-variant text-right"
								>
									${(TagsCounts.PUT / EndpointCounts.PUT).toFixed(2)}
								</td>
    `;
	tableBody.appendChild(tr);
};

loadData().then(async () => {
	await renderEnpointsData();
	await renderEnpointsSegmentsData();
	await renderEnpointsQPpairsData();
	await renderEnpointstagsData();
	await renderRatioEnpointsQPpairsData();
	await renderRatioEnpointsTagsData();
	renderChart(EndpointCounts);
});

class BarChart {
	constructor(canvas, config = {}) {
		this.canvas =
			typeof canvas === "string" ? document.querySelector(canvas) : canvas;
		this.ctx = this.canvas.getContext("2d");
		this.labels = config.labels ?? [];
		this.datasets = this.normalizeDatasets(config);
		this.padding = config.padding ?? {
			top: 56,
			right: 56,
			bottom: 56,
			left: 64,
		};
		this.maxTicks = config.maxTicks ?? 5;
		this.labelColor = config.labelColor ?? "#6b7280";
		this.axisColor = config.axisColor ?? "rgba(0, 0, 0, 0.25)";
		this.gridColor = config.gridColor ?? "rgba(0, 0, 0, 0.08)";
		this.barGap = config.barGap ?? 0.2;
		this.barRadius = config.barRadius ?? 6;
		this.height = config.height ?? 420;
		this.width = config.width ?? null;
		this.valuePadding = config.valuePadding ?? 0.12;
		this.minBarHeight = config.minBarHeight ?? 2;
		this.legendColor = config.legendColor ?? "#111827";

		this.canvas.style.display = "block";
		this.canvas.style.width = "100%";
		this.canvas.style.height = `${this.height}px`;
		this.resize();
	}

	normalizeDatasets(config) {
		if (Array.isArray(config.datasets) && config.datasets.length > 0) {
			return config.datasets.map((dataset, index) => ({
				label: dataset.label ?? `Series ${index + 1}`,
				data: dataset.data ?? [],
				colors: dataset.colors ?? [dataset.color ?? "rgb(54, 162, 235)"],
				scaleMax: dataset.scaleMax ?? null,
				axis: dataset.axis ?? "left",
				formatter:
					dataset.formatter ??
					((value) =>
						Number.isInteger(value)
							? value.toLocaleString()
							: value.toFixed(1)),
			}));
		}

		if (Array.isArray(config.data)) {
			return [
				{
					label: config.datasetLabel ?? "Series 1",
					data: config.data,
					colors: config.colors ?? ["rgb(54, 162, 235)"],
					scaleMax: config.scaleMax ?? null,
					axis: config.axis ?? "left",
					formatter:
						config.formatter ??
						((value) =>
							Number.isInteger(value)
								? value.toLocaleString()
								: value.toFixed(1)),
				},
			];
		}

		return [];
	}

	resize() {
		const parentRect = this.canvas.parentElement?.getBoundingClientRect();
		const rect = this.canvas.getBoundingClientRect();
		const ratio = window.devicePixelRatio || 1;
		const displayWidth = this.width ?? parentRect?.width ?? rect.width ?? 640;
		const displayHeight =
			this.height ?? parentRect?.height ?? rect.height ?? 360;

		this.canvas.width = displayWidth * ratio;
		this.canvas.height = displayHeight * ratio;
		this.ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
		this.displayWidth = displayWidth;
		this.displayHeight = displayHeight;
	}

	clear() {
		this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
	}

	getSeriesScaleMax(series) {
		const dataMax = Math.max(...(series.data ?? []), 0);
		if (series.scaleMax && series.scaleMax > 0) {
			return series.scaleMax;
		}

		return dataMax > 0 ? dataMax * (1 + this.valuePadding) : 1;
	}

	formatAxisValue(value, isPercentage = false) {
		if (isPercentage) {
			return `${Math.round(value)}%`;
		}

		return Number.isInteger(value) ? value.toLocaleString() : value.toFixed(1);
	}

	draw() {
		const width =
			this.displayWidth ?? this.canvas.getBoundingClientRect().width;
		const height =
			this.displayHeight ?? this.canvas.getBoundingClientRect().height;
		const { top, right, bottom, left } = this.padding;
		const chartWidth = width - left - right;
		const chartHeight = height - top - bottom;
		const tickCount = Math.max(1, this.maxTicks);
		const hasRightAxis = this.datasets.length > 1;
		const leftSeries =
			this.datasets.find((dataset) => dataset.axis === "left") ??
			this.datasets[0];
		const rightSeries =
			this.datasets.find((dataset) => dataset.axis === "right") ??
			this.datasets[1] ??
			null;
		const leftScaleMax = this.getSeriesScaleMax(leftSeries);
		const rightScaleMax = rightSeries
			? this.getSeriesScaleMax(rightSeries)
			: null;

		this.clear();
		this.ctx.font = "12px sans-serif";
		this.ctx.textBaseline = "middle";
		this.ctx.lineWidth = 1;
		this.ctx.lineJoin = "round";

		if (this.datasets.length > 0) {
			const legendX = left;
			const legendY = Math.max(18, top - 28);
			let legendOffset = 0;

			this.datasets.forEach((dataset, index) => {
				const color = dataset.colors[0] ?? "rgb(54, 162, 235)";
				const textWidth = this.ctx.measureText(dataset.label).width;
				this.ctx.fillStyle = color;
				this.ctx.fillRect(legendX + legendOffset, legendY - 8, 14, 14);
				this.ctx.fillStyle = this.legendColor;
				this.ctx.textAlign = "left";
				this.ctx.textBaseline = "middle";
				this.ctx.fillText(dataset.label, legendX + legendOffset + 20, legendY);
				legendOffset += textWidth + 48;
			});
		}

		for (let i = 0; i <= tickCount; i += 1) {
			const tickRatio = i / tickCount;
			const leftTickValue = leftScaleMax * tickRatio;
			const rightTickValue = rightScaleMax ? rightScaleMax * tickRatio : null;
			const y = top + chartHeight - (chartHeight / tickCount) * i;

			this.ctx.strokeStyle = this.gridColor;
			this.ctx.beginPath();
			this.ctx.moveTo(left, y);
			this.ctx.lineTo(width - right, y);
			this.ctx.stroke();

			this.ctx.fillStyle = this.labelColor;
			this.ctx.textAlign = "right";
			this.ctx.fillText(this.formatAxisValue(leftTickValue), left - 10, y);

			if (hasRightAxis && rightTickValue !== null) {
				this.ctx.textAlign = "left";
				this.ctx.fillText(
					this.formatAxisValue(rightTickValue, true),
					width - right + 10,
					y,
				);
			}
		}

		this.ctx.strokeStyle = this.axisColor;
		this.ctx.beginPath();
		this.ctx.moveTo(left, top);
		this.ctx.lineTo(left, top + chartHeight);
		this.ctx.lineTo(width - right, top + chartHeight);
		if (hasRightAxis) {
			this.ctx.moveTo(width - right, top);
			this.ctx.lineTo(width - right, top + chartHeight);
		}
		this.ctx.stroke();

		if (!this.labels.length || !this.datasets.length) {
			return;
		}

		const slotWidth = chartWidth / this.labels.length;
		const groupWidth = slotWidth * 0.82;
		const gapWidth = Math.min(12, groupWidth * 0.12);
		const seriesCount = this.datasets.length;
		const barWidth =
			(seriesCount > 1
				? (groupWidth - gapWidth * (seriesCount - 1)) / seriesCount
				: groupWidth) || 1;
		const baseline = top + chartHeight;

		this.labels.forEach((label, labelIndex) => {
			const groupStartX =
				left + labelIndex * slotWidth + (slotWidth - groupWidth) / 2;

			this.datasets.forEach((dataset, datasetIndex) => {
				const value = dataset.data[labelIndex] ?? 0;
				const seriesScaleMax = this.getSeriesScaleMax(dataset);
				const barHeight =
					value <= 0
						? 0
						: Math.max(
								(value / seriesScaleMax) * chartHeight,
								this.minBarHeight,
							);
				const barX = groupStartX + datasetIndex * (barWidth + gapWidth);
				const barY = baseline - barHeight;
				const barColor = dataset.colors[labelIndex] ?? dataset.colors[0];

				this.ctx.fillStyle = barColor;
				this.ctx.fillRect(barX, barY, barWidth, barHeight);

				this.ctx.fillStyle = this.labelColor;
				this.ctx.textAlign = "center";
				this.ctx.textBaseline = "bottom";
				this.ctx.fillText(
					dataset.formatter(value),
					barX + barWidth / 2,
					Math.max(barY - 6, top + 10),
				);
			});

			this.ctx.fillStyle = this.labelColor;
			this.ctx.textAlign = "center";
			this.ctx.textBaseline = "top";
			this.ctx.fillText(
				label,
				left + labelIndex * slotWidth + slotWidth / 2,
				baseline + 12,
			);
		});
	}

	render() {
		this.resize();
		this.draw();
	}
}

const renderChart = (selectedCategory) => {
	const canvas = document.getElementById("myChart");

	const counts = [
		selectedCategory.GET,
		selectedCategory.POST,
		selectedCategory.DELETE,
		selectedCategory.PUT,
	];
	const total = selectedCategory.TOTAL || 1;
	const percentages = counts.map((value) => (value / total) * 100);
	const chartV = new BarChart(canvas, {
		labels: ["GET", "POST", "DELETE", "PUT"],
		datasets: [
			{
				label: "Counts",
				axis: "left",
				data: counts,
				scaleMax: Math.max(...counts, 1) * 1.15,
				colors: ["#40a02b", "#1e66f5", "#d20f39", "#fe640b"],
				formatter: (value) => value.toLocaleString(),
			},
			{
				label: "Percentage",
				axis: "right",
				data: percentages,
				scaleMax: 100,
				colors: ["#a6da95", "#8aadf4", "#ed8796", "#f5a97f"],
				formatter: (value) => `${value.toFixed(1)}%`,
			},
		],
		height: 620,
		padding: {
			top: 60,
			right: 64,
			bottom: 58,
			left: 68,
		},
		barGap: 0.18,
	});

	chartV.render();
};
