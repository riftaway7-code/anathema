# anathema.summyt.net

The landing page. Vite + React, prerendered to static HTML at build time, served as static assets by Cloudflare.

```
npm install
npm run dev        # http://localhost:5174
npm run build      # dist/, prerendered
npm run deploy     # build + wrangler deploy (needs Cloudflare access to summyt.net)
```

All copy is in `src/content.js`.

## The hero render

The monitor is a studio render made in Blender (Cycles). The screen shows real QEMU captures of Anathema booting, from `render/captures/`. To change it:

```
blender -b -P render/monitor.py -- $PWD/render/captures/kernel.png $PWD/render/captures/loader.png OUT --frames 40 --samples 96 --size 1440x810
npm run frames -- OUT     # webp frame sets in public/frames/, og.png
```

On Apple silicon a frame takes about 12 seconds. `--only N` renders one frame for a quick look, and `--from N` resumes a turn. The page scrubs `public/frames/*/turn_*.webp` with scroll; `boot_off` and `boot_loader` are the power-on intro.
