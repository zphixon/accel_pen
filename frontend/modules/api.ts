import * as types from '../bindings/index';

const API_URL_DEV = new URL("http://localhost:2460/v1");
const API_URL = new URL("https://api.xdd.farm/v1");

const DEV: boolean = window.location.hostname == "localhost";

export function apiUrl(): URL {
  if (DEV) {
    return API_URL_DEV;
  } else {
    return API_URL;
  }
}

interface ApiCallOptions {
  params?: any,
  body?: any,
  method?: string,
}
async function apiCall<T>(path: string, { params, body, method }: ApiCallOptions = {}): Promise<T | types.TsApiError> {
  let error: types.TsApiError = {
    type: 'TsApiError',
    error: { type: 'ApiFailed' },
    status: 500,
    message: `Request to backend failed`,
  };

  try {
    var response = await fetch(
      apiUrl() + path + "?" + new URLSearchParams(params).toString(),
      {
        method: method,
        mode: 'cors',
        credentials: 'include',
        body: body,
        signal: AbortSignal.timeout(5000),
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

export async function getSelf(): Promise<types.UserResponse | types.TsApiError> {
  return await apiCall<types.UserResponse>("/self");
}

//export async function allMapsBy(request: types.AllMapsByRequest): Promise<types.AllMapsByResponse | types.TsApiError> {
//  return await apiCall<types.AllMapsByResponse>("/map/all_by", { params: request });
//}
//
//export async function favoriteMaps(): Promise<[types.FavoriteMapResponse] | types.TsApiError> {
//  return await apiCall<[types.FavoriteMapResponse]>("/self/favorite_maps");
//}

export async function mapData(request: types.MapDataRequest): Promise<types.MapDataResponse | types.TsApiError> {
  return await apiCall<types.MapDataResponse>("/map/data", { params: request });
}

export async function uploadMap(data: FormData): Promise<types.MapUploadResponse | types.TsApiError> {
  return await apiCall<types.MapUploadResponse>("/map/upload", { body: data, method: 'POST' });
}