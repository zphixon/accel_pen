import { Link } from "wouter";

function Home() {
  let mode;
  if (import.meta.env.DEV) {
    mode = "dev bruh";
  } else {
    mode = "rpod fjaei";
  }

  return <>
    <p>{mode} <Link href="/map/32">Some test map view</Link></p>
    <p><Link href="/login">Log in</Link></p>
  </>;
}

export default Home
