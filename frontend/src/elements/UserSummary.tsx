
import * as types from "../../../backend/bindings/index";
import ClubTag from "./ClubTag";

function UserSummary({ user }: { user: types.UserResponse }) {
  return <>
    <span className="userClubTag"><ClubTag tag={user.club_tag} /></span>&nbsp;
    <span className="userDisplayName">{user.display_name}</span>&nbsp;
  </>;
}

export default UserSummary