// The compiler writes the manifest and the asset list beside the bundled program; the extension needs them in dist/.
import { copyFileSync, readFileSync } from 'node:fs';
copyFileSync('src/generated/manifest.json', 'dist/manifest.json');
for (const { from, to } of JSON.parse(readFileSync('src/generated/assets.json', 'utf8'))) {
  copyFileSync(from, `dist/${to}`);
}
