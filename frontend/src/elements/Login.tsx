import { useLocation, useSearchParams } from 'wouter';
import { Suspense, use } from 'react';
import { v4 as uuidv4 } from 'uuid'

import * as api from '../api.tsx';
import { useLocalStorage } from 'react-use';

interface LoginInnerProps {
  finishedOauthPromise: ReturnType<typeof api.finishOauth>,
}
function LoginInner({ finishedOauthPromise }: LoginInnerProps) {
  let finishedOauth = use(finishedOauthPromise);
  let [_tokens, setTokens] = useLocalStorage<api.OauthResponse>("accessTokens", undefined);
  let [_oauthState, setOauthState] = useLocalStorage<string>("oauthState", uuidv4());
  let [_location, setLocation] = useLocation();

  if (finishedOauth?.type == "ApiError") {
    return <>Login failed: {finishedOauth.message}</>;
  } else if (finishedOauth?.type == "OauthResponse") {
    setTokens(finishedOauth);
    setOauthState(undefined);
    setLocation("~/")
    return <>Logged in, redirecting</>;
  } else {
    return <>Unknown response from APIs: {JSON.stringify(finishedOauth)}</>;
  }
}

interface LoginProps {
  finish: boolean,
}
function Login({ finish }: LoginProps) {
  let [searchParams, _setSearchParams] = useSearchParams();
  let [oauthState, _setOauthState] = useLocalStorage<string>("oauthState", uuidv4());

  if (finish) {
    let paramCode = searchParams.get("code");
    let paramState = searchParams.get("state");
    if (!paramCode || !paramState) {
      return <>Missing state or code from Nadeo</>;
    }

    if (paramState != oauthState) {
      console.error(paramState, oauthState);
      return <>Incorrect state returned</>;
    }

    return <Suspense fallback={<>Logging in</>}>
      <LoginInner finishedOauthPromise={api.finishOauth(paramCode)}/>
    </Suspense>;
  } else {
    if (oauthState) {
      window.location.href = api.nadeoOauthUrl(oauthState).href;
    } else {
      console.error("Oauth state undefined?????????");
    }
    return null;
  }
}

export default Login