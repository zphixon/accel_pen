import { Link } from "wouter";

import LoginLink from "./LoginLink";
import * as api from "../api";

function NavBar() {
  let user = api.useLoggedInUser();

  let userLink;
  if (user == undefined || user.type == 'TsApiError' && user.status == 401 && user.error.type == 'Rejected') {
    userLink = <LoginLink />;
  } else if (user.type == 'TsApiError') {
    userLink = <>Could not log in: {user.message}</>;
  } else {
    userLink = <>{user.display_name} <a href={api.oauthLogoutUrl().href}>Log out</a></>;
  }

  return <div>
    <Link to="~/">Accel Pen</Link> {userLink}
  </div>;
}

export default NavBar