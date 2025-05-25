import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { useOnClickOutside } from "usehooks-ts";
import { UserPermission } from "./components/PermissionEdit";
import * as api from "./api";
import * as types from "./bindings/index";
import React, { StrictMode, useRef, useState } from "react";
import TagSelect from "./components/TagSelect";
import { UserSearch } from "./components/UserSearch";

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

  let [apiError, setApiError] = useState<types.TsApiError | undefined>(undefined);

  let [deleted, setDeleted] = useState(false);
  if (deleted) {
    return <>Map deleted</>;
  }

  function doDeleteMap() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: { type: "Delete" },
    }).then(response => {
      if (response.type == "TsApiError") {
        setApiError(response);
      } else {
        setDeleted(true);
      }
    });
  }

  function doSetTags() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: { type: "SetTags", tags: selectedTags },
    }).then(response => {
      if (response.type == "TsApiError") {
        setApiError(response);
      } else {
        location.reload();
      }
    });
  }

  let [permChanges, setPermChanges] = useState<types.PermissionUpdate[]>([]);

  let [addUser, setAddUser] = useState<types.UserResponse | undefined>(undefined);
  function prepareAddUser() {
    if (!addUser) {
      return;
    }

    let newPermChanges = [...permChanges];
    let newPermChange: types.PermissionUpdate = {
      type: "PermissionUpdate",
      update_type: "Add",
      permission: {
        type: "Permission",
        user_id: addUser.user_id,
        display_name: addUser.display_name,
        may_grant: false,
        may_manage: false,
      }
    };

    let existingPermIndex = newPermChanges.findIndex(change => change.permission.user_id == addUser.user_id)
    if (existingPermIndex >= 0) {
      newPermChanges[existingPermIndex] = newPermChange;
    } else {
      newPermChanges.push(newPermChange);
    }

    setPermChanges(newPermChanges);
  }
  function prepareEditUser(update: types.PermissionUpdate) {
    console.log(update);
    let newPermChanges = [...permChanges];
    let editPermIndex = newPermChanges.findIndex(perm => perm.permission.user_id == update.permission.user_id);
    if (editPermIndex >= 0) {
      let editPerm = newPermChanges[editPermIndex];
      editPerm.permission = update.permission;
      if (editPerm.update_type == "Add" && update.update_type == "Modify") {
        editPerm.update_type = "Add";
      } else if (editPerm.update_type == "Add" && update.update_type == "Remove") {
        newPermChanges.splice(editPermIndex, 1);
      } else {
        editPerm.update_type = update.update_type;
        if (update.update_type == "Remove") {
          editPerm.permission = { ...editPerm.permission, may_grant: false, may_manage: false };
        }
      }
    } else {
      newPermChanges.push(update);
    }
    setPermChanges(newPermChanges);
  }

  interface PermSomething {
    perm: types.Permission,
    update?: types.PermissionUpdateType,
  }

  let permsNoSelf: PermSomething[] = permData.filter(perm => perm.user_id != userData.user_id).map(perm => ({ perm }));
  let selfPerm = permData.find(perm => perm.user_id == userData.user_id)!;

  for (let change of permChanges) {
    if (change.update_type == "Add") {
      permsNoSelf.push({
        perm: change.permission,
        update: "Add",
      });
      continue;
    }
    let indexExisting = permsNoSelf.findIndex(perm => perm.perm.user_id == change.permission.user_id);
    if (indexExisting < 0) {
      continue;
    }
    if (change.update_type == "Modify") {
      permsNoSelf[indexExisting].perm = change.permission;
      permsNoSelf[indexExisting].update = "Modify";
    } else if (change.update_type == "Remove") {
      permsNoSelf[indexExisting].update = "Remove";
    }
  }

  let editControllers = [];
  for (let perm of permsNoSelf) {
    editControllers.push(
      <UserPermission
        key={"" + perm.perm.user_id + Math.random()}
        perm={perm.perm}
        update={perm.update}
        onUpdatePerm={prepareEditUser}
      />
    );
  }

  function requestPermUpdate() {
    api.manageMap(mapData.id, {
      type: "MapManageRequest",
      command: {
        type: "SetPermissions",
        permissions: permChanges,
      }
    }).then(response => {
      if (response.type == "TsApiError") {
        setApiError(response);
      } else {
        location.reload();
      }
    });
  }

  let apiErrorRendered = <></>;
  if (apiError) {
    apiErrorRendered = <>{apiError.error.type}: {apiError.message}</>
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
      {editControllers}
    </p>
    <div className="userSearch">
      <button onClick={_ => prepareAddUser()}>Add</button>
      <UserSearch selection={addUser} setSelection={setAddUser} />
    </div>
    <br/>
    <button onClick={_ => requestPermUpdate()}>Update permissions</button>

    <h3>Delete map</h3>
    <p>
      This action is not reversable.
      <button onClick={_ => setShowDelete(true)}>Delete map</button>
    </p>

    <p>{apiErrorRendered}</p>

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
