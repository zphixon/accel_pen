import * as types from "../bindings/index";

interface TagBadgeProps {
  tag: types.TagInfo,
  searchLink?: boolean,
}
export function TagBadge({ tag, searchLink = false }: TagBadgeProps) {
  let parts = tag.name.split("/");
  let rootName = parts[0];
  let rest = parts.slice(1).join("/");

  let innerClasses = ["tagName", rootName].join(" ");
  let outerClasses = ["tagBadge", rootName, tag.name].join(" ");

  let inner = (
    <span className={innerClasses}>
      <span className="rootName">{rootName}/</span>{rest}
    </span>
  );

  if (searchLink) {
    return <a className={outerClasses} href={"/map/search?tagged_with=" + tag.name}>{inner}</a>
  } else {
    return <span className={outerClasses}>{inner}</span>;
  }
}
