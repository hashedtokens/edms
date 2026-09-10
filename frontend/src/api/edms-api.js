// ============================================================
// EDMS API
// BACKEND COMMUNICATION LAYER
// ============================================================

(() => {
'use strict';

// ============================================================
// CONFIG
// ============================================================

const API_BASE = 'http://localhost:3000';

const WS_BASE = 'ws://localhost:3000';

// ============================================================
// HTTP HELPER
// ============================================================

async function http(method, path, body) {

    const response =
        await fetch(
            `${API_BASE}${path}`,
            {
                method,

                headers: {
                    'Content-Type': 'application/json'
                },

                body:
                    body === undefined
                        ? undefined
                        : JSON.stringify(body)
            }
        );


    const text =
        await response.text();


    let data;


    try {

        data =
            JSON.parse(text);

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
// R1 — REGISTER ENDPOINT
// ============================================================

async function registerEndpoint(
    endpointId,
    endpointUrl,
    method
) {

    return http(
        'POST',
        '/endpoints/create',
        {
            endpoint_id:
                endpointId,

            endpoint_str:
                endpointUrl,

            method:
                method
        }
    );

}

// ============================================================
// ONE-TIME BULK REGISTRATION UTILITY
// ============================================================

async function registerAllEndpoints(endpointList) {

    const results = [];


    for (const endpoint of endpointList) {

        const url =
            (endpoint.baseUrl || '') +
            (endpoint.endpoint || '');


        const result =
            await registerEndpoint(
                endpoint.id,
                url,
                endpoint.method
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
// TEST VIEW — STATIC METADATA
// ============================================================

async function getTestView() {

    return http(
        'GET',
        '/test-view'
    );

}

// ============================================================
// COLLECTIONS — SYSTEM 2
// ============================================================

async function listCollections() {

    return http(
        'GET',
        '/collections/list'
    );

}


async function getCollection(name) {

    return http(
        'GET',
        `/collections/${encodeURIComponent(name)}`
    );

}


async function createCollection(name) {

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


async function deleteCollection(name) {

    return http(
        'POST',
        `/collections/${encodeURIComponent(name)}/delete`
    );

}


async function listCollectionEndpoints(name) {

    return http(
        'GET',
        `/collections/${encodeURIComponent(name)}/endpoints`
    );

}
// ============================================================
// WS — LOAD COLLECTION INTO ACTIVE BOOKMARK WORKSPACE
// ============================================================

function loadCollection(name) {

    return new Promise(
        (resolve, reject) => {

            const ws =
                new WebSocket(
                    `${WS_BASE}/bookmarks/${encodeURIComponent(name)}/load`
                );


            let settled = false;


            const finish = (
                callback,
                value
            ) => {

                if (settled) return;

                settled =
                    true;

                try {
                    ws.close();
                } catch {}

                callback(value);

            };


            ws.addEventListener(
                'open',
                () => {

                    // The collection name is already part
                    // of the WebSocket URL.

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
// COLLECTION TAGS — SYSTEM 2
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


async function deleteGlobalTags(names) {

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


async function addMembershipTag(
    collectionName,
    tag
) {

    return http(
        'POST',
        `/collections/${encodeURIComponent(collectionName)}/membership-tags/add`,
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
        `/collections/${encodeURIComponent(collectionName)}/membership-tags/remove`,
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
        `/collections/${encodeURIComponent(collectionName)}/membership-tags`
    );

}


async function listCollectionsByTag(tagName) {

    return http(
        'GET',
        `/collections/by-tag/${encodeURIComponent(tagName)}`
    );

}

// ============================================================
// WS — TEST VIEW RUN
// ============================================================

function connectTestView() {

    return new WebSocket(
        `${WS_BASE}/test-view/run`
    );

}

// ============================================================
// WS — ENDPOINT SNAPSHOT
// ============================================================

function connectEndpointLoader() {

    return new WebSocket(
        `${WS_BASE}/test-view/endpoints/load`
    );

}

// ============================================================
// WS — TEST VIEW BOOKMARK SNAPSHOT
// ============================================================

function connectBookmarkLoader() {

    return new WebSocket(
        `${WS_BASE}/test-view/bookmarks/load`
    );

}

// ============================================================
// WS — TEST VIEW HISTORY SNAPSHOT
// ============================================================

function connectHistoryLoader() {

    return new WebSocket(
        `${WS_BASE}/test-view/history/load`
    );

}

// ============================================================
// WS — START TEST
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

        endpoint_id:
            endpointId,

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


    const message = {

        type:
            'run_test',

        payload

    };


    ws.send(
        JSON.stringify(message)
    );

}

// ============================================================
// WS — ADD TO ACTIVE BOOKMARKS
// ============================================================

function addActiveBookmark(
    endpointId
) {

    return new Promise(
        (resolve, reject) => {

            const ws =
                new WebSocket(
                    `${WS_BASE}/test-view/active/add`
                );


            let settled = false;


            const finish = (
                callback,
                value
            ) => {

                if (settled) return;

                settled =
                    true;

                try {
                    ws.close();
                } catch {}

                callback(value);

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

// ============================================================
// REST — SAVE ACTIVE BOOKMARK TO LOADED COLLECTION
// ============================================================

async function saveActiveBookmark(
    endpointId
) {

    return http(
        'POST',
        `/bookmarks/active/${encodeURIComponent(endpointId)}/save`
    );

}

// ============================================================
// REST — UNSAVE ACTIVE BOOKMARK
// ============================================================

async function unsaveActiveBookmark(
    endpointId
) {

    return http(
        'POST',
        `/bookmarks/active/${encodeURIComponent(endpointId)}/unsave`
    );

}

// ============================================================
// WS — WAIT FOR TEST EVENTS
// ============================================================

function waitForTestFinished(
    ws,
    handlers = {}
) {

    return new Promise(
        (resolve, reject) => {

            function handleMessage(wsEvent) {

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

                    reject(error);

                }

            }


            function handleError(error) {

                cleanup();

                reject(error);

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
// R2 — FETCH SAVED RESPONSE
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
// R2 — FETCH SAVED REQUEST
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
// R2 — FETCH SAVED HEADERS
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
// STOP TEST
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
// CLEAR HISTORY
// ============================================================

async function clearHistory() {

    return http(
        'POST',
        '/test-view/history/clearall'
    );

}

// ============================================================
// CLEAR BOOKMARKS
// ============================================================

async function clearBookmarks() {

    return http(
        'POST',
        '/test-view/bookmark/clearall'
    );

}

// ============================================================
// SAVE TO HISTORY
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

// ============================================================
// SAVE BOOKMARK — LEGACY
// ============================================================

async function saveBookmark(
    endpointId,
    notes
) {

    return http(
        'POST',
        '/test-view/save/bookmark',
        {
            endpoint_id:
                endpointId,

            notes
        }
    );

}

// ============================================================
// PUBLIC API
// ============================================================

window.EdmsAPI = {

    // ----------------------------------------
    // General / Endpoint
    // ----------------------------------------

    registerEndpoint,
    registerAllEndpoints,

    getTestView,


    // ----------------------------------------
    // Test View WebSockets
    // ----------------------------------------

    connectTestView,
    connectEndpointLoader,
    connectBookmarkLoader,
    connectHistoryLoader,

    startTest,
    waitForTestFinished,

    addActiveBookmark,


    // ----------------------------------------
    // Test View REST
    // ----------------------------------------

    fetchResponse,
    fetchRequest,
    fetchHeaders,

    stopTest,

    clearHistory,
    clearBookmarks,

    saveHistory,
    saveBookmark,


    // ----------------------------------------
    // Active Bookmarks
    // ----------------------------------------

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

    listCollectionEndpoints,
    removeEndpointFromCollection,
    loadCollection,


    // ----------------------------------------
    // Global Tags
    // ----------------------------------------

    createGlobalTag,
    deleteGlobalTags,
    renameGlobalTag,
    listGlobalTags,


    // ----------------------------------------
    // Membership Tags
    // ----------------------------------------

    addMembershipTag,
    removeMembershipTag,
    listMembershipTags,
    listCollectionsByTag

};

})();