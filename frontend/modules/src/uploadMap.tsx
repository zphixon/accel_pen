import { createRoot } from "react-dom/client";
import React, { useEffect, useState } from "react";
import * as types from "./bindings/index";
import * as api from "./api.js";
import TagSelect from "./components/tagSelect";

function UploadMap() {
  // TODO configurable
  let maxTags = 7;
  let [selectedTags, setSelectedTags] = useState<types.TagInfo[]>([]);
  let [mapFile, setMapFile] = useState<File | undefined>(undefined);
  let [apiResponse, setApiResponse] = useState<types.MapUploadResponse | types.TsApiError | undefined>(undefined);

  function onChangeMap(event: React.ChangeEvent<HTMLInputElement>) {
    if (event.target.files && event.target.files.length > 0) {
      setMapFile(event.target.files[0]);
    }
  }

  function onSubmitMap() {
    if (selectedTags.length <= 0 || mapFile == undefined) {
      return;
    }

    api.uploadMap(mapFile, {
      type: 'MapUploadMeta',
      tags: selectedTags.map(tag => tag.name),
    }).then(response => setApiResponse(response))
  }

  let response = <></>;
  if (apiResponse) {
    if (apiResponse.type == "TsApiError") {
      if (apiResponse.error.type == "AlreadyUploaded") {
        // TODO root hmmmmmmmm
        // or just give up trying to support this because it's annoying
        response = <a href={"/map/" + apiResponse.error.map_id}>Map already uploaded</a>;
      } else {
        response = <>Could not upload map: {apiResponse.message}</>;
      }
    } else {
      response = <a href={"/map/" + apiResponse.map_id}>Uploaded successfully!</a>;
    }
  }

  let mayUpload = selectedTags.length > 0 && selectedTags.length <= maxTags && mapFile != undefined;

  return <>
    <p>
      <label htmlFor="mapFile">Map file:</label>
      <input type="file" id="mapFile" onChange={onChangeMap} />
    </p>
    <TagSelect
      tagInfo={JSON.parse(tagInfoNode.innerText)}
      selectedTags={selectedTags}
      setSelectedTags={setSelectedTags}
      maxTags={maxTags}
    />
    <p>
      <button disabled={!mayUpload} onClick={_ => onSubmitMap()}>Upload map</button>
    </p>
    {response}
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let mapUploadNode = document.getElementById("mapUpload")!;
let mapUploadRoot = createRoot(mapUploadNode);
mapUploadRoot.render(<UploadMap />);
