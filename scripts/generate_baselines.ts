import { chromium } from 'playwright';
import * as fs from 'fs';
import * as path from 'path';

const FIXTURES_DIR = path.join(__dirname, '..', 'test_fixtures');
const DEFAULT_VIEWPORT = { width: 800, height: 600 };

interface ElementInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  background: string;
  className: string;
  tagName: string;
}

interface Viewport {
  width: number;
  height: number;
}

function parseViewportFromHtml(htmlContent: string): Viewport {
  // Look for <meta name="viewport-size" content="WIDTHxHEIGHT">
  const match = htmlContent.match(/<meta\s+name="viewport-size"\s+content="(\d+)x(\d+)"/);
  if (match) {
    return { width: parseInt(match[1], 10), height: parseInt(match[2], 10) };
  }
  return DEFAULT_VIEWPORT;
}

async function generateBaseline(htmlPath: string): Promise<void> {
  const htmlContent = fs.readFileSync(htmlPath, 'utf-8');
  const viewport = parseViewportFromHtml(htmlContent);

  const browser = await chromium.launch();
  const page = await browser.newPage();
  await page.setViewportSize(viewport);

  const absolutePath = path.resolve(htmlPath);
  await page.goto(`file://${absolutePath}`);

  // Wait for any potential rendering to complete
  await page.waitForTimeout(100);

  // Extract all element bounding boxes
  const elements = await page.evaluate(() => {
    const result: ElementInfo[] = [];

    function rgbToString(color: string): string {
      // Convert rgb/rgba to consistent format
      if (!color || color === 'transparent' || color === 'rgba(0, 0, 0, 0)') {
        return 'rgba(0,0,0,0)';
      }
      // Parse rgb(r, g, b) or rgba(r, g, b, a)
      const match = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/);
      if (match) {
        const r = parseInt(match[1]);
        const g = parseInt(match[2]);
        const b = parseInt(match[3]);
        const a = match[4] !== undefined ? parseFloat(match[4]) : 1;
        return `rgba(${r},${g},${b},${a})`;
      }
      return color;
    }

    function traverse(el: Element): void {
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);

      // Only include elements with visible dimensions and non-transparent background
      if (rect.width > 0 && rect.height > 0) {
        const bg = rgbToString(style.backgroundColor);
        // Include body even without background, include elements with visible background
        if (el.tagName === 'BODY' || (bg && bg !== 'rgba(0,0,0,0)')) {
          result.push({
            x: Math.round(rect.x),
            y: Math.round(rect.y),
            width: Math.round(rect.width),
            height: Math.round(rect.height),
            background: bg,
            className: el.className || '',
            tagName: el.tagName.toLowerCase(),
          });
        }
      }

      for (const child of el.children) {
        traverse(child);
      }
    }

    traverse(document.body);
    return result;
  }) as ElementInfo[];

  // Generate SVG
  let svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${viewport.width}" height="${viewport.height}" viewBox="0 0 ${viewport.width} ${viewport.height}">\n`;
  svg += `  <rect x="0" y="0" width="${viewport.width}" height="${viewport.height}" fill="white"/>\n`;

  for (const el of elements) {
    const fill = el.background || 'rgba(0,0,0,0)';
    const dataClass = el.className ? ` data-class="${el.className}"` : '';
    svg += `  <rect x="${el.x}" y="${el.y}" width="${el.width}" height="${el.height}" fill="${fill}"${dataClass}/>\n`;
  }

  svg += '</svg>';

  const svgPath = htmlPath.replace('.html', '.svg');
  fs.writeFileSync(svgPath, svg);
  console.log(`Generated: ${svgPath}`);

  await browser.close();
}

async function main(): Promise<void> {
  console.log(`Looking for HTML files in: ${FIXTURES_DIR}`);

  if (!fs.existsSync(FIXTURES_DIR)) {
    console.error(`Fixtures directory not found: ${FIXTURES_DIR}`);
    process.exit(1);
  }

  const files = fs.readdirSync(FIXTURES_DIR).filter(f => f.endsWith('.html'));

  if (files.length === 0) {
    console.log('No HTML files found in fixtures directory');
    return;
  }

  console.log(`Found ${files.length} HTML files`);

  for (const file of files) {
    const htmlPath = path.join(FIXTURES_DIR, file);
    console.log(`Processing: ${file}`);
    await generateBaseline(htmlPath);
  }

  console.log('Done generating all baselines');
}

main().catch(err => {
  console.error('Error:', err);
  process.exit(1);
});
