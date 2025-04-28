import { useFormStatus } from "react-dom";
import { useState } from "react";

import * as api from "../../api.tsx";

function MapUpload() {
  let { pending } = useFormStatus();
  return <>
    <form method="POST" action="http://localhost:2460/v1/map/upload" encType="multipart/form-data">
      <label htmlFor="mapData">Map file </label>
      <input name="map_data" id="mapData" type="file"></input>
      <button name="submit" type="submit" disabled={pending}>Upload</button>
    </form>
  </>;
}

export default MapUpload
