import { useParams } from "react-router";
import { Suspense, use } from "react";

import * as api from "../../api";

interface MapViewInnerProps {
  mapDataPromise: ReturnType<typeof api.mapData>,
}
function MapViewInner({ mapDataPromise }: MapViewInnerProps) {
  let mapData = use(mapDataPromise);
  if (mapData == undefined || mapData.type == 'ApiError') {
    let message = `Could not load map - ${mapData.message}`
    return <>{message}</>;
  }

  return <>
    Map {mapData.name}
  </>
}

function MapView() {
  let params = useParams();
  if (params.mapId == undefined) {
    return <>Missing map ID</>;
  }
  try {
    var mapId = Number.parseInt(params.mapId);
  } catch (ex) {
    return <>Invalid map ID</>;
  }

  return <Suspense>
    <MapViewInner mapDataPromise={api.mapData(mapId)} />
  </Suspense>;
}

export default MapView
