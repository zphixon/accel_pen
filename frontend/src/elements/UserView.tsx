import { Suspense, use } from "react";
import { Link } from "wouter";
import { usePathname } from "wouter/use-browser-location";

import * as api from "../api";

interface UserOrLoginProps {
  selfPromise: ReturnType<typeof api.getSelf>,
}
function UserOrLogin({ selfPromise }: UserOrLoginProps) {
  let returnPath = encodeURIComponent(usePathname());
  let selfResult = use(selfPromise);

  if (selfResult.type == "TsApiError") {
    if (selfResult.status == 401 && selfResult.error.type == "Rejected") {
      return <Link href={`~/login?returnPath=${returnPath}`}>Log in</Link>;
    } else {
      return <>Could not log in: {selfResult.message}</>;
    }
  } else {
    return <>{selfResult.display_name} <a href={api.oauthLogoutUrl().href}>Log out</a></>;
  }
}

function UserView() {
  return <>
    <Suspense fallback={"Loading"}>
      <UserOrLogin selfPromise={api.getSelf()} />
    </Suspense>
  </>;
}

export default UserView
