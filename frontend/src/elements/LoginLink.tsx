import { Link } from "wouter";
import { usePathname } from "wouter/use-browser-location";

function LoginLink() {
  let returnPath = encodeURIComponent(usePathname());
  return <Link className="loginLink" href={`~/login?returnPath=${returnPath}`}>Log in</Link>;
}

export default LoginLink