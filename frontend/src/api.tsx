import * as types from '../../backend/bindings/index.ts';

const API_URL_DEV = new URL("http://localhost:2460/v1");
const API_URL = new URL("https://api.xdd.farm/v1");

const SELF_URL_DEV = new URL("http://localhost:5173/");
const SELF_URL = new URL("https://xdd.farm/");

export function apiUrl(): URL {
  if (import.meta.env.DEV) {
    return API_URL_DEV;
  } else {
    return API_URL;
  }
}

export function oauthStartUrl(): URL {
  return new URL(apiUrl() + "/oauth/start");
}

export function selfUrl(): URL {
  if (import.meta.env.DEV) {
    return SELF_URL_DEV;
  } else {
    return SELF_URL;
  }
}

async function get<T>(path: string, data: any): Promise<T | types.TsApiError> {
  let response = await fetch(
    apiUrl() + path + "?" + new URLSearchParams(data).toString(),
    { mode: 'cors' },
  );
  let value: T | types.TsApiError = await response.json();
  if (response.ok) {
    return value as T;
  } else {
    console.error("API call failed:", value);
    return value as types.TsApiError;
  }
}

export async function mapData(request: types.MapDataRequest): Promise<types.MapDataResponse | types.TsApiError> {
  return await get<types.MapDataResponse>("/map_data", request);
}
