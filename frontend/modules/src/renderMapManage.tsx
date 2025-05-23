import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { useOnClickOutside } from "usehooks-ts";
import { UserPermission } from "./components/PermissionEdit";
import * as api from "./api";
import * as types from "./bindings/index";
import React, { StrictMode, useRef, useState } from "react";
import TagSelect from "./components/TagSelect";

let maxTags = 7;

interface MapManageProps {
  tagInfo: types.TagInfo[],
  mapData: types.MapContext,
  permData: types.Permission[],
  userData: types.UserResponse,
}
function MapManage({ tagInfo, mapData }: MapManageProps) {
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

  let [permsResponse, setPermsResponse] = useState<types.MapManageResponse | types.TsApiError | undefined>(undefined);
  let [permsNoSelf, setPermsNoSelf] = useState(permData.filter(perm => perm.user_id != userData.user_id));
  let selfPerm = permData.find(perm => perm.user_id == userData.user_id)!;
  function onUpdatePerm(newPerm: types.Permission) {
    let newPermsNoSelf = permsNoSelf.filter(perm => perm.user_id != newPerm.user_id);
    newPermsNoSelf = [...newPermsNoSelf, newPerm];
    setPermsNoSelf(newPermsNoSelf);
  }
  function requestPermUpdate() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: {
        type: "SetPermissions",
        permissions: permsNoSelf,
      },
    }).then(setPermsResponse)
  }

  let manageResponse = <></>;
  if (deleteResponse?.type == "TsApiError") {
    manageResponse = <>Couldn't delete map: {deleteResponse.message}</>;
  } else if (deleteResponse?.type == "MapManageResponse") {
    return <>Map deleted</>;
  }
  if (setTagsResponse?.type == "TsApiError") {
    manageResponse = <>{manageResponse} Couldn't set tags: {setTagsResponse.message}</>;
  } else if (setTagsResponse?.type == "MapManageResponse") {
    manageResponse = <>{manageResponse} Set tags successfully</>;
  }
  if (permsResponse?.type == "TsApiError") {
    manageResponse = <>{manageResponse} Couldn't set permissions: {permsResponse.message}</>;
  } else if (permsResponse?.type == "MapManageResponse") {
    manageResponse = <>{manageResponse} Set permissions successfully</>;
  }

  return <>
    <h3>Update tags</h3>
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

    <h3>Edit permissions</h3>
    <p>
      <UserPermission perm={selfPerm} isUser={true} />
      {permsNoSelf.map(perm => <UserPermission key={perm.user_id} perm={perm} onUpdatePerm={onUpdatePerm} />)}
      <br/>
      <button onClick={_ => requestPermUpdate()}>Update permissions</button>
    </p>

    <h3>Delete map</h3>
    <p>
      This action is not reversable.
      <button onClick={_ => setShowDelete(true)}>Delete map</button>
    </p>

    <p>{manageResponse}</p>

    {showDelete && <div className="bgBlur"></div>}
    {showDelete && createPortal(<div className="deleteMapConfirmation" ref={ref}>
      <div className="confirmMessage">
        Are you sure you want to delete this map? <b><i>This is action is not reversable.</i></b>
      </div>
      &nbsp;
      <div className="confirmButtons">
        <button onClick={_ => doDeleteMap()}>Delete</button>
        &nbsp;
        <button onClick={_ => setShowDelete(false)}>Cancel</button>
      </div>
    </div>, document.body)}
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let mapDataNode = document.getElementById("mapData")!;
let mapData: types.MapContext = JSON.parse(mapDataNode.innerText);

let permDataNode = document.getElementById("permData")!;
let permData: types.Permission[] = JSON.parse(permDataNode.innerText);

let userDataNode = document.getElementById("userData")!;
let userData: types.UserResponse = JSON.parse(userDataNode.innerText);

let mapManageNode = document.getElementById("mapManage")!;
let mapManageRoot = createRoot(mapManageNode);
mapManageRoot.render(
  <StrictMode>
    <MapManage tagInfo={tagInfo} mapData={mapData} permData={permData} userData={userData} />
  </StrictMode>
);
