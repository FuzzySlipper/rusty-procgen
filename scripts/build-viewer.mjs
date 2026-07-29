import { build } from 'esbuild';

await build({
  bundle: true,
  entryPoints: ['viewer/app.ts'],
  format: 'esm',
  outfile: 'dist/ts/viewer/app.js',
  platform: 'browser',
  target: 'es2022',
});
