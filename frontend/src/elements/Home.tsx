import { Link } from "react-router";

function Home() {
  let mode;
  if (import.meta.env.DEV) {
    mode = "dev bruh";
  } else {
    mode = "rpod fjaei";
  }

  return <>
    <p>Fungus</p>
    <p><Link to="/map/32">Map view</Link></p>
    <p>{mode}</p>
  </>;
}

export default Home
