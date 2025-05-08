import * as api from './api.js';

async function uploadMap() {
  let mapData = document.getElementById("mapData")! as HTMLInputElement;

  let tags = [];
  for (let tagbox of tagCheckboxes) {
    if (tagbox.checked) {
      tags.push(tagbox.id);
    }
  }

  let response = await api.uploadMap(mapData.files![0], {
    'type': 'MapUploadMeta',
    tags,
  });

  let responseElement = document.getElementById("response")!;
  responseElement.innerHTML = '';
  if (response.type == "TsApiError") {
    if (response.error.type == "AlreadyUploaded") {
      let link = document.createElement("a");
      link.innerText = "Map already uploaded";
      link.href = api.webUrl() + "map/" + response.error.map_id;
      responseElement.appendChild(link);
    } else {
      responseElement.appendChild(document.createTextNode("Could not upload: " + response.message));
    }
  } else {
    let link = document.createElement("a");
    link.innerText = "Uploaded!";
    link.href = api.webUrl() + "map/" + response.map_id;
    responseElement.appendChild(link);
  }
}

let uploadButton = document.getElementById("uploadButton");
if (uploadButton) {
  uploadButton.onclick = _ => uploadMap();
}

let tagList = document.getElementById("selectedTagList")!;
let tagCheckboxes: NodeListOf<HTMLInputElement> = document.querySelectorAll(".checkbox");
for (let checkbox of tagCheckboxes) {
  checkbox.checked = false;
  let label = checkbox.parentElement!;
  let container = label.parentElement!;
  checkbox.onchange = _ => {
    if (checkbox.checked) {
      tagList.appendChild(label);
    } else {
      container.appendChild(label);
    }
  };
}

document.getElementById("resetTags")!.onclick = _ => {
  for (let checkbox of tagCheckboxes) {
    checkbox.checked = false;
    let event = new Event("change");
    checkbox.dispatchEvent(event);
  }
};
