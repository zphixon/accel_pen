export function url(): string {
  if (import.meta.env.DEV) {
    return "http://localhost:2460/api/v1";
  } else {
    return "https://xdd.farm/api/v1";
  }
}

export interface MapData {
  type: 'MapData',
  name: string,
}

export type ApiErrorType = 'Database' | 'InvalidMapId' | 'MapNotFound' | 'NotFound';

export interface ApiError {
  type: 'ApiError',
  error: ApiErrorType,
  message: string,
}

export async function mapData(mapId: number): Promise<MapData | ApiError> {
  let response = await fetch(url() + "/map_data/" + mapId, {mode: 'cors'});
  let json: MapData | ApiError = await response.json();
  if (json.type == 'ApiError') {
    console.error(json);
  }
  return json;
}