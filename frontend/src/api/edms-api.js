// ============================================================
// EDMS API
// BACKEND COMMUNICATION LAYER
// ============================================================

(() => {
    'use strict';


    // ============================================================
    // CONFIG
    // ============================================================

    if (typeof window.showToast !== 'function') {
        window.showToast = function(message, type = 'info') {
            let container = document.getElementById('edms-toast-container');
            if (!container) {
                container = document.createElement('div');
                container.id = 'edms-toast-container';
                container.style.cssText = 'position:fixed;top:24px;right:24px;z-index:999999;display:flex;flex-direction:column;gap:10px;pointer-events:none;';
                document.body.appendChild(container);
            }
            const toast = document.createElement('div');
            const bg = type === 'warning' ? '#d97706' : (type === 'error' ? '#dc2626' : '#2563eb');
            const icon = type === 'warning' ? '⚠️ ' : (type === 'error' ? '❌ ' : 'ℹ️ ');
            toast.style.cssText = `background:${bg};color:#ffffff;padding:12px 20px;border-radius:8px;font-size:14px;font-family:sans-serif;font-weight:500;box-shadow:0 8px 24px rgba(0,0,0,0.35);opacity:0;transform:translateY(-12px);transition:all 0.3s cubic-bezier(0.16, 1, 0.3, 1);pointer-events:auto;display:flex;align-items:center;gap:10px;`;
            toast.textContent = `${icon}${message}`;
            container.appendChild(toast);
            requestAnimationFrame(() => {
                toast.style.opacity = '1';
                toast.style.transform = 'translateY(0)';
            });
            setTimeout(() => {
                toast.style.opacity = '0';
                toast.style.transform = 'translateY(-12px)';
                setTimeout(() => toast.remove(), 350);
            }, 4500);
        };
    }

    const API_BASE = 'http://localhost:3000';
    const WS_BASE = 'ws://localhost:3000';


    // ============================================================
    // HTTP HELPER
    // ============================================================

    async function http(
        method,
        path,
        body
    ) {

        const options = {
            method,

            headers: {
                'Content-Type': 'application/json'
            }
        };


        if (body !== undefined) {

            options.body =
                JSON.stringify(body);

        }


        const response =
            await fetch(
                `${API_BASE}${path}`,
                options
            );


        const text =
            await response.text();


        let data;


        try {

            data =
                text
                    ? JSON.parse(text)
                    : null;

        } catch {

            data =
                text;

        }


        return {

            status:
                response.status,

            ok:
                response.ok,

            data

        };

    }


    // ============================================================
    // WEBSOCKET HELPER
    // ============================================================

    function createWebSocket(
        path,
        options = {}
    ) {

        return new WebSocket(
            `${WS_BASE}${path}`,
            options.protocols
        );

    }


    // ============================================================
    // R1 — ENDPOINTS
    // ============================================================

    async function registerEndpoint(
        endpointId,
        endpointUrl,
        method,
        annotation
    ) {

        const body = {

            endpoint_id:
                endpointId,

            endpoint_str:
                endpointUrl

        };


        if (method !== undefined) {

            body.method =
                method;

        }


        if (annotation !== undefined) {

            body.annotation =
                annotation;

        }


        return http(
            'POST',
            '/endpoints/create',
            body
        );

    }


    // ============================================================
    // ONE-TIME BULK REGISTRATION UTILITY
    // ============================================================

    async function registerAllEndpoints(
        endpointList
    ) {

        const results = [];


        for (
            const endpoint of endpointList
        ) {

            const url =
                (endpoint.baseUrl || '') +
                (endpoint.endpoint || '');


            const result =
                await registerEndpoint(
                    endpoint.id,
                    url,
                    endpoint.method,
                    endpoint.annotation
                );


            results.push({

                id:
                    endpoint.id,

                status:
                    result.status,

                ok:
                    result.ok

            });


            console.log(
                `Registered ${endpoint.id}:`,
                result.status
            );

        }


        return results;

    }


    // ============================================================
    // DELETE ENDPOINT
    // ============================================================

    async function deleteEndpoint(
        endpointId
    ) {

        return http(
            'POST',
            `/endpoints/${encodeURIComponent(endpointId)}/delete`
        );

    }


    // ============================================================
    // UPDATE ENDPOINT ANNOTATION
    // ============================================================

    async function setEndpointAnnotation(
        endpointId,
        annotation
    ) {

        return http(
            'POST',
            `/endpoints/${encodeURIComponent(endpointId)}/annotation`,
            {
                annotation
            }
        );

    }


    // ============================================================
    // STATIC VIEW METADATA
    // ============================================================

    async function getHome() {

        return http(
            'GET',
            '/home'
        );

    }


    async function getTestView() {

        return http(
            'GET',
            '/test-view'
        );

    }


    async function getListView() {

        return http(
            'GET',
            '/list-view'
        );

    }


    // ============================================================
    // TEST VIEW — WEBSOCKETS
    // ============================================================

    function connectTestView() {

        return createWebSocket(
            '/test-view/run'
        );

    }


    function connectEndpointLoader() {

        return createWebSocket(
            '/test-view/endpoints/load'
        );

    }


    function connectBookmarkLoader(collection) {

        return createWebSocket(
            `/test-view/${encodeURIComponent(collection)}/bookmarks/load`
        );

    }


    function connectHistoryLoader() {

        return createWebSocket(
            '/test-view/history/load'
        );

    }


    // ============================================================
    // TEST VIEW — START TEST
    // ============================================================

    function startTest(
        ws,
        endpointId,
        endpointStr,
        method,
        body = {},
        timeoutMs = 30000,
        tickIntervalMs = 500,
        headers,
        annotation
    ) {

        const payload = {

            endpoint_str:
                endpointStr,

            method:
                method,

            body:
                body,

            timeout_ms:
                timeoutMs,

            tick_interval_ms:
                tickIntervalMs

        };


        // endpoint_id became optional in the
        // backend update from 2026-09-08.

        if (
            endpointId !== undefined &&
            endpointId !== null &&
            endpointId !== ''
        ) {

            payload.endpoint_id =
                endpointId;

        }


        if (
            headers !== undefined
        ) {

            payload.headers =
                headers;

        }


        if (
            annotation !== undefined
        ) {

            payload.annotation =
                annotation;

        }


        ws.send(
            JSON.stringify({

                type:
                    'run_test',

                payload

            })
        );

    }


    // ============================================================
    // TEST VIEW — WAIT FOR TEST EVENTS
    // ============================================================

    function waitForTestFinished(
        ws,
        handlers = {}
    ) {

        return new Promise(
            (
                resolve,
                reject
            ) => {

                function handleMessage(
                    wsEvent
                ) {

                    try {

                        const message =
                            JSON.parse(
                                wsEvent.data
                            );


                        const event =
                            message.event;


                        if (!event) {

                            return;

                        }


                        // ----------------------------------------
                        // Test Started
                        // ----------------------------------------

                        if (
                            event.type ===
                            'DuplicateQpBlocked'
                        ) {
                            console.warn(
                                '%c[DUPLICATE DETECTED] Duplicate request detected! Backend skipped saving redundant copy to disk.',
                                'color: #ff9800; font-weight: bold; font-size: 14px;',
                                event
                            );
                            const origQp = event.payload?.duplicate_of ?? event.payload?.request_number ?? event.duplicate_of ?? event.request_number ?? 1;
                            if (typeof window.showToast === 'function') {
                                window.showToast(`Duplicate request detected. Existing QP #${origQp} selected.`, 'warning');
                            }
                        }

                        if (
                            event.type ===
                            'TestStarted'
                        ) {

                            if (
                                typeof handlers.onStarted ===
                                'function'
                            ) {

                                handlers.onStarted(
                                    event
                                );

                            }

                            return;

                        }


                        // ----------------------------------------
                        // Timer Tick
                        // ----------------------------------------

                        if (
                            event.type ===
                            'TimerTick'
                        ) {

                            if (
                                typeof handlers.onTick ===
                                'function'
                            ) {

                                handlers.onTick(
                                    event
                                );

                            }

                            return;

                        }


                        // ----------------------------------------
                        // Test Finished
                        // ----------------------------------------

                        if (
                            event.type ===
                            'TestFinished'
                        ) {

                            cleanup();


                            if (
                                typeof handlers.onFinished ===
                                'function'
                            ) {

                                handlers.onFinished(
                                    event
                                );

                            }


                            resolve(
                                event
                            );


                            return;

                        }


                        // ----------------------------------------
                        // Test Timeout
                        // ----------------------------------------

                        if (
                            event.type ===
                            'TestTimeout'
                        ) {

                            cleanup();


                            if (
                                typeof handlers.onTimeout ===
                                'function'
                            ) {

                                handlers.onTimeout(
                                    event
                                );

                            }


                            reject(
                                new Error(
                                    'Backend reported TestTimeout'
                                )
                            );


                            return;

                        }


                        // ----------------------------------------
                        // Backend Error
                        // ----------------------------------------

                        if (
                            event.type ===
                            'Error'
                        ) {

                            cleanup();


                            if (
                                typeof handlers.onError ===
                                'function'
                            ) {

                                handlers.onError(
                                    event
                                );

                            }


                            reject(
                                new Error(
                                    event.payload?.message ||
                                    'Backend returned Error'
                                )
                            );


                            return;

                        }

                    } catch (error) {

                        cleanup();

                        reject(
                            error
                        );

                    }

                }


                function handleError(
                    error
                ) {

                    cleanup();

                    reject(
                        error
                    );

                }


                function cleanup() {

                    ws.removeEventListener(
                        'message',
                        handleMessage
                    );


                    ws.removeEventListener(
                        'error',
                        handleError
                    );

                }


                ws.addEventListener(
                    'message',
                    handleMessage
                );


                ws.addEventListener(
                    'error',
                    handleError
                );

            }
        );

    }


    // ============================================================
    // TEST VIEW — REQUEST
    // ============================================================

    async function fetchRequest(
        endpointId,
        requestNumber
    ) {

        return http(
            'GET',
            `/test-view/` +
            `${encodeURIComponent(endpointId)}` +
            `/request/` +
            `${encodeURIComponent(requestNumber)}`
        );

    }


    // ============================================================
    // TEST VIEW — RESPONSE
    // ============================================================

    async function fetchResponse(
        endpointId,
        requestNumber
    ) {

        return http(
            'GET',
            `/test-view/` +
            `${encodeURIComponent(endpointId)}` +
            `/response/` +
            `${encodeURIComponent(requestNumber)}`
        );

    }


    // ============================================================
    // TEST VIEW — HEADERS
    // ============================================================

    async function fetchHeaders(
        endpointId,
        requestNumber
    ) {

        return http(
            'GET',
            `/test-view/` +
            `${encodeURIComponent(endpointId)}` +
            `/headers/` +
            `${encodeURIComponent(requestNumber)}`
        );

    }


    // ============================================================
    // TEST VIEW — STOP TEST
    // ============================================================

    async function stopTest(
        endpointId,
        requestNumber
    ) {

        return http(
            'POST',
            '/test-view/stop',
            {
                endpoint_id:
                    endpointId,

                request_number:
                    requestNumber
            }
        );

    }


    // ============================================================
    // TEST VIEW — HISTORY
    // ============================================================

    async function saveHistory(
        endpointId,
        action,
        details
    ) {

        return http(
            'POST',
            '/test-view/save/history',
            {
                endpoint_id:
                    endpointId,

                action,

                details:
                    details === undefined ||
                    details === null
                        ? undefined
                        : typeof details === 'string'
                            ? details
                            : JSON.stringify(details)
            }
        );

    }


    async function clearHistory() {

        return http(
            'POST',
            '/test-view/history/clearall'
        );

    }


    // ============================================================
    // TEST VIEW — LEGACY BOOKMARK
    // ============================================================

    async function saveBookmark(
        endpointId,
        notes,
        collection
    ) {

        return http(
            'POST',
            '/test-view/save/bookmark',
            {
                endpoint_id:
                    endpointId,

                notes,

                collection
            }
        );

    }


    async function clearBookmarks(collection) {

        return http(
            'POST',
            `/test-view/${encodeURIComponent(collection)}/bookmark/clearall`
        );

    }


    // ============================================================
    // TEST VIEW — ACTIVE BOOKMARK
    // ============================================================

    function addActiveBookmark(
        collection,
        endpointId
    ) {

        return new Promise(
            (
                resolve,
                reject
            ) => {

                const ws =
                    createWebSocket(
                        `/test-view/${encodeURIComponent(collection)}/add`
                    );


                let settled =
                    false;


                const finish =
                    (
                        callback,
                        value
                    ) => {

                        if (settled) {

                            return;

                        }


                        settled =
                            true;


                        try {

                            ws.close();

                        } catch {}


                        callback(
                            value
                        );

                    };


                ws.addEventListener(
                    'open',
                    () => {

                        ws.send(
                            JSON.stringify({

                                endpoint_id:
                                    endpointId

                            })
                        );

                    }
                );


                ws.addEventListener(
                    'message',
                    event => {

                        try {

                            const message =
                                JSON.parse(
                                    event.data
                                );


                            if (
                                message.type ===
                                'error'
                            ) {

                                finish(
                                    reject,
                                    new Error(
                                        message.message ||
                                        message.payload?.message ||
                                        'Could not add endpoint to bookmarks.'
                                    )
                                );


                                return;

                            }


                            finish(
                                resolve,
                                message
                            );

                        } catch (error) {

                            finish(
                                reject,
                                error
                            );

                        }

                    }
                );


                ws.addEventListener(
                    'error',
                    error => {

                        finish(
                            reject,
                            error
                        );

                    }
                );

            }
        );

    }


    async function saveActiveBookmark(
        collection,
        endpointId
    ) {

        return http(
            'POST',
            `/bookmarks/` +
            `${encodeURIComponent(collection)}/` +
            `${encodeURIComponent(endpointId)}/save`
        );

    }


    async function unsaveActiveBookmark(
        collection,
        endpointId
    ) {

        return http(
            'POST',
            `/bookmarks/` +
            `${encodeURIComponent(collection)}/` +
            `${encodeURIComponent(endpointId)}/unsave`
        );

    }


    // ============================================================
    // COLLECTIONS
    // ============================================================

    async function listCollections() {

        return http(
            'GET',
            '/collections/list'
        );

    }


    async function getCollection(
        name
    ) {

        return http(
            'GET',
            `/collections/${encodeURIComponent(name)}`
        );

    }


    async function createCollection(
        name
    ) {

        return http(
            'POST',
            '/collections/create',
            {
                name
            }
        );

    }


    async function renameCollection(
        name,
        newName
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(name)}/rename`,
            {
                new_name:
                    newName
            }
        );

    }


    async function deleteCollection(
        name
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(name)}/delete`
        );

    }

    async function setCollectionAnnotation(
        name,
        annotation
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(name)}/annotation`,
            {
                annotation
            }
        );

    }


    async function listCollectionEndpoints(
        name
    ) {

        return http(
            'GET',
            `/collections/${encodeURIComponent(name)}/endpoints`
        );

    }


    async function removeEndpointFromCollection(
        name,
        endpointId
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(name)}/endpoints/remove`,
            {
                endpoint_id:
                    endpointId
            }
        );

    }


    // ============================================================
    // COLLECTIONS — LOAD INTO ACTIVE BOOKMARK WORKSPACE
    // ============================================================

    function loadCollection(
        name
    ) {

        return new Promise(
            (
                resolve,
                reject
            ) => {

                const ws =
                    createWebSocket(
                        `/bookmarks/` +
                        `${encodeURIComponent(name)}/load`
                    );


                let settled =
                    false;


                const finish =
                    (
                        callback,
                        value
                    ) => {

                        if (settled) {

                            return;

                        }


                        settled =
                            true;


                        try {

                            ws.close();

                        } catch {}


                        callback(
                            value
                        );

                    };


                ws.addEventListener(
                    'message',
                    event => {

                        try {

                            const message =
                                JSON.parse(
                                    event.data
                                );


                            if (
                                message.type ===
                                'error'
                            ) {

                                finish(
                                    reject,
                                    new Error(
                                        message.message ||
                                        message.payload?.message ||
                                        'Could not load collection.'
                                    )
                                );


                                return;

                            }


                            finish(
                                resolve,
                                message
                            );

                        } catch (error) {

                            finish(
                                reject,
                                error
                            );

                        }

                    }
                );


                ws.addEventListener(
                    'error',
                    error => {

                        finish(
                            reject,
                            error
                        );

                    }
                );

            }
        );

    }


    // ============================================================
    // COLLECTION TAG ROLLUPS — GLOBAL TAGS
    // ============================================================

    async function createGlobalTag(
        name,
        endpointIds
    ) {

        return http(
            'POST',
            '/collections/tags/create',
            {
                name,

                endpoint_ids:
                    endpointIds
            }
        );

    }


    async function deleteGlobalTags(
        names
    ) {

        return http(
            'POST',
            '/collections/tags/delete',
            {
                names
            }
        );

    }


    async function renameGlobalTag(
        oldName,
        newName
    ) {

        return http(
            'POST',
            '/collections/tags/rename',
            {
                old_name:
                    oldName,

                new_name:
                    newName
            }
        );

    }


    async function listGlobalTags() {

        return http(
            'GET',
            '/collections/tags/list'
        );

    }


    // ============================================================
    // COLLECTION — MEMBERSHIP TAGS
    // ============================================================

    async function addMembershipTag(
        collectionName,
        tag
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(collectionName)}` +
            `/membership-tags/add`,
            {
                tag
            }
        );

    }


    async function removeMembershipTag(
        collectionName,
        tag
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(collectionName)}` +
            `/membership-tags/remove`,
            {
                tag
            }
        );

    }


    async function listMembershipTags(
        collectionName
    ) {

        return http(
            'GET',
            `/collections/${encodeURIComponent(collectionName)}` +
            `/membership-tags`
        );

    }


    async function listCollectionsByTag(
        tagName
    ) {

        return http(
            'GET',
            `/collections/by-tag/${encodeURIComponent(tagName)}`
        );

    }


    // ============================================================
    // PER-ENDPOINT TAGS
    // ============================================================

    async function listPopularTags() {

        return http(
            'GET',
            '/tags/popular'
        );

    }
    async function deleteActiveBookmark(endpointId) {

    return new Promise((resolve, reject) => {

        const ws =
            createWebSocket(
                "/test-view/active/delete"
            );

        let settled = false;

        const finish = (
            callback,
            value
        ) => {

            if (settled) return;

            settled = true;

            try {
                ws.close();
            } catch {}

            callback(value);
        };

        ws.addEventListener(
            "open",
            () => {

                ws.send(
                    JSON.stringify({
                        endpoint_id:
                            endpointId
                    })
                );

            }
        );

        ws.addEventListener(
            "message",
            event => {

                try {

                    const message =
                        JSON.parse(
                            event.data
                        );

                    if (
                        message.type === "error"
                    ) {

                        finish(
                            reject,
                            new Error(
                                message.message ||
                                "Bookmark deletion failed."
                            )
                        );

                        return;
                    }

                    finish(
                        resolve,
                        message
                    );

                } catch (error) {

                    finish(
                        reject,
                        error
                    );

                }

            }
        );

        ws.addEventListener(
            "error",
            () => {

                finish(
                    reject,
                    new Error(
                        "Bookmark delete WebSocket failed."
                    )
                );

            }
        );

    });
}


    async function listEndpointTags(
        endpointId
    ) {

        return http(
            'GET',
            `/tags/${encodeURIComponent(endpointId)}`
        );

    }


    async function addEndpointTag(
        endpointId,
        tag
    ) {

        return http(
            'POST',
            `/tags/${encodeURIComponent(endpointId)}/add`,
            {
                tag
            }
        );

    }


    async function removeEndpointTag(
        endpointId,
        tag
    ) {

        return http(
            'POST',
            `/tags/${encodeURIComponent(endpointId)}/remove`,
            {
                tag
            }
        );

    }


    // ============================================================
    // DATA VIEW
    // ============================================================

    async function deleteDataViewFolder(
        folder
    ) {

        return http(
            'POST',
            `/dataview/${encodeURIComponent(folder)}/delete`
        );

    }


    async function mergeDataViewFolder(
        folder,
        body
    ) {

        return http(
            'POST',
            `/dataview/${encodeURIComponent(folder)}/merge`,
            body
        );

    }


    function listenForMergeNotification(
        folder,
        onReady,
        timeoutMs = 10000
    ) {

        let settled = false;
        const sockets = [];
        let timer = null;

        const cleanup = () => {
            if (timer) {
                clearTimeout(timer);
                timer = null;
            }
            sockets.forEach(ws => {
                try {
                    ws.onmessage = null;
                    ws.onerror = null;
                    ws.onclose = null;
                    ws.close();
                } catch {}
            });
            sockets.length = 0;
        };

        const trigger = (data) => {
            if (settled) return;
            settled = true;
            cleanup();
            if (typeof onReady === 'function') {
                onReady(data);
            }
        };

        // Fallback timer in case websocket is unavailable or silent
        if (timeoutMs > 0) {
            timer = setTimeout(() => {
                trigger({ type: 'timeout_fallback' });
            }, timeoutMs);
        }

        // Try connecting to /jobs/ws and shared WS /test-view/run
        const endpoints = ['/jobs/ws', '/test-view/run'];
        if (folder) {
            endpoints.push(`/dataview/${encodeURIComponent(folder)}/active`);
        }

        endpoints.forEach(path => {
            try {
                const ws = createWebSocket(path);
                sockets.push(ws);

                ws.onmessage = (event) => {
                    try {
                        const msg = typeof event.data === 'string'
                            ? JSON.parse(event.data)
                            : event.data;

                        const eventType = msg?.event?.type || msg?.type || msg?.event;
                        const op = msg?.op || msg?.event?.payload?.op || msg?.payload?.op;

                        if (
                            eventType === 'ExportReady' ||
                            eventType === 'ViewIoDone' ||
                            eventType === 'FolderBecameActive' ||
                            op === 'export_merge' ||
                            msg?.status === 'done' ||
                            msg?.status === 'completed'
                        ) {
                            trigger(msg);
                        }
                    } catch (e) {
                        console.warn('[EDMS-API] Error parsing merge WS event:', e);
                    }
                };

                ws.onerror = () => {};
            } catch (err) {
                console.warn(`[EDMS-API] Could not connect WS to ${path}:`, err);
            }
        });

        return cleanup;
    }

    function removeCollectionEndpoint(
        collectionId,
        endpointId
    ) {

        return http(
            'POST',
            `/collections/${encodeURIComponent(collectionId)}/endpoints/remove`,
            {
                endpoint_id: endpointId
            }
        );

    }

    function activateDataViewFolder(
        folder
    ) {

        return new Promise(
            (
                resolve,
                reject
            ) => {

                const ws =
                    createWebSocket(
                        `/dataview/${encodeURIComponent(folder)}/active`
                    );


                let settled =
                    false;


                const finish =
                    (
                        callback,
                        value
                    ) => {

                        if (settled) {

                            return;

                        }


                        settled =
                            true;


                        callback(
                            value
                        );

                    };


                ws.addEventListener(
                    'message',
                    event => {

                        try {

                            const message =
                                JSON.parse(
                                    event.data
                                );


                            finish(
                                resolve,
                                message
                            );

                        } catch (error) {

                            finish(
                                reject,
                                error
                            );

                        }

                    }
                );


                ws.addEventListener(
                    'error',
                    error => {

                        finish(
                            reject,
                            error
                        );

                    }
                );

            }
        );

    }


    // ============================================================
    // WEBVIEW
    // ============================================================

    async function createWebview(
        payload,
        sourceCollection,
        annotation
    ) {

        let body;

        if (typeof payload === 'object' && payload !== null) {

            body = payload;

        } else {

            body = {
                name: payload,
                source_collection: sourceCollection
            };

            if (annotation !== undefined) {

                body.annotation = annotation;

            }

        }

        return http(
            'POST',
            '/webview/create',
            body
        );

    }


    async function listWebviews() {

        return http(
            'GET',
            '/webview/list'
        );

    }


    async function createWebviewTag(
        name,
        endpointIds
    ) {

        const body = {
            name
        };


        if (
            endpointIds !== undefined
        ) {

            body.endpoint_ids =
                endpointIds;

        }


        return http(
            'POST',
            '/webview/tags/create',
            body
        );

    }


    async function deleteWebviewTags(
        names
    ) {

        return http(
            'POST',
            '/webview/tags/delete',
            {
                names
            }
        );

    }


    async function renameWebviewTag(
        oldName,
        newName
    ) {

        return http(
            'POST',
            '/webview/tags/rename',
            {
                old_name:
                    oldName,

                new_name:
                    newName
            }
        );

    }


    async function listWebviewTags() {

        return http(
            'GET',
            '/webview/tags/list'
        );

    }


    async function duplicateWebview(
        name,
        newName
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/duplicate`,
            {
                new_name:
                    newName
            }
        );

    }


    async function deleteWebview(
        name
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/delete`
        );

    }


    async function deleteWebviews(
        names
    ) {

        return http(
            'POST',
            '/webview/delete',
            {
                names:
                    Array.isArray(names) ? names : [names]
            }
        );

    }


    async function renameWebview(
        name,
        newName
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/rename`,
            {
                new_name:
                    newName
            }
        );

    }


    async function setWebviewAnnotation(
        name,
        annotation
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/annotation`,
            {
                annotation
            }
        );

    }


    async function combineWebviews(
        payload,
        newName,
        options = {}
    ) {

        let body;

        if (typeof payload === 'object' && !Array.isArray(payload) && payload !== null) {

            body = { ...payload };

            if (body.names && !body.sources) {

                body.sources = body.names;
                delete body.names;

            }

            if (body.new_name && !body.name) {

                body.name = body.new_name;
                delete body.new_name;

            }

        } else {

            const sources = Array.isArray(payload) ? payload : [payload];
            body = {
                name: newName,
                sources,
                ...options
            };

        }

        return http(
            'POST',
            '/webview/combine',
            body
        );

    }


    async function convertWebviewToCollection(
        name,
        payload
    ) {

        let body;

        if (typeof payload === 'object' && payload !== null) {

            body = { ...payload };

            if (body.collection_name && !body.collection) {

                body.collection = body.collection_name;
                delete body.collection_name;

            }

        } else if (typeof payload === 'string' && payload.trim()) {

            body = {
                collection: payload.trim()
            };

        } else {

            body = {
                collection: name
            };

        }

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/convert-to-collection`,
            body
        );

    }


    async function takeoutWebview(
        name,
        options = {}
    ) {

        let body;

        if (typeof options === 'object' && options !== null) {

            body = { ...options };

            if (body.action === 'overwrite') {

                body.overwrite = true;

            }

        } else {

            body = {};

        }

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/takeout`,
            body
        );

    }


    async function addWebviewMembershipTag(
        webviewName,
        tag
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(webviewName)}/membership-tags/add`,
            {
                tag
            }
        );

    }


    async function removeWebviewMembershipTag(
        webviewName,
        tag
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(webviewName)}/membership-tags/remove`,
            {
                tag
            }
        );

    }


    async function listWebviewMembershipTags(
        webviewName
    ) {

        return http(
            'GET',
            `/webview/${encodeURIComponent(webviewName)}/membership-tags`
        );

    }


    async function getWebviewFrontPage(
        name
    ) {

        return http(
            'GET',
            `/webview/${encodeURIComponent(name)}/front-page`
        );

    }


    async function saveWebviewFrontPage(
        name,
        frontPage
    ) {

        return http(
            'POST',
            `/webview/${encodeURIComponent(name)}/front-page`,
            {
                front_page: frontPage
            }
        );

    }


    async function getWebviewByTag(
        tagName
    ) {

        return http(
            'GET',
            `/webview/by-tag/${encodeURIComponent(tagName)}`
        );

    }


    // ============================================================
    // WEBVIEW — IMPORT
    // ============================================================

    async function importWebview(
        payload
    ) {

        return http(
            'POST',
            '/webview/import',
            payload
        );

    }




    // ============================================================
    // REPO VIEW
    // ============================================================

    async function createRepoview(
        payload,
        sourceCollection,
        annotation
    ) {

        let body;

        if (typeof payload === 'object' && payload !== null) {

            body = payload;

        } else {

            body = {
                name: payload,
                source_collection: sourceCollection
            };

            if (annotation !== undefined) {

                body.annotation = annotation;

            }

        }

        return http(
            'POST',
            '/repoview/create',
            body
        );

    }


    async function listRepoviews() {

        return http(
            'GET',
            '/repoview/list'
        );

    }


    async function createRepoviewTag(
        name,
        endpointIds
    ) {

        const body = {
            name
        };


        if (
            endpointIds !== undefined
        ) {

            body.endpoint_ids =
                endpointIds;

        }


        return http(
            'POST',
            '/repoview/tags/create',
            body
        );

    }


    async function deleteRepoviewTags(
        names
    ) {

        return http(
            'POST',
            '/repoview/tags/delete',
            {
                names
            }
        );

    }


    async function renameRepoviewTag(
        oldName,
        newName
    ) {

        return http(
            'POST',
            '/repoview/tags/rename',
            {
                old_name:
                    oldName,

                new_name:
                    newName
            }
        );

    }


    async function listRepoviewTags() {

        return http(
            'GET',
            '/repoview/tags/list'
        );

    }


    async function duplicateRepoview(
        name,
        newName
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/duplicate`,
            {
                new_name:
                    newName
            }
        );

    }


    async function deleteRepoview(
        name
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/delete`
        );

    }


    async function deleteRepoviews(
        names
    ) {

        return http(
            'POST',
            '/repoview/delete',
            {
                names:
                    Array.isArray(names) ? names : [names]
            }
        );

    }


    async function renameRepoview(
        name,
        newName
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/rename`,
            {
                new_name:
                    newName
            }
        );

    }


    async function setRepoviewAnnotation(
        name,
        annotation
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/annotation`,
            {
                annotation
            }
        );

    }


    async function combineRepoviews(
        payload,
        newName,
        options = {}
    ) {

        let body;

        if (typeof payload === 'object' && !Array.isArray(payload) && payload !== null) {

            body = { ...payload };

            if (body.names && !body.sources) {

                body.sources = body.names;
                delete body.names;

            }

            if (body.new_name && !body.name) {

                body.name = body.new_name;
                delete body.new_name;

            }

        } else {

            const sources = Array.isArray(payload) ? payload : [payload];
            body = {
                name: newName,
                sources,
                ...options
            };

        }

        return http(
            'POST',
            '/repoview/combine',
            body
        );

    }


    async function convertRepoviewToCollection(
        name,
        payload
    ) {

        let body;

        if (typeof payload === 'object' && payload !== null) {

            body = { ...payload };

            if (body.collection_name && !body.collection) {

                body.collection = body.collection_name;
                delete body.collection_name;

            }

        } else if (typeof payload === 'string' && payload.trim()) {

            body = {
                collection: payload.trim()
            };

        } else {

            body = {
                collection: name
            };

        }

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/convert-to-collection`,
            body
        );

    }


    async function takeoutRepoview(
        name,
        options = {}
    ) {

        let body;

        if (typeof options === 'object' && options !== null) {

            body = { ...options };

            if (body.action === 'overwrite') {

                body.overwrite = true;

            }

        } else {

            body = {};

        }

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/takeout`,
            body
        );

    }


    async function addRepoviewMembershipTag(
        repoviewName,
        tag
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(repoviewName)}/membership-tags/add`,
            {
                tag
            }
        );

    }


    async function removeRepoviewMembershipTag(
        repoviewName,
        tag
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(repoviewName)}/membership-tags/remove`,
            {
                tag
            }
        );

    }


    async function listRepoviewMembershipTags(
        repoviewName
    ) {

        return http(
            'GET',
            `/repoview/${encodeURIComponent(repoviewName)}/membership-tags`
        );

    }


    async function getRepoviewByTag(
        tagName
    ) {

        return http(
            'GET',
            `/repoview/by-tag/${encodeURIComponent(tagName)}`
        );

    }


    // ============================================================
    // REPOVIEW — TABLES
    // ============================================================

    async function generateRepoviewTables(
        name,
        options = {}
    ) {

        return http(
            'POST',
            `/repoview/${encodeURIComponent(name)}/tables/generate`,
            options
        );

    }


    async function getRepoviewTables(
        name
    ) {

        return http(
            'GET',
            `/repoview/${encodeURIComponent(name)}/tables`
        );

    }


    async function getRepoviewTableFile(
        name,
        file
    ) {

        return http(
            'GET',
            `/repoview/${encodeURIComponent(name)}/tables/${encodeURIComponent(file)}`
        );

    }


    // ============================================================
    // REPOVIEW — IMPORT
    // ============================================================

    async function importRepoview(
        payload
    ) {

        return http(
            'POST',
            '/repoview/import',
            payload
        );

    }


    // ============================================================
    // JOBS / ASYNC OPERATIONS
    // ============================================================

    async function getJob(
        jobId
    ) {

        return http(
            'GET',
            `/jobs/${encodeURIComponent(jobId)}`
        );

    }


    function listenForJob(
        jobId,
        onComplete,
        onError
    ) {

        let ws;
        let isDone = false;
        let pollTimer;

        try {

            ws = createWebSocket('/jobs/ws');

            ws.onmessage = (event) => {

                try {

                    const msg = JSON.parse(event.data);

                    const inner = msg.payload || msg.event || {};
                    const matchesJob = (msg.job_id === jobId || msg.id === jobId || inner.job_id === jobId || inner.id === jobId);
                    const status = msg.status || inner.status;
                    const isSuccess = status === 'completed' || status === 'done' || msg.type === 'ViewIoDone' || msg.event === 'ViewIoDone' || inner.type === 'ViewIoDone';
                    const isFailed = status === 'failed' || status === 'error';

                    if (matchesJob && isSuccess) {
                        isDone = true;
                        cleanup();

                        if (typeof onComplete === 'function') {
                            onComplete(msg);
                        }
                    } else if (matchesJob && isFailed) {
                        isDone = true;
                        cleanup();

                        if (typeof onError === 'function') {
                            onError(msg);
                        }
                    }

                } catch (e) {

                    console.warn('Error parsing jobs ws message:', e);

                }

            };

            ws.onerror = () => {

                startPolling();

            };

        } catch {

            startPolling();

        }

        function cleanup() {

            if (ws) {

                try { ws.close(); } catch {}

            }

            if (pollTimer) {

                clearInterval(pollTimer);

            }

        }

        function startPolling() {

            if (isDone || pollTimer) return;

            let attempts = 0;
            const maxAttempts = 30;

            pollTimer = setInterval(async () => {

                attempts++;

                try {

                    const res = await getJob(jobId);

                    if (res && res.ok && res.data) {

                        const status = res.data.status;

                        if (status === 'completed' || status === 'done' || res.data.ok) {

                            isDone = true;
                            cleanup();

                            if (typeof onComplete === 'function') {

                                onComplete(res.data);

                            }

                        } else if (status === 'failed' || status === 'error') {

                            isDone = true;
                            cleanup();

                            if (typeof onError === 'function') {

                                onError(res.data);

                            }

                        }

                    }

                } catch {}

                if (attempts >= maxAttempts) {

                    cleanup();

                }

            }, 2000);

        }

        return {
            cancel: cleanup
        };

    }


    // ============================================================
    // MERGE JOB LISTENER
    // ============================================================

    /**
     * listenForMergeJob — scoped variant of listenForJob for merge operations.
     *
     * The 202 response from POST /collections/:target/merge (or the legacy
     * /dataview/:target/merge) carries { job_id }.  Pass that id here so the
     * WS listener and poll-fallback only fire for this specific job, not for
     * any concurrent ViewIoDone event on the socket.
     *
     * @param {string}   jobId      - job_id from the 202 body
     * @param {Function} onComplete - called with job data on success
     * @param {Function} onError    - called with job data on failure
     * @returns {{ cancel: Function }}
     */
    function listenForMergeJob(jobId, onComplete, onError) {
        return listenForJob(jobId, onComplete, onError);
    }


    // ============================================================
    // QP (REQUEST/RESPONSE PAIRS)
    // ============================================================

    async function listQPs(
        endpointId
    ) {

        return http(
            'GET',
            `/test-view/${encodeURIComponent(endpointId)}/qps`
        );

    }


    async function createQP(
        endpointId,
        method,
        requestBody,
        responseBody
    ) {

        return http(
            'POST',
            `/test-view/${encodeURIComponent(endpointId)}/qps/create`,
            {
                method,
                request_body: requestBody,
                response_body: responseBody
            }
        );

    }


    async function updateQP(
        endpointId,
        requestNumber,
        requestBody,
        responseBody
    ) {

        return http(
            'POST',
            `/test-view/${encodeURIComponent(endpointId)}/qps/${encodeURIComponent(requestNumber)}/update`,
            {
                request_body: requestBody,
                response_body: responseBody
            }
        );

    }


    async function deleteQP(
        endpointId,
        requestNumber
    ) {

        return http(
            'POST',
            `/test-view/${encodeURIComponent(endpointId)}/qps/${encodeURIComponent(requestNumber)}/delete`
        );

    }


    // ============================================================
    // IMPORT/EXPORT TABLE
    // ============================================================

    async function getImportExportTable() {

        return http(
            'GET',
            '/import-export/table'
        );

    }


    async function runFormatCheck(
        section,
        path
    ) {

        return http(
            'POST',
            '/import-export/check',
            {
                section,
                path
            }
        );

    }


    // ============================================================
    // ENDPOINTS LOOKUP
    // ============================================================

    async function lookupEndpoint(
        endpointStr,
        method
    ) {

        const params = new URLSearchParams({
            endpoint_str: endpointStr
        });

        if (method !== undefined) {
            params.set('method', method);
        }

        return http(
            'GET',
            `/endpoints/lookup?${params.toString()}`
        );

    }


    // ============================================================
    // REPO EXPORT
    // ============================================================

    async function exportRepo(
        collection,
        filename
    ) {

        return http(
            'GET',
            `/repo/` +
            `${encodeURIComponent(collection)}/` +
            `${encodeURIComponent(filename)}/export`
        );

    }


    // ============================================================
    // REPO IMPORT
    // ============================================================

    async function importRepo(
        collection,
        filename
    ) {

        return http(
            'POST',
            `/repo/` +
            `${encodeURIComponent(collection)}/` +
            `${encodeURIComponent(filename)}/import`
        );

    }


    // ============================================================
    // LOGS
    // ============================================================

    async function getLogs() {

        return http(
            'GET',
            '/logs'
        );

    }


    // ============================================================
    // DASHBOARD
    // ============================================================

    async function getDataViewDashboard() {

        return http(
            'GET',
            '/dataview/dashboard'
        );

    }


    async function getDashboardSnapshot() {

        return http(
            'GET',
            '/dashboard/snapshot'
        );

    }


    async function getDashboardSnapshotHistory() {

        return http(
            'GET',
            '/dashboard/snapshot/history'
        );

    }


    async function getDashboardStatic() {

        return http(
            'GET',
            '/dashboard/static'
        );

    }


    async function getDashboardCrudOperations() {

        return http(
            'GET',
            '/dashboard/crud-operations'
        );

    }


    async function refreshDashboardCrudOperations() {

        return http(
            'POST',
            '/dashboard/crud-operations/refresh'
        );

    }


    async function compareDashboardSnapshots(
        from,
        to
    ) {

        const params =
            new URLSearchParams({

                from,
                to

            });


        return http(
            'GET',
            `/dashboard/compare?${params.toString()}`
        );

    }


    // ============================================================
    // COLLECTIONS — TAGS IMPORT
    // ============================================================

    async function importCollectionTags(
        collectionName,
        tags,
        exportExistingTags
    ) {

        const body = {
            tags
        };

        if (exportExistingTags !== undefined) {
            body.export_existing_tags = exportExistingTags;
        }

        return http(
            'POST',
            `/collections/${encodeURIComponent(collectionName)}/tags/import`,
            body
        );

    }


    async function listCollectionTagEndpoints(
        collectionName
    ) {

        return http(
            'GET',
            `/collections/${encodeURIComponent(collectionName)}/tags/endpoints`
        );

    }


    // ============================================================
    // PUBLIC API
    // ============================================================

    window.EdmsAPI = {

        // ----------------------------------------
        // General / Views
        // ----------------------------------------

        getHome,
        getTestView,
        getListView,


        // ----------------------------------------
        // Endpoints
        // ----------------------------------------

        registerEndpoint,
        registerAllEndpoints,
        deleteEndpoint,
        setEndpointAnnotation,


        // ----------------------------------------
        // Test View WebSockets
        // ----------------------------------------

        connectTestView,
        connectEndpointLoader,
        connectBookmarkLoader,
        connectHistoryLoader,

        startTest,
        waitForTestFinished,


        // ----------------------------------------
        // Test View REST
        // ----------------------------------------

        fetchRequest,
        fetchResponse,
        fetchHeaders,

        stopTest,

        saveHistory,
        clearHistory,

        saveBookmark,
        clearBookmarks,


        // ----------------------------------------
        // Active Bookmarks
        // ----------------------------------------

        addActiveBookmark,
        saveActiveBookmark,
        unsaveActiveBookmark,


        // ----------------------------------------
        // Collections
        // ----------------------------------------

        listCollections,
        getCollection,
        createCollection,
        renameCollection,
        deleteCollection,
        setCollectionAnnotation,

        listCollectionEndpoints,
        removeEndpointFromCollection,

        loadCollection,


        // ----------------------------------------
        // Collection Global Tags
        // ----------------------------------------

        createGlobalTag,
        deleteGlobalTags,
        renameGlobalTag,
        listGlobalTags,


        // ----------------------------------------
        // Collection Membership Tags
        // ----------------------------------------

        addMembershipTag,
        removeMembershipTag,
        listMembershipTags,
        listCollectionsByTag,


        // ----------------------------------------
        // Per-Endpoint Tags
        // ----------------------------------------

        listPopularTags,
        listEndpointTags,
        addEndpointTag,
        removeEndpointTag,
        deleteActiveBookmark,



        // ----------------------------------------
        // Data View
        // ----------------------------------------

        deleteDataViewFolder,
        mergeDataViewFolder,
        mergeCollections: mergeDataViewFolder,
        listenForMergeNotification,
        listenForMergeJob,
        removeCollectionEndpoint,
        activateDataViewFolder,


        // ----------------------------------------
        // Webview
        // ----------------------------------------

        createWebview,
        listWebviews,

        duplicateWebview,
        duplicateWeb: duplicateWebview,

        deleteWebview,
        deleteWebviews,
        deleteWebs: deleteWebviews,

        renameWebview,
        renameWeb: renameWebview,

        setWebviewAnnotation,
        annotateWebview: setWebviewAnnotation,

        combineWebviews,
        combineWebs: combineWebviews,

        convertWebviewToCollection,
        convertToCollection: convertWebviewToCollection,

        takeoutWebview,
        takeoutWeb: takeoutWebview,

        createWebviewTag,
        deleteWebviewTags,
        renameWebviewTag,
        listWebviewTags,

        addWebviewMembershipTag,
        removeWebviewMembershipTag,
        listWebviewMembershipTags,

        getWebviewFrontPage,
        saveWebviewFrontPage,

        getWebviewByTag,
        importWebview,


        // ----------------------------------------
        // Repo View
        // ----------------------------------------

        createRepoview,
        listRepoviews,

        duplicateRepoview,
        duplicateRepo: duplicateRepoview,

        deleteRepoview,
        deleteRepoviews,
        deleteRepos: deleteRepoviews,

        renameRepoview,
        renameRepo: renameRepoview,

        setRepoviewAnnotation,
        annotateRepoview: setRepoviewAnnotation,

        combineRepoviews,
        combineRepos: combineRepoviews,

        convertRepoviewToCollection,
        convertToCollectionRepo: convertRepoviewToCollection,

        takeoutRepoview,
        takeoutRepo: takeoutRepoview,

        createRepoviewTag,
        deleteRepoviewTags,
        renameRepoviewTag,
        listRepoviewTags,

        addRepoviewMembershipTag,
        removeRepoviewMembershipTag,
        listRepoviewMembershipTags,

        getRepoviewByTag,

        generateRepoviewTables,
        getRepoviewTables,
        getRepoviewTableFile,

        importRepoview,


        // ----------------------------------------
        // QPs (Request/Response Pairs)
        // ----------------------------------------

        listQPs,
        createQP,
        updateQP,
        deleteQP,


        // ----------------------------------------
        // Import / Export Table
        // ----------------------------------------

        getImportExportTable,
        runFormatCheck,


        // ----------------------------------------
        // Endpoints Lookup
        // ----------------------------------------

        lookupEndpoint,


        // ----------------------------------------
        // Collections Extended
        // ----------------------------------------

        importCollectionTags,
        listCollectionTagEndpoints,


        // ----------------------------------------
        // Jobs
        // ----------------------------------------

        getJob,
        listenForJob,


        // ----------------------------------------
        // Repo Import / Export
        // ----------------------------------------

        exportRepo,
        importRepo,


        // ----------------------------------------
        // Logs
        // ----------------------------------------

        getLogs,


        // ----------------------------------------
        // Dashboard
        // ----------------------------------------

        getDataViewDashboard,
        getDashboardSnapshot,
        getDashboardSnapshotHistory,
        getDashboardStatic,
        getDashboardCrudOperations,
        refreshDashboardCrudOperations,
        compareDashboardSnapshots

    };

})();