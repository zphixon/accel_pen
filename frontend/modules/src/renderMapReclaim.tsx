import { createRoot } from "react-dom/client";
import * as api from "./api";
import * as types from "./bindings/index";
import React, { useEffect, useRef, useState } from "react";
import TagSelect from "./components/TagSelect";
import { MapTile } from "./components/MapTile";

interface MapReclaimProps {
  mapsUsers: types.MapUsers[],
  user: types.UserResponse,
}
function MapReclaim({ mapsUsers, user }: MapReclaimProps) {
  interface MapReclaimRequest {
    mapId: number,
    updates: types.PermissionUpdate[],
  }
  let requests: MapReclaimRequest[] = mapsUsers.map(
    mapUsers => ({
      mapId: mapUsers.map.id,
      updates: mapUsers.users.map(user => ({
        type: "PermissionUpdate",
        update_type: "Remove",
        permission: {
          type: "Permission",
          user_id: user.user_id,
          display_name: user.display_name,
          may_manage: false,
          may_grant: false,
        },
      })),
    })
  );

  let [reclaimSuccess, setReclaimSuccess] = useState<boolean | undefined>(undefined);
  function doReclaim() {
    let promises = [];
    for (let request of requests) {
      promises.push(
        api.manageMap(request.mapId, {
          type: "MapManageRequest",
          command: {
            type: "SetPermissions",
            permissions: request.updates,
          },
        })
      );
    }
    Promise.all(promises).then(responses => {
      let success = true;
      for (let response of responses) {
        if (response.type == "TsApiError") {
          success = false;
          break;
        }
      }

      if (success) {
        api.apiCall<types.MapReclaimResponse>("/user/reclaimed", { method: 'POST' }).then(response => {
          if (response.type == "MapReclaimResponse") {
            setReclaimSuccess(success);
          } else {
            setReclaimSuccess(false);
          }
        })
      } else {
        setReclaimSuccess(false);
      }
    })
  }
  let reclaimResponse = <></>;
  if (reclaimSuccess === false) {
    // ^ could be undefined
    reclaimResponse = <>Could not reclaim maps, API error</>;
  } else if (reclaimSuccess === true) {
    reclaimResponse = <>Maps successfully reclaimed!</>;
  }

  let mapInfos = [];
  for (let mapUsers of mapsUsers) {
    let users = [];
    for (let mapUser of mapUsers.users) {
      if (mapUser.user_id != user.user_id) {
        users.push(
          <div className="user" key={mapUser.user_id}>
            {mapUser.display_name}
          </div>
        );
      }
    }
    mapInfos.push(
      <div key={mapUsers.map.id} className="mapUser">
        <MapTile showAuthor={false} map={mapUsers.map} link="/manage" />
        {users}
      </div>
    );
  }

  return <>
    <p>These maps are managed by other users:</p>
    <div className="mapUserList">
      {mapInfos}
    </div>
    <button onClick={_ => doReclaim()}>Reclaim maps</button>
    {reclaimResponse}
  </>;
}

let mapsUsersNode = document.getElementById("mapsUsers")!;
let mapsUsersData: types.MapUsers[] = JSON.parse(mapsUsersNode.innerText);

let userDataNode = document.getElementById("userData")!;
let userData: types.UserResponse = JSON.parse(userDataNode.innerText);

let mapReclaimNode = document.getElementById("mapReclaim")!;
let mapReclaimRoot = createRoot(mapReclaimNode);
mapReclaimRoot.render(<MapReclaim mapsUsers={mapsUsersData} user={userData} />);
