# Accel Pen

*The TrackMania map corral*

- [ ] map packs
  - [ ] list of maps
  - [ ] map pack administrators
  - [ ] submission approval
- [ ] map uploads
  - [ ] tags
    * [ ] tag voting
    * [ ] tag janitors?
  * [ ] like reddit but for maps
    * [ ] new/hot tabs, sortable by tag
  - [ ] link back to map pack
  - [ ] multiple authors
  - [ ] threaded comments
  - [ ] awards
  - [ ] leaderboards
  - [ ] unlisted maps
  * [ ] links to ecircuitmania?
- [ ] daily featured maps
  - [ ] totd tie-in
- [ ] forums?
- [ ] ~~ubi login~~ nadeo login


## Deploying

- [ ] Regular Dockerfile
- [ ] Docker compose support incoming

### Configuration

TODO docs


## Developing

To build for the first time:

- Clone: `git clone https://github.com/zphixon/accel_pen --recurse-submodules`

- Install build dependencies
  - Node 23.11 (through node version manager is easiest)
  - [SQLx CLI](https://github.com/launchbadge/sqlx/blob/6b2e0247d47d020d91dc6f7402d42e4e6131af11/sqlx-cli/README.md)
  - Postgres (alternatively, `docker compose -f dev.docker-compose.yml up --build` will bring up a containerized installation)

- Build the backend, and typescript bindings for the frontend
  ```shell
  # Test first - the build.rs script combines the generated bindings files
  cargo t && cargo b
  ```

- Build the frontend
  ```shell
  cd frontend
  npm install
  ./node_modules/.bin/rollup -c
  ```

- Start the postgres database (Docker is recommended)

- Run the project. Open in the browser at `localhost:2460`
  ```shell
  RUST_LOG='trace,notify=info,globset=info' cargo r -- dev.accel_pen.toml
  ```


### Adding Typescript modules

Create your *.ts*/*.tsx* file in *frontend/modules/src*.

```ts
import * as api from './api.js'; // .js extension required for modules to work
import * as types from './bindings/index'; // no extension necessary, doesn't contain module exports
```

Build it with the same command from earlier. The compiled JS will be placed in
*frontend/static/js*. Use it in a page:

```html
<script type="module" src="/static/js/myNewModule.js"></script>
```

Note that since it's a module, it can't interact with other `script` elements.
Write anything you need to run on page load directly in the *.ts* file. Use the
`rollup` command from earlier to build, and refresh.


### Adding SSR pages

Template files are automatically registered by a `Tera` instance if they are
placed in *frontend/templates*. There is a *layout.html.tera* template with some
common blocks that you might want, fontawesome support for club tags, and
[ManiaPlanet-formatted strings](https://wiki.trackmania.io/en/content-creation/text-styling);
as well as a macros file; otherwise it's pretty free-form. (Once I get annoyed
by this aspect I will switch to [askama](https://lib.rs/crates/askama))

The `Tera` instance is stored in a `RwLock`, allowing it to refresh templates
when changes are discovered (enabled by [notify](https://lib.rs/crates/notify)).
Set the top-level config value `debug_templates = true` to allow live template
reloading. Still requires an F5 though.

They are not automatically served however. Add a `.route()` call to the `Router`:

```rust
    let app = Router::new()
        ...
        .route("/map/{map_id}", get(get_map_page))
        ...
```

Define the handler:

```rust
async fn get_map_page(
    // AppState contains the database connection pool
    State(state): State<AppState>,

    // Contains cookie-based server-side Nadeo OAuth API tokens which are
    // automatically refreshed when an authenticated user connects to this
    // endpoint. The cookie is just an index into the server-side storage,
    // so no oauth tokens are transmitted to the client.
    auth: Option<NadeoAuthSession>,

    // Other extractors defined in the call to `route`
    Path(map_id): Path<i32>,
) -> Response {

    // Get a Tera context containing information about the authenticated
    // user, if there was one
    let mut context = config::context_with_auth_session(auth.as_ref());

    // Gather more information to put in `context` - do DB queries, make
    // requests to the Nadeo OAuth API or web services API...

    // Render and return the page
    match state
        .tera
        .read()
        .unwrap()
        .render("map/page.html.tera", &context)
        .context("Rendering map page template")
    {
        Ok(page) => Html(page).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}
```

Finally, write your new template *.html.tera* file. See [the Tera docs](https://keats.github.io/tera/docs/)
for more info.


### Adding API routes

A little simpler. Add a route:

```rust
    let app = Router::new()
        ...
        .route(&CONFIG.route_api_v1("/map/upload"), post(post_map_upload))
        ...
```

Define the handler:

```rust
// Create types for the request and response. This example handler doesn't have
// a separate request type since it's using multipart, but requests are defined
// exactly the same way, except with Deserialize instead of Serialize.

#[derive(Serialize, TS)] // Make the type serializable, and definable in typescript
#[ts(export)] // Export the type to *frontend/modules/src/bindigs*
#[serde(tag = "type")] // Add a .type property to allow introspection in TS
struct MapUploadResponse {
    map_id: i32,
}

async fn post_map_upload(
    State(state): State<AppState>,

    // Not optional now - require that a user interacting with this endpoint is
    // authenticated by OAuth, and has the relevant server-side session state
    session: NadeoAuthSession,

    // Perhaps other extractors, like Json(...). Use `WithRejection` to wrap the
    // extractor's rejection error in an ApiError, if for example the request
    // sends invalid multipart data or JSON objects.
    WithRejection(mut multipart, _): WithRejection<Multipart, ApiError>,

    // ApiError will serialize into JSON as a TsApiError in the bindings folder.
    // This also applies to any rejections using WithRejection.
) -> Result<Json<MapUploadResponse>, ApiError> {
    ...
}
```


### Adding static resources

Add images, CSS, or plain JS files to *frontend/static*. They are served by a
`ServeDir` service under the web path */static*.


### Creating a new migration

```shell
sqlx migrate add -rs $migration_name
# edit migration up/down
sqlx migrate run
```

We prefer revertable migrations with sequential names (rather than date-based
ones). Theoretically you should be able to `revert`, edit the migration, and
`run` it again but I haven't tried this. If you screw up then GG, you might need
to restart from scratch lol


### Modifying an existing schema

```shell
sqlx migrate revert --target-version 0
# edit migration up+down
sqlx migrate run # alternatively, since we use the sqlx::migrate! macro, just run the backend
```


### Adding new queries or modifying existing ones

Edit the `query!` macro invocation, then run

```shell
cargo sqlx prepare -D "postgres://$user:$pass@$address:5432/accel_pen"
```

Sometimes also restart rust_analyzer (in VS Code at least). Once you're happy,
don't forget to include changes to the *.sqlx* directory in your commit.


## Architecture

The backend (*src*) is an [Axum](https://lib.rs/crates/axum) web and API server
connected to a postgres database. The types for API requests and responses are
generated by [ts-rs](https://lib.rs/crates/ts-rs) for consumption by the
frontend.

Accel Pen users correspond directly with Ubisoft accounts. An Accel Pen user
instance is created when authenticating through OAuth for the first time, and
returned on subsequent logins.

The frontend (*frontend*) is a series of [Tera](https://lib.rs/crates/tera)
templates rendered by the backend, using data filled in from the postgres
database, the Nadeo OAuth API, or the Nadeo Ubisoft-credentialed API.

This is a little cursed because I want server-side rendering without a
framework. Dioxus and Next.JS exist and are probably pretty nice, but they seem
too magical. React was possible to add without too much pain.


### Gamebox file support (*gbx_rs*)

A normal Rust library. Fuzzing is supported with
[cargo-fuzz](https://lib.rs/crates/cargo-fuzz).


### LZO decompression (*lzokay-native-rs*)

Forked from https://github.com/arma-tools/lzokay-native-rs due to some issues discovered in fuzzing.


### Configuration with environment variables (*from_env*)

Enables overriding config file values with structured environment variable names.

