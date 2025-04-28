import { Suspense, use } from "react";

import * as api from "../../api";
import NavBar from "../NavBar";

interface MapViewInnerProps {
  mapDataPromise: ReturnType<typeof api.mapData>,
}
function MapViewInner({ mapDataPromise }: MapViewInnerProps) {
  let mapData = use(mapDataPromise);
  if (mapData.type == 'TsApiError') {
    let message = `Could not load map - ${mapData.message}`
    return <>{message}</>;
  }

  let upload = new Date(mapData.uploaded);
  return <>
    <p>
      Map <br/>
      {mapData.name} by {mapData.author_name}, uploaded {upload.toString()}
    </p>
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

  return <>
    <NavBar />
    <Suspense fallback={<p>Loading</p>}>
      <MapViewInner mapDataPromise={api.mapData({
        type: "MapDataRequest",
        map_id: mapIdNumber,
      })} />
    </Suspense>
  </>;
}

export default MapView
