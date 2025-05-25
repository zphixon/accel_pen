import * as types from "../bindings/index";

interface MapTileProps {
  map: types.MapContext,
  showAuthor?: boolean,
  link?: string,
}
export function MapTile({ map, showAuthor = true, link = "" }: MapTileProps) {
  return <>
    <div className="mapTile">
      <a href={"/map/" + map.id + link}>
        <img className="mapThumbnail smallThumbnail" src={"/api/v1/map/" + map.id + "/thumbnail/small"} />
      </a>
      <div className="mapTileData">
        {/* TODO nadeo string */}
        { map.plain_name }
        {showAuthor ?
          <div className="mapTileAuthor">
            {/* TODO user summary */}
            by {map.author.display_name}
          </div>
          : ""}
      </div>
    </div>
  </>;
}
