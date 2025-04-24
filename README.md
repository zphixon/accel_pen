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
2. If you've changed any queries in the project:
    1. Close any running containers.
    2. Run `docker compose -f database.docker-compose.yml -f prepare.docker-compose.yml up` to run the
       database container in dev mode.
    3. Set the DATABASE_URL environment variable to point to the database container. Add the `root`
       user and password from the password file to the URL. For example:
       `mysql://root:$(cat ../secret_root_pw.txt)@localhost:3306/accel_pen`
       If you're using WSL, get the IP from `wsl hostname -I`. 
    3. Run `cargo sqlx prepare` in the `backend` directory to update the `.sqlx` directory.
    4. Commit changes to the `.sqlx` directory.
3. Otherwise, run `docker compose -f docker-compose.yml -f dev.docker-compose.yml up --build` to build
   and run in dev mode. This takes a million years though, so you will definitely want to write a
   custom backend config file (which would let you just run the database container on its own).
4. If you've changed the database schema, IDK


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
