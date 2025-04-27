import { Link } from "wouter";
import { Suspense, use } from "react";

import * as api from "../api.tsx";

interface UserDisplayProps {
  selfPromise: ReturnType<typeof api.self>,
}
function UserDisplay({ selfPromise }: UserDisplayProps) {
  let self = use(selfPromise);
  if (self.type == "TsApiError") {
    if (self.status == 401 && self.error == "Rejected") {
      return <Link href="/login">Log in</Link>;
    } else {
      return <div>Could not log in: {self.message}</div>;
    }
  } else {
    return <div>{self.display_name} <a href={api.oauthLogoutUrl().href}>Log out</a></div>;
  }
}

function Home() {
  let mode;
  if (import.meta.env.DEV) {
    mode = "dev bruh";
  } else {
    mode = "rpod fjaei";
  }

  return <>
    <Suspense fallback={"Loading"}><UserDisplay selfPromise={api.self()} /></Suspense>
    <p>{mode} <Link href="/map/32">Some test map view</Link></p>
  </>;
}

export default Home
