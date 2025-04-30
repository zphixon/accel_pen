function NandoString({ string: tag }: { string: string }) {
  let result = <></>;
  let bold = false;
  let italic = false;
  let wide = false;
  let narrow = false;
  let uppercase = false;
  let shadow = false;
  let color: string | undefined = undefined;

  let ptr = 0;
  while (ptr < tag.length) {
    let codepoint = tag.codePointAt(ptr) ?? 0;

    let part;
    if (
      0xE000 <= codepoint && codepoint <= 0xF8FF
      || 0xF0000 <= codepoint && codepoint <= 0xFFFFD
      || 0x100000 <= codepoint && codepoint <= 0x10FFFD
    ) {
      part = <i className="fa-solid">{tag.charAt(ptr)}</i>;
    } else {
      part = <>{tag.charAt(ptr)}</>;
    }

    if (tag.charAt(ptr) == "$") {
      ptr += 1;
      if (ptr >= tag.length) {
        break;
      }

      if (tag.charAt(ptr) == "$") {
        ptr += 1;
      } else if (tag.charAt(ptr) == "o") {
        bold = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "i") {
        italic = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "w") {
        wide = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "n") {
        narrow = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "t") {
        uppercase = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "s") {
        shadow = true;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "L") {
        while (ptr < tag.length && tag.charAt(ptr) != ']') {
          ptr += 1;
        }
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "g") {
        color = undefined;
        ptr += 1;
        continue;
      } else if (tag.charAt(ptr) == "z") {
        bold = italic = wide = narrow = uppercase = shadow = false;
        ptr += 1;
        continue;
      } else {
        color = "";
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr++);
        color += tag.charAt(ptr);
        ptr += 1;
        continue;
      }
    }

    if (bold) {
      part = <b>{part}</b>;
    }
    if (italic) {
      part = <i>{part}</i>;
    }
    if (wide) {
      //
    }
    if (narrow) {
      //
    }
    if (uppercase) {
      //
    }
    if (shadow) {
      //
    }
    if (color != undefined) {
      part = <span style={{ color: "#" + color }}>{part}</span>;
    }

    result = <>{result}{part}</>;
    ptr += 1;
  }

  return <span className="nandoString">{result}</span>;
}

export default NandoString