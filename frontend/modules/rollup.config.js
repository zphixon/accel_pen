import { fileURLToPath } from 'node:url';
import { globSync } from 'glob';
import nodeResolve from '@rollup/plugin-node-resolve';
import path from 'node:path';
import typescript from '@rollup/plugin-typescript';
import commonjs from '@rollup/plugin-commonjs';
import replace from '@rollup/plugin-replace';

function doGlob(pattern) {
  return globSync(pattern).map(file => [
    path.relative("src", file.slice(0, file.length - path.extname(file).length)),
    fileURLToPath(new URL(file, import.meta.url))
  ])
}

export default {
  input: Object.fromEntries([
    ...doGlob("src/**/*.ts"),
    ...doGlob("src/**/*.tsx"),
  ]),
  output: {
    dir: "../static/js",
    format: "es",
    sourcemap: true,
  },
  plugins: [
    replace({
      preventAssignment: true,
      "process.env.NODE_ENV": JSON.stringify("development"),
    }),
    nodeResolve({ browser: true }),
    commonjs({
      include: /node_modules/,
      requireReturnsDefault: 'auto',
    }),
    typescript(),
  ],
  jsx: "react-jsx",
};
