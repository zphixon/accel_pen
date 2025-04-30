import { Link } from "wouter";

import LoginLink from "./LoginLink";
import * as api from "../api";

import "./NavBar.css";
import UserSummary from "./UserSummary";

function NavBar() {
  let user = api.useLoggedInUser();

  let userLink;
  if (user == undefined || user.type == 'TsApiError' && user.status == 401 && user.error.type == 'Rejected') {
    userLink = <LoginLink />;
  } else if (user.type == 'TsApiError') {
    userLink = <div className="errorMessage">Could not log in: {user.message}</div>;
  } else {
    userLink = <span className="userHeader">
      <UserSummary user={user} />
      <a href={api.oauthLogoutUrl().href}>Log out</a>
    </span>;
  }

  return <div className="navBar">
    <Link className="homeLink" to="~/">Accel Pen</Link> {userLink}
  </div>;
}

export default NavBar