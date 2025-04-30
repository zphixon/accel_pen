import { Link } from "wouter";

import LoginLink from "./LoginLink";
import * as api from "../api";

import "./NavBar.css";

function renderClubTag(tag: string) {
  let result = <></>;
  let bold = false;
  let italic = false;
  let wide = false;
  let narrow = false;
  let uppercase = false;
  let shadow = false;
  let color: string | undefined = undefined;

  let ptr = 0;
  while (ptr < tag.length) {
    let part = <i className="fa-solid">{tag.charAt(ptr)}</i>;

    if (tag.charAt(ptr) == "$") {
      ptr += 1;
      if (ptr >= tag.length) {
        break;
      }

      if (tag.charAt(ptr) == "$") {
        ptr += 1;
      } else if (tag.charAt(ptr) == "o") {
        bold = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "i") {
        italic = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "w") {
        wide = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "n") {
        narrow = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "t") {
        uppercase = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "s") {
        shadow = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "L") {
        while (ptr < tag.length && tag.charAt(ptr) != ']') {
          ptr += 1;
        }
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "g") {
        color = undefined;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "z") {
        bold = italic = wide = narrow = uppercase = shadow = false;
        ptr += 1;
        continue;
      } else {
        color = "";
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr);
        ptr += 1;
        continue;
      }
    }

    if (bold) {
      part = <b>{part}</b>;
    }
    if (italic) {
      part = <i>{part}</i>;
    }
    if (wide) {
      //
    }
    if (narrow) {
      //
    }
    if (uppercase) {
      //
    }
    if (shadow) {
      //
    }
    if (color != undefined) {
      part = <span style={{ color: "#" + color }}>{part}</span>;
    }

    result = <>{result}{part}</>;
    ptr += 1;
  }

  return <>{result}</>;
}

function NavBar() {
  let user = api.useLoggedInUser();

  let userLink;
  if (user == undefined || user.type == 'TsApiError' && user.status == 401 && user.error.type == 'Rejected') {
    userLink = <LoginLink />;
  } else if (user.type == 'TsApiError') {
    userLink = <div className="errorMessage">Could not log in: {user.message}</div>;
  } else {
    userLink = <span className="userHeader">
      <span className="userClubTag">{renderClubTag(user.club_tag)}</span>&nbsp;
      <span className="userDisplayName">{user.display_name}</span>&nbsp;
      <a href={api.oauthLogoutUrl().href}>Log out</a>
    </span>;
  }

  return <div className="navBar">
    <Link className="homeLink" to="~/">Accel Pen</Link> {userLink}
  </div>;
}

export default NavBar