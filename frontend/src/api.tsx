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

