function Home() {
  let mode;
  if (import.meta.env.DEV) {
    mode = "dev bruh";
  }

  return <>
    <p>{mode}</p>
  </>;
}

export default Home
