import NavBar from "./NavBar";

function Home() {
  let mode;
  if (import.meta.env.DEV) {
    mode = "dev bruh";
  } else {
    mode = "rpod fjaei";
  }

  return <>
    <NavBar />
    <p>{mode}</p>
  </>;
}

export default Home
