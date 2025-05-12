import { createRoot } from "react-dom/client";
import * as types from "./bindings/index";

function Search() {
  return <>
    funkus
  </>;
}

let tagInfoNode = document.getElementById("tagData")!;
let tagInfo: types.TagInfo[] = JSON.parse(tagInfoNode.innerText);

let searchNode = document.getElementById("search")!;
let searchRoot = createRoot(searchNode);
searchRoot.render(<Search />);
