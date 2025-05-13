import { createRoot } from "react-dom/client";
import * as api from "./api";
import * as types from "./bindings/index";
import React, { useEffect, useRef, useState } from "react";
import TagSelect from "./components/TagSelect";

function MapUpload() {
  // TODO configurable
  let maxTags = 7;
  let [maySelectTags, setMaySelectTags] = useState(false);
  let [selectedTags, setSelectedTags] = useState<types.TagInfo[]>([]);

  function createKey() {
    return Math.random().toString(36);
  }
  let [forceRerender, setForceRerender] = useState(createKey());
  let mapFileRef = useRef<HTMLInputElement>(null);
  let [mapFile, setMapFile] = useState<File | undefined>(undefined);

  let [loading, setLoading] = useState(false);
  let [apiResponse, setApiResponse] = useState<types.MapUploadResponse | types.TsApiError | undefined>(undefined);

  function onChangeMap(event: React.ChangeEvent<HTMLInputElement>) {
    if (event.target.files && event.target.files.length > 0) {
      setSelectedTags([]);
      setMapFile(event.target.files[0]);
      setMaySelectTags(true);
    }
  }

  function onSubmitMap() {
    if (selectedTags.length <= 0 || mapFile == undefined) {
      return;
    }

    setLoading(true);
    api.uploadMap(mapFile, {
      type: 'MapUploadMeta',
      tags: selectedTags.map(tag => tag.name),
      last_modified: mapFile.lastModified,
    }).then(response => {
      if (response.type == 'MapUploadResponse') {
        setSelectedTags([]);
        setMapFile(undefined);
        setMaySelectTags(false);
        setForceRerender(createKey());
        if (mapFileRef.current) {
          mapFileRef.current.value = "";
        }
      }
      setApiResponse(response);
      setLoading(false);
    })
  }

  let response = <></>;
  if (loading) {
    response = <>Uploading</>;
  }

  if (apiResponse) {
    if (apiResponse.type == "TsApiError") {
      if (apiResponse.error.type == "AlreadyUploaded") {
        response = <a href={"/map/" + apiResponse.error.map_id}>Map already uploaded</a>;
      } else {
        response = <>Could not upload map: {apiResponse.message}</>;
      }
    } else {
      response = <a href={"/map/" + apiResponse.map_id}>{apiResponse.map_name} uploaded successfully!</a>;
    }
  }

  let mayUpload = selectedTags.length > 0 && selectedTags.length <= maxTags && mapFile != undefined;

  return <>
    <p>
      <label htmlFor="mapFile">Map file:</label>
      <input type="file" id="mapFile" onChange={onChangeMap} ref={mapFileRef} />
    </p>
    <TagSelect
      tagInfo={JSON.parse(tagInfoNode.innerText)}
      selectedTags={selectedTags}
      setSelectedTags={setSelectedTags}
      maxTags={maxTags}
      maySelectTags={maySelectTags}
    />
    <p>
      <button disabled={!mayUpload} onClick={_ => onSubmitMap()}>Upload map</button>
      {response}
    </p>
    <div hidden>{ forceRerender }</div>
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let mapUploadNode = document.getElementById("mapUpload")!;
let mapUploadRoot = createRoot(mapUploadNode);
mapUploadRoot.render(<MapUpload />);
