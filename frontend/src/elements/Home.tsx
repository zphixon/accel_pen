import { Suspense, use } from "react";
import * as api from "../api";
import UserSummary from "./UserSummary";
import NandoString from "./NandoString";

import "./Home.css";

interface FavoriteMapsProps {
  favoriteMapsPromise: ReturnType<typeof api.favoriteMaps>,
}
function FavoriteMaps({ favoriteMapsPromise }: FavoriteMapsProps) {
  let favoriteMaps = use(favoriteMapsPromise);

  if (!Array.isArray(favoriteMaps)) {
    return <>Could not load favorite maps: {favoriteMaps.message}</>;
  } else {
    let faves = [];
    for (let fave of favoriteMaps) {
      faves.push(<>
        <span key={fave.uid}>
          <NandoString string={fave.name} /> by <UserSummary user={fave.author} />
        </span>
        <br/>
      </>);
    }

    return <>{faves}</>;
  }
}

function Home() {
  let user = api.useLoggedInUser();

  let mode = <></>;
  if (import.meta.env.DEV) {
    mode = <p>dev bruh</p>;
  }

  let favoriteMaps = <></>;
  if (user?.type == 'UserResponse') {
    favoriteMaps = <Suspense fallback={"Loading favorite maps"}>
      <FavoriteMaps favoriteMapsPromise={api.favoriteMaps()} />
    </Suspense>;
  }

  return <>
    {mode}
    {favoriteMaps}
  </>;
}

export default Home
