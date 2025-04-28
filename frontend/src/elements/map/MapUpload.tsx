import { useFormStatus } from "react-dom";
import { useState } from "react";
import { Link } from "wouter";

import * as api from "../../api.tsx";
import * as types from "../../../../backend/bindings/index.ts";
import NavBar from "../NavBar.tsx";

function MapUpload() {
  let { pending } = useFormStatus();
  let [uploadResult, setUploadResult] = useState<types.MapUploadResponse | types.TsApiError>();

  async function uploadMap(form: FormData) {
    setUploadResult(await api.uploadMap(form));
  }

  let linkToUploadedMap = undefined;
  if (uploadResult?.type == "MapUploadResponse") {
    linkToUploadedMap = <Link href={`~/map/${uploadResult.map_id}`}>Map uploaded</Link>;
  }

  let mapUploadError = undefined;
  if (uploadResult?.type == "TsApiError") {
    console.log(uploadResult.error.type);
    if (uploadResult.error.type == "AlreadyUploaded") {
      mapUploadError = <Link href={`~/map/${uploadResult.error.map_id}`}>Already uploaded</Link>;
    } else {
      mapUploadError = <>Could not upload map: {uploadResult.message}</>;
    }
  }

  return <>
    <NavBar />
    <form action={uploadMap}>
      <label htmlFor="mapData">Map file </label>
      <input name="map_data" id="mapData" type="file"></input>
      <button name="submit" type="submit" disabled={pending}>Upload</button>
    </form>
    {linkToUploadedMap ? linkToUploadedMap : ""}
    {mapUploadError ? mapUploadError : ""}
  </>;
}

export default MapUpload
