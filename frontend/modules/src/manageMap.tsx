import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { useOnClickOutside } from "../node_modules/usehooks-ts/dist/index";
import React, { useRef, useState } from "react";
import * as api from "./api.js";
import * as types from "./bindings/index";
import TagSelect from "./components/tagSelect";

let maxTags = 7;

interface ManageMapProps {
  tagInfo: types.TagInfo[],
  mapData: types.MapContext,
}
function ManageMap({ tagInfo, mapData }: ManageMapProps) {
  let [showDelete, setShowDelete] = useState(false);
  let [selectedTags, setSelectedTags] = useState<types.TagInfo[]>(mapData.tags);
  let maySetTags = selectedTags.length > 0 && selectedTags.length <= maxTags;

  let ref = useRef<HTMLDivElement>(null);
  useOnClickOutside(ref as React.RefObject<HTMLDivElement>, _ => setShowDelete(false));

  let [deleteResponse, setDeleteResponse] = useState<types.MapManageResponse | types.TsApiError | undefined>(undefined);
  function doDeleteMap() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: { type: "Delete" },
    }).then(setDeleteResponse);
  }

  let [setTagsResponse, setSetTagsResponse] = useState<types.MapManageResponse | types.TsApiError | undefined>(undefined);
  function doSetTags() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: { type: "SetTags", tags: selectedTags },
    }).then(setSetTagsResponse);
  }

  let manageResponse = <></>;
  if (deleteResponse?.type == "TsApiError") {
    manageResponse = <>Couldn't delete map: {deleteResponse.message}</>;
  } else if (deleteResponse?.type == "MapManageResponse") {
    return <>Map deleted</>;
  }
  if (setTagsResponse?.type == "TsApiError") {
    manageResponse = <>Couldn't set tags: {setTagsResponse.message}</>;
  } else if (setTagsResponse?.type == "MapManageResponse") {
    manageResponse = <>Set tags successfully</>;
  }

  return <>
    <TagSelect
      tagInfo={tagInfo}
      selectedTags={selectedTags}
      setSelectedTags={setSelectedTags}
      originalSelectedTags={mapData.tags}
      maxTags={maxTags}
    />
    <p>
      <button disabled={!maySetTags} onClick={_ => doSetTags()}>Update tags</button>
    </p>
    <p>
      <button onClick={_ => setShowDelete(true)}>Delete map</button>
    </p>
    <p>{manageResponse}</p>
    {showDelete && createPortal(<div className="deleteMapConfirmation" ref={ref}>
      <div className="confirmMessage">Are you sure you want to delete this map?</div>
      <div className="confirmButtons">
        <button onClick={_ => doDeleteMap()}>Delete</button>
        <button onClick={_ => setShowDelete(false)}>Cancel</button>
      </div>
    </div>, document.body)}
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let mapDataNode = document.getElementById("mapData")!;
let mapData: types.MapContext = JSON.parse(mapDataNode.innerText);

let mapManageNode = document.getElementById("mapManage")!;
let mapManageRoot = createRoot(mapManageNode);
mapManageRoot.render(<ManageMap tagInfo={tagInfo} mapData={mapData} />);
