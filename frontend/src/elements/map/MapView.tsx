import { Suspense, use } from "react";

import * as api from "../../api";

interface MapViewInnerProps {
  mapDataPromise: ReturnType<typeof api.mapData>,
}
function MapViewInner({ mapDataPromise }: MapViewInnerProps) {
  let mapData = use(mapDataPromise);
  if (mapData == undefined || mapData.type == 'TsApiError') {
    let message = `Could not load map - ${mapData.message}`
    return <>{message}</>;
  }

  return <>
    Map {mapData.name}
  </>
}

interface MapViewProps {
  mapId: string,
}
function MapView({ mapId }: MapViewProps) {
  try {
    var mapIdNumber = Number.parseInt(mapId);
  } catch (ex) {
    return <>Invalid map ID</>;
  }

  return <Suspense>
    <MapViewInner mapDataPromise={api.mapData({
      type: "MapDataRequest",
      map_id: mapIdNumber,
    })} />
  </Suspense>;
}

export default MapView
