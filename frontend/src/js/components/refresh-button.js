(() => {
    "use strict";

    if (window.RefreshButton) return;

    // ------------------------------------------------------------
    // Styles
    // ------------------------------------------------------------
    const style = document.createElement("style");
    style.textContent = `
    [data-refresh-state="stale"] {
      border-color: #f59e0b !important;
      background: rgba(245, 158, 11, 0.15) !important;
      color: #b45309 !important;
    }
    .dark [data-refresh-state="stale"] { color: #fcd34d !important; }
    [data-refresh-state="busy"] { opacity: .7; cursor: progress; }
    [data-refresh-state="busy"] svg { animation: edms-spin 1s linear infinite; }
    @keyframes edms-spin { to { transform: rotate(360deg); } }
  `;
    document.head.appendChild(style);

    // ------------------------------------------------------------
    // Self-write tracking (so this page ignores its own echo)
    // ------------------------------------------------------------
    const SELF_WINDOW_MS = 2500;
    let pendingWrites = 0;
    let lastWriteDoneAt = 0;

    const nativeFetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
        const method = String(
            (init && init.method) || (input instanceof Request ? input.method : "GET"),
        ).toUpperCase();

        if (method === "GET" || method === "HEAD") {
            return nativeFetch(input, init);
        }

        pendingWrites++;
        try {
            return await nativeFetch(input, init);
        } finally {
            pendingWrites--;
            lastWriteDoneAt = Date.now();
        }
    };

    function isOwnEcho() {
        return pendingWrites > 0 || Date.now() - lastWriteDoneAt < SELF_WINDOW_MS;
    }

    // ------------------------------------------------------------
    // Component
    // ------------------------------------------------------------
    const instances = new Set();

    function init({ buttonId, onRefresh, watch = true, events = null, debug = false }) {
        const btn = document.getElementById(buttonId);
        if (!btn) return null;

        btn.type = "button";
        const idleTitle = "Refresh";
        btn.title = idleTitle;

        const dot = document.createElement("span");
        dot.className = "hidden h-2 w-2 shrink-0 rounded-full bg-amber-500 animate-pulse";
        btn.appendChild(dot);

        let busy = false;
        let ignoreUntil = 0;

        function setState(next) {
            if (next) btn.dataset.refreshState = next;
            else delete btn.dataset.refreshState;
            dot.classList.toggle("hidden", next !== "stale");
            btn.title = next === "stale" ? "Updates available. Click Refresh." : idleTitle;
        }

        function markStale() {
            if (busy || Date.now() < ignoreUntil) return;
            if (isOwnEcho()) return; // change came from this page
            setState("stale");
        }

        // Call after this page reloads its own data by any other route.
        function synced() {
            ignoreUntil = Date.now() + 1500;
            if (!busy) setState(null);
        }

        async function refresh() {
            if (busy) return;
            busy = true;
            setState("busy");
            try {
                await onRefresh();
            } catch (err) {
                console.error("Refresh failed:", err);
            } finally {
                busy = false;
                ignoreUntil = Date.now() + 1500; // ignore echo of our own reload
                setState(null);
            }
        }

        btn.addEventListener("click", (e) => {
            e.preventDefault();
            refresh();
        });

        if (watch) {
            const api = window.EdmsAPI;
            if (api && typeof api.connectEndpointLoader === "function") {
                try {
                    const ws = api.connectEndpointLoader();
                    ws.addEventListener("message", (e) => {
                        try {
                            const msg = JSON.parse(e.data);
                            if (msg.type !== "event" || !msg.event) return;

                            if (debug) {
                                console.log("[RefreshButton] event:", msg.event.type, msg.event);
                            }

                            // Only whitelisted event types can make the button stale.
                            if (!events || events.includes(msg.event.type)) markStale();
                        } catch (_) { }
                    });
                } catch (err) {
                    console.warn("Refresh watcher unavailable:", err);
                }
            }
        }

        const instance = { markStale, refresh, synced };
        instances.add(instance);
        return instance;
    }

    window.RefreshButton = {
        init,
        synced: () => instances.forEach((i) => i.synced()),
    };
})();