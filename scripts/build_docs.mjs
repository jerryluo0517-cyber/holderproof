#!/usr/bin/env node
// Generate HTML, diagram PNG and Markdown tokens for the companion PDF builder.
// Dependencies are optional: npm install --prefix docs.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

const require = createRequire(new URL('../docs/package.json', import.meta.url));
const { marked } = await import(pathToFileURL(require.resolve('marked')).href);
const sharp = require('sharp');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(await fs.readFile(path.join(root, 'scripts/docs_manifest.json'), 'utf8'));
const diagramPath = path.join(root, 'docs/architecture/assets/holderproof-flow.svg');
const diagram = await fs.readFile(diagramPath, 'utf8');
await sharp(Buffer.from(diagram)).png().toFile(diagramPath.replace('.svg', '.png'));

const css = `
:root { color-scheme: light; }
body { margin: 0 auto; max-width: 1040px; padding: 48px 40px; color: #243449;
  font: 16px/1.8 "PingFang SC", "Microsoft YaHei", Arial, sans-serif; background: #fff; }
h1 { color: #183153; font-size: 32px; line-height: 1.4; border-top: 6px solid #0f766e; padding-top: 24px; }
h2 { color: #183153; font-size: 23px; border-bottom: 1px solid #cbd5e1; padding-bottom: 7px; margin-top: 34px; }
a { color: #0f766e; } table { width: 100%; border-collapse: collapse; margin: 20px 0; }
th, td { border: 1px solid #cbd5e1; padding: 10px 13px; text-align: left; vertical-align: top; }
th { color: #183153; background: #eff6ff; } tr:nth-child(even) td { background: #f8fafc; }
pre { padding: 16px; background: #f1f5f9; border-left: 3px solid #0f766e; white-space: pre-wrap; overflow-wrap: anywhere; }
code { font: .88em/1.6 Menlo, Consolas, monospace; } p, li { overflow-wrap: anywhere; }
svg, img { display: block; width: 100%; height: auto; } .source-note { color: #52657b; font-size: 14px; }
@media print { body { padding: 0; font-size: 10pt; } h1,h2 { break-after: avoid; } tr,pre { break-inside: avoid; } }
`;

const exports = [];
for (const document of manifest) {
  const markdown = await fs.readFile(path.join(root, document.source), 'utf8');
  const title = markdown.split('\n')[0].replace(/^# /, '');
  const tokens = marked.lexer(markdown);
  const displayMarkdown = markdown.replace(/```mermaid[\s\S]*?```/g,
    '时序图的可编辑 Mermaid 源码见本页对应的 Markdown；上方架构图可离线查看。');
  let body = marked.parse(displayMarkdown);
  body = body.replace(/<img[^>]+holderproof-flow\.svg[^>]*>/g, diagram);
  const html = `<!doctype html>\n<html lang="zh-CN"><head><meta charset="utf-8"><title>${title}</title><style>${css}</style></head><body>${body}</body></html>\n`;
  await fs.writeFile(path.join(root, document.html), html);
  exports.push({ ...document, title, tokens });
}
await fs.mkdir(path.join(root, '.run/docs'), { recursive: true });
await fs.writeFile(path.join(root, '.run/docs/tokens.json'), JSON.stringify(exports));
console.log(`Generated diagram PNG and ${exports.length} HTML documents.`);
