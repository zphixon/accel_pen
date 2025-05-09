import { useState } from 'react';
import { createRoot } from 'react-dom/client';

function ReactiveThingy() {
  let [num, setNum] = useState(0);
  return <button onClick={_ => setNum(num + 1)}>
    num is { num }
  </button>;
}

let node = document.getElementById("tryReactRoot")!;
let root = createRoot(node);
root.render(<ReactiveThingy />);
