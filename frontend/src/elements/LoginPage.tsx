import { useParams, useSearchParams } from "react-router";

interface LoginPageProps {
  oauthRedirect: boolean
}
function LoginPage({oauthRedirect}: LoginPageProps) {
  let [params, _setSearchParams] = useSearchParams();
  return <>
    
  </>;
}

export default LoginPage
