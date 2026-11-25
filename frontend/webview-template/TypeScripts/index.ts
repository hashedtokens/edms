// ─────────────────────────────────────────────
// TYPES
// ─────────────────────────────────────────────

export type methodType = "GET" | "POST" | "PATCH" | "DELETE" | "PUT";

export interface Endpoint {
  eid: string;
  count: number;
  qpPairs: number;
  annotation: string;
  method: methodType;
  path: string;
  tags: string[];
  status: "active" | "inactive" | "deprecated";
}

/** A single entry in the endpoint index list */
export interface EndpointIndex {
  eid: string;
}

export interface MetaData {
  tags: number;
  segments: number;
  endpoints: number;
}

export interface PanelData {
  body: unknown;
  meta_data: {
    request_size: number;
  };
}

// ─────────────────────────────────────────────
// CONSTANTS  (import these instead of re-declaring in other files)
// ─────────────────────────────────────────────

const ROOT_FOLDER = "data-new-2";
const ALLEID_DATA_FOLDER = "allEIDs";
const EIDs_FOLDER = "EIDs";
// const REQUESTS_FOLDER = "requests";
// const RESPONSES_FOLDER = "responses";

// File name patterns — change these to rename all request/response files at once
const REQUEST_FILE_SUFFIX = "request";   // e.g. E0001-BG-request1.json
const RESPONSE_FILE_SUFFIX = "response"; // e.g. E0001-BG-response1.json

export const ENDPOINT_PER_PAGE = 500;
export const DEFAULT_CATEGORY = "ALL";

// ─────────────────────────────────────────────
// ENDPOINT CACHE
// ─────────────────────────────────────────────

const endpointDetailsCache = new Map<string, Endpoint>();

// ─────────────────────────────────────────────
// FETCH HELPER (always read fresh data)
// ─────────────────────────────────────────────

const fetchJson = <T>(url: string): Promise<T> => {
  const separator = url.includes("?") ? "&" : "?";
  return fetch(`${url}${separator}_=${Date.now()}`, { cache: "no-store" }).then(
    (r) => r.json() as Promise<T>,
  );
};

// ─────────────────────────────────────────────
// ENDPOINT INDEX FETCH
// ─────────────────────────────────────────────

export const LoadEndpointsIndex = async (): Promise<EndpointIndex[]> => {
  const path = `./${ROOT_FOLDER}/${ALLEID_DATA_FOLDER}/EndpointIndex.json`;
  const endpointsIndex = await fetchJson<EndpointIndex[]>(path);
  return endpointsIndex;
};

// ─────────────────────────────────────────────
// ENDPOINT DETAIL FETCH
// ─────────────────────────────────────────────

export const FetchEndpointDetail = async (
  eid: string,
): Promise<Endpoint | null> => {
  if (endpointDetailsCache.has(eid)) return endpointDetailsCache.get(eid)!;
  try {
    const path = `./${ROOT_FOLDER}/${EIDs_FOLDER}/${eid}/${eid}.json`;
    const data = await fetchJson<Endpoint>(path);
    endpointDetailsCache.set(eid, data);
    return data;
  } catch (err) {
    console.error("failed to load", eid, err);
    return null;
  }
};

export const FetchEndpointsByIndex = async (
  indexArray: EndpointIndex[],
): Promise<Endpoint[]> => {
  const details: Endpoint[] = [];
  const CONCURRENCY_LIMIT = 100;

  for (let i = 0; i < indexArray.length; i += CONCURRENCY_LIMIT) {
    const batch = indexArray.slice(i, i + CONCURRENCY_LIMIT);
    const batchResults = await Promise.all(
      batch.map((it) => FetchEndpointDetail(it.eid)),
    );
    const validResults = batchResults.filter(
      (item): item is Endpoint => item !== null,
    );
    details.push(...validResults);
  }

  return details;
};

// ─────────────────────────────────────────────
// META DATA FETCH
// ─────────────────────────────────────────────

export const LoadMetaData = async (): Promise<MetaData | null> => {
  try {
    const path = `./${ROOT_FOLDER}/${ALLEID_DATA_FOLDER}/meta-data.json`;
    const data = await fetchJson<MetaData>(path);
    return data;
  } catch (error) {
    console.error("failed to load meta data", error);
    return null;
  }
};

// ─────────────────────────────────────────────
// REQUEST / RESPONSE PANEL FETCH
// ─────────────────────────────────────────────

export const LoadRequestPanelData = async (
  eid: string,
  rqIndex: number,
): Promise<PanelData | null> => {
  const path = `./${ROOT_FOLDER}/${EIDs_FOLDER}/${eid}/${REQUEST_FILE_SUFFIX}-${rqIndex + 1}.json`;
  try {
    return await fetchJson<PanelData>(path);
  } catch (error) {
    console.error(`Failed to load request data: ${path}`, error);
    return null;
  }
};

export const LoadResponsePanelData = async (
  eid: string,
  rqIndex: number,
): Promise<PanelData | null> => {
  const path = `./${ROOT_FOLDER}/${EIDs_FOLDER}/${eid}/${RESPONSE_FILE_SUFFIX}-${rqIndex + 1}.json`;
  try {
    return await fetchJson<PanelData>(path);
  } catch (error) {
    console.error(`Failed to load response data: ${path}`, error);
    return null;
  }
};

// ─────────────────────────────────────────────
// SIDEBAR DATA FETCH
// ─────────────────────────────────────────────

export const LoadSegments = async (): Promise<Record<string, string[]>> => {
  const path = `./${ROOT_FOLDER}/${ALLEID_DATA_FOLDER}/EndpointSegments_EID.json`;
  return await fetchJson<Record<string, string[]>>(path);
};

export const LoadAllTags = async (): Promise<Record<string, string[]>> => {
  const path = `./${ROOT_FOLDER}/${ALLEID_DATA_FOLDER}/Tags_EID.json`;
  return await fetchJson<Record<string, string[]>>(path);
};
