import * as api from './api.js';
import * as tagSelect from './tagSelect.js';

async function uploadMap() {
  let mapData = document.getElementById("mapData")! as HTMLInputElement;

  let tags = [];
  for (let tagbox of tagSelect.tagCheckboxes) {
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

let uploadButton = document.getElementById("uploadButton")!;
uploadButton.onclick = _ => uploadMap();
