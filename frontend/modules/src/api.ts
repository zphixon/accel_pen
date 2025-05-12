import * as types from './bindings/index';

const DEV: boolean = window.location.hostname == "localhost";

const API_URL = new URL("https://xdd.farm/api/v1");
const API_URL_DEV = new URL("http://localhost:2460/api/v1");
export function apiUrl(): URL {
  if (DEV) {
    return API_URL_DEV;
  } else {
    return API_URL;
  }
}

const WEB_URL = new URL("https://xdd.farm/");
const WEB_URL_DEV = new URL("http://localhost:2460/");
export function webUrl(): URL {
  if (DEV) {
    return WEB_URL_DEV;
  } else {
    return WEB_URL;
  }
}

interface ApiCallOptions {
  params?: URLSearchParams,
  body?: any,
  method?: string,
  contentType?: string,
}
async function apiCall<T>(path: string, { params, body, method, contentType }: ApiCallOptions = {}): Promise<T | types.TsApiError> {
  let error: types.TsApiError = {
    type: 'TsApiError',
    error: { type: 'ApiFailed' },
    status: 500,
    message: `Request to backend failed`,
  };

  try {
    var response = await fetch(
      apiUrl() + path + (params ? "?" + params.toString() : ""),
      {
        method: method,
        mode: 'cors',
        credentials: 'include',
        body: body,
        signal: AbortSignal.timeout(5000),
        headers: contentType ? { "Content-Type": contentType } : undefined,
      },
    );
  } catch (err) {
    console.log("Could not request API:", err);
    return error;
  }

  var text = "";
  try {
    text = await response.text();
    var value: T | types.TsApiError = JSON.parse(text);
  } catch (err) {
    console.log("Could not parse API response as JSON:", err, text);
    return error;
  }

  if (response.ok) {
    if (DEV) {
      console.log("Logging API response in dev mode", value);
    }
    return value as T;
  } else {
    console.error("API call failed:", value);
    return value as types.TsApiError;
  }
}

export async function uploadMap(map: File, mapMeta: types.MapUploadMeta): Promise<types.MapUploadResponse | types.TsApiError> {
  let body = new FormData();
  body.append("map_data", map);
  body.append("map_meta", JSON.stringify(mapMeta));
  return await apiCall<types.MapUploadResponse>("/map/upload", { body, method: 'POST' });
}

export async function manageMap(mapId: number, request: types.MapManageRequest): Promise<types.MapManageResponse | types.TsApiError> {
  return await apiCall<types.MapManageResponse>(
    "/map/" + mapId + "/manage",
    {
      method: 'POST',
      contentType: 'application/json',
      body: JSON.stringify(request),
    },
  );
}

export async function mapSearch(request: types.MapSearchRequest): Promise<types.MapSearchResponse | types.TsApiError> {
  return await apiCall<types.MapSearchResponse>(
    "/map/search",
    {
      method: 'POST',
      contentType: 'application/json',
      body: JSON.stringify(request),
    }
  );
}
