# RepoAtlas brand mark

The approved RepoAtlas mark uses three amber index bars and a continuous
negative-space path inside a graphite app tile. It represents scattered local
Projects becoming one legible, navigable index.

## Source and generated assets

- `repoatlas-mark.png` is the approved high-resolution master artwork.
- `public/repoatlas-mark.png` is the optimized desktop UI and favicon copy.
- `website/assets/repoatlas-mark.png` is the optimized GitHub Pages copy.
- `src-tauri/icons/` contains platform derivatives generated from the master.

Regenerate the platform derivatives from the repository root:

```powershell
pnpm tauri icon assets/brand/repoatlas-mark.png
```

## Usage

- Keep the mark square and preserve its transparent outer canvas.
- Use it at 24 px or larger in product and website interfaces.
- Keep clear space around it equal to at least one eighth of its displayed size.
- Use the approved asset without recoloring, stretching, rotating, cropping, or
  adding effects.
- Pair it with the RepoAtlas wordmark in Geist or the surrounding product font;
  do not typeset text inside the mark.

The approved palette is graphite `#0D0D0D` and amber `#FFA31A`. The source
artwork contains subtle tonal rendering; do not add further gradients, shadows,
outlines, or texture in downstream use.
