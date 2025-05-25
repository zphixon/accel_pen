import { useEffect, useState } from "react";
import * as types from "../bindings/index";

interface UserPermissionProps {
  perm: types.Permission,
  isUser?: boolean,
  update?: types.PermissionUpdateType,
  onUpdatePerm?: (perm: types.PermissionUpdate) => void,
}
export function UserPermission({ perm, onUpdatePerm, isUser = false, update }: UserPermissionProps) {
  function id(id: string): string {
    return id + perm.user_id;
  }

  function onUpdate(update: types.Permission) {
    if (onUpdatePerm) {
      onUpdatePerm({
        type: "PermissionUpdate",
        update_type: "Modify",
        permission: update,
      });
    }
  }

  function onClickRemove() {
    if (onUpdatePerm) {
      onUpdatePerm({
        type: "PermissionUpdate",
        update_type: "Remove",
        permission: perm,
      });
    }
  }

  let classes = ["permissionEdit"];
  if (update == "Modify") {
    classes.push("permissionEditModify");
  } else if (update == "Remove") {
    classes.push("permissionEditRemove");
  } else if (update == "Add") {
    classes.push("permissionEditAdd");
  }

  return <>
    <span className={classes.join(" ")}>
      <span>
        <a href={"/user/" + perm.user_id}>{perm.display_name}</a>
        {isUser ? "(you)" : ""}
      </span>

      <span>
        <input
          type="checkbox"
          disabled={isUser}
          id={id("mayManage")}
          checked={perm.may_manage}
          onChange={e => onUpdate({ ...perm, may_manage: e.target.checked })}
        />
        <label htmlFor={id("mayManage")}>May manage map</label>
      </span>

      <span>
        <input
          type="checkbox"
          disabled={isUser}
          id={id("mayGrant")}
          checked={perm.may_grant}
          onChange={e => onUpdate({ ...perm, may_grant: e.target.checked })}
        />
        <label htmlFor={id("mayGrant")}>May grant permissions</label>
      </span>

      <button onClick={_ => onClickRemove()} disabled={isUser}>Remove</button>
    </span>
  </>;
}
