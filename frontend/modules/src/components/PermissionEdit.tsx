import { useEffect, useState } from "react";
import * as types from "../bindings/index";

interface UserPermissionProps {
  perm: types.Permission,
  isUser?: boolean,
  onUpdatePerm?: (perm: types.Permission) => void,
}
export function UserPermission({ perm, onUpdatePerm, isUser = false }: UserPermissionProps) {
  function id(id: string): string {
    return id + perm.user_id;
  }

  let [mayManage, setMayManage] = useState(perm.may_manage);
  let [mayGrant, setMayGrant] = useState(perm.may_grant);

  useEffect(() => {
    if (onUpdatePerm) {
      onUpdatePerm({
        ...perm,
        may_manage: mayManage,
        may_grant: mayGrant,
      });
    }
  }, [mayManage, mayGrant]);

  return <>
    <span className="permissionEdit">
      <span>
        <a href={"/user/" + perm.user_id}>{perm.display_name}</a>
        {isUser ? "(you)" : ""}
      </span>

      <span>
        <input
          type="checkbox"
          disabled={isUser}
          id={id("mayManage")}
          checked={mayManage}
          onChange={e => setMayManage(e.target.checked)}
        />
        <label htmlFor={id("mayManage")}>May manage map</label>
      </span>

      <span>
        <input
          type="checkbox"
          disabled={isUser}
          id={id("mayGrant")}
          checked={mayGrant}
          onChange={e => setMayGrant(e.target.checked)}
        />
        <label htmlFor={id("mayGrant")}>May grant permissions</label>
      </span>
    </span>
  </>;
}
