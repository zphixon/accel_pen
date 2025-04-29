import { Suspense, use, useContext } from "react";

import LoginLink from "./LoginLink";

import * as api from "../api";

interface UserOrLoginProps {
  selfPromise: ReturnType<typeof api.getSelf>,
}
function UserOrLogin({ selfPromise }: UserOrLoginProps) {
  let selfResult = use(selfPromise);

  if (selfResult.type == "TsApiError") {
    if (selfResult.status == 401 && selfResult.error.type == "Rejected") {
      return <LoginLink />;
    } else {
      return <>Could not log in: {selfResult.message}</>;
    }
  } else {
    return <>{selfResult.display_name} <a href={api.oauthLogoutUrl().href}>Log out</a></>;
  }
}

function NavBar() {
  return <div>
    Accel Pen <Suspense fallback={"Loading"}>
      <UserOrLogin selfPromise={api.getSelf()} />
    </Suspense>
  </div>;
}

export default NavBar