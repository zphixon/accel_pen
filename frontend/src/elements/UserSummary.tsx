
import * as types from "../../../backend/bindings/index";
import NandoString from "./NandoString";

function UserSummary({ user }: { user: types.UserResponse | types.AuthorResponse }) {
  return <>
    <span className="userClubTag">[<NandoString string={user.club_tag} />]</span>&nbsp;
    <span className="userDisplayName">{user.display_name}</span>&nbsp;
  </>;
}

export default UserSummary