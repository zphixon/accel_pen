# Accel Pen

the trackmania map corral

- map packs
  - map pack administrators
  - list of maps
  - submission approval
- map uploads
  - tags
    * tag voting
    * tag janitors?
  * like reddit but for maps
    * new/hot tabs, sortable by tag
  - link back to map pack
  - multiple authors
  - comments
  - awards
  - leaderboards
  - unlisted maps
  * links to ecircuitmania?
- daily featured maps
  - totd tie-in
- forums?
- ubi login


## deploying

https://stackoverflow.com/a/68916787/18270160

Set the environment variable ACCEL_PEN_DB_ROOT_PASSWORD_FILE to the path of a file containing the root
password for MySQL. Run `docker compose -f docker-compose.yml up` and that should be it.

## developing

1. Create a MySQL database root password by creating a file `secret_root_pw.txt` in the repo root.
2. Run `docker compose -f docker-compose.yml -f dev.docker-compose.yml up --build` to run the project in dev mode.
3. If you're planning on changing any queries in the backend:
  1. Run the project in a clean working tree. The database should be online while you make these changes.
  2. Set the DATABASE_URL environment variable to point to the database container. Add the `root`
     user and password from the password file to the URL. For example:
     `mysql://root:$(cat ../secret_root_pw.txt)@localhost:3306/accel_pen`
     If you're using WSL, get the IP from `wsl hostname -I`. 
  3. Once you're happy with the new queries, run `cargo sqlx prepare` in the `backend` directory to
     update the `.sqlx` directory.
  4. Commit changes to the `.sqlx` directory.
4. If you're planning on changing the database schema:
  1. For now, delete the database volume and re-build the compose project. Kind of annoying :/


## auth dings

- frontend
  - set window.location to backend `oauth_start` endpoint
- backend
  - set httpOnly cookie to a session ID
  - redirect to third party
- third party
  - user authenticates and authorizes the app
  - redirects to backend `oauth_finish` endpoint
- backend
  - check session ID cookie exists
  - get access+refresh token
  - create CSRF token
  - redirect to frontend with CSRF token in params
- frontend
  - receive CSRF token


then
- frontend
  - send session ID cookie and CSRF token
- backend
  - check session ID cookie corresponds with CSRF token


# React + TypeScript + Vite

This template provides a minimal setup to get React working in Vite with HMR and some ESLint rules.

Currently, two official plugins are available:

- [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react/README.md) uses [Babel](https://babeljs.io/) for Fast Refresh
- [@vitejs/plugin-react-swc](https://github.com/vitejs/vite-plugin-react-swc) uses [SWC](https://swc.rs/) for Fast Refresh

## Expanding the ESLint configuration

If you are developing a production application, we recommend updating the configuration to enable type-aware lint rules:

```js
export default tseslint.config({
  extends: [
    // Remove ...tseslint.configs.recommended and replace with this
    ...tseslint.configs.recommendedTypeChecked,
    // Alternatively, use this for stricter rules
    ...tseslint.configs.strictTypeChecked,
    // Optionally, add this for stylistic rules
    ...tseslint.configs.stylisticTypeChecked,
  ],
  languageOptions: {
    // other options...
    parserOptions: {
      project: ['./tsconfig.node.json', './tsconfig.app.json'],
      tsconfigRootDir: import.meta.dirname,
    },
  },
})
```

You can also install [eslint-plugin-react-x](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-x) and [eslint-plugin-react-dom](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-dom) for React-specific lint rules:

```js
// eslint.config.js
import reactX from 'eslint-plugin-react-x'
import reactDom from 'eslint-plugin-react-dom'

export default tseslint.config({
  plugins: {
    // Add the react-x and react-dom plugins
    'react-x': reactX,
    'react-dom': reactDom,
  },
  rules: {
    // other rules...
    // Enable its recommended typescript rules
    ...reactX.configs['recommended-typescript'].rules,
    ...reactDom.configs.recommended.rules,
  },
})
```
