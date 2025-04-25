const CLIENT_ID = "e9cfcb43163263a46845";

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

export function selfUrl(): URL {
  if (import.meta.env.DEV) {
    return SELF_URL_DEV;
  } else {
    return SELF_URL;
  }
}

export function nadeoOauthUrl(state: string): URL {
  let url = new URL("https://api.trackmania.com/oauth/authorize");
  url.searchParams.append("response_type", "code");
  url.searchParams.append("client_id", CLIENT_ID);
  url.searchParams.append("scope", "read_favorite write_favorite");
  url.searchParams.append("redirect_uri", selfUrl() + "login/finish");
  url.searchParams.append("state", state);
  return url;
}

export type ApiErrorType = 'Database' | 'InvalidMapId' | 'MapNotFound' | 'NotFound';

export interface ApiError {
  type: 'ApiError',
  error: ApiErrorType,
  message: string,
}

export interface MapData {
  type: 'MapData',
  name: string,
}

export const MAP_DATA_URL = new URL(apiUrl() + "/map_data/")

export async function mapData(mapId: number): Promise<MapData | ApiError> {
  let response = await fetch(MAP_DATA_URL + mapId.toString(), { mode: 'cors' });
  let json: MapData | ApiError = await response.json();
  if (json.type == 'ApiError') {
    console.error(json);
  }
  return json;
}

export interface OauthResponse {
  type: 'OauthResponse',
  access_token: string,
  refresh_token: string,
}

export async function finishOauth(code: string): Promise<OauthResponse | ApiError> {
  let response = await fetch(
    apiUrl() + "/oauth",
    {
      //mode: 'cors',
      method: 'POST',
      body: JSON.stringify({ code: code }),
      headers: new Headers({ 'Content-Type': 'application/json' })
    }
  );
  let json: OauthResponse | ApiError = await response.json();
  if (json.type == 'ApiError') {
    console.error(json);
  }
  return json;
}