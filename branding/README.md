# GitTree brand assets

Everything in this folder is generated from `source/concept.png` by `scripts/build-brand.sh`. Do not edit the SVG or PNG files by hand; change the source or the script and regenerate.

```bash
pip install -r scripts/requirements-brand.txt   # vtracer, potracer, numpy, scipy
./scripts/build-brand.sh                        # needs rsvg-convert, magick and cargo-tauri
```

The script also rewrites `src-tauri/icons/` and `frontend/public/` (favicon and title bar logos).

## Elements

| Element | Files | What it is |
| --- | --- | --- |
| Logo | `logo*.svg` | Tree mark above the wordmark. |
| Horizontal logo | `logo-horizontal*.svg` | Mark left, wordmark right. |
| Mark | `mark*.svg` | Tree alone, full detail. |
| Wordmark | `wordmark*.svg` | The word "gittree" alone. |
| Small mark | `mark-small*.svg`, `app-icon-small.svg` | Thickened silhouette with the penguin outline. |
| Tiny mark | `mark-tiny*.svg`, `app-icon-tiny.svg`, `favicon.svg` | Thickened solid silhouette, no penguin. |
| App icon | `app-icon.svg` | Mark on a rounded navy square. |
| Concept | `source/concept.png` | Approved design the whole set is traced from. |

## Colour variants

| Suffix | Use on | Notes |
| --- | --- | --- |
| none | White or very light backgrounds | Full colour. Default choice. |
| `-dark` | Dark backgrounds (dark themes, dark pages) | Lightened version of the full colour set. |
| `-mono` | Light backgrounds where only one ink is available | Single navy, e.g. print, stamps, fax, documents. |
| `-white` | Photos or saturated backgrounds, dark brand panels | Single white, needs enough contrast behind it. |

Backgrounds are transparent. Counters (the hole in the "g") and the node dots are real holes, so the background shows through.

## Which asset to use

| Context | Asset |
| --- | --- |
| README, docs, website header, slides | `logo.svg` (light) or `logo-dark.svg` |
| Narrow banner, email signature, page header | `logo-horizontal.svg` / `-dark` |
| Social preview, release cards | `png/social-preview.png` (1280x640) |
| Product name next to other UI | `wordmark.svg` / `-dark` |
| Avatar, square tile, large app icon | `app-icon.svg` or `png/app-icon-1024.png` |
| Window title bar | `tiny` mark: `frontend/public/logo.png` and `logo-dark.png`, swapped by theme |
| Browser tab | `favicon.svg` (follows the system colour scheme) |
| One-colour documents | `*-mono.svg` |
| Over images or dark panels | `*-white.svg` |

## Size rules

The detail level changes with size. Use the version that matches the rendered size, not the largest one scaled down.

| Rendered size | Use |
| --- | --- |
| 64 px and up | Full mark or app icon (penguin visible). |
| 48 px | Small mark (`app-icon-small`): thickened, penguin outline. |
| 47 px and below (16, 24, 32) | Tiny mark (`app-icon-tiny`, `favicon.svg`): solid, no penguin. |

Other minimums:

- Vertical logo: 96 px wide.
- Horizontal logo: 160 px wide.
- Wordmark alone: 80 px wide.

Below those widths, drop the wordmark and use the mark.

## Clear space

Keep free space around the logo equal to at least one eighth of the mark's height on every side. No text, edges or other graphics inside it.

## Do

- Use the variant that contrasts with the background: default on light, `-dark` or `-white` on dark.
- Scale proportionally.
- Use the exported files in `png/` when SVG is not supported (office tools, some chat clients).

## Don't

- Don't recolour, apply gradients or shadows, or change the blues.
- Don't stretch, rotate or crop the tree.
- Don't re-type the wordmark in another font or separate the two "t" letters.
- Don't place the default variant on dark or busy backgrounds.
- Don't show the penguin at 32 px or smaller; use the tiny mark.
- Don't edit generated files; regenerate them.

## Colours

| Role | Hex |
| --- | --- |
| Navy (mono, wordmark "git") | `#0F1F4D` |
| Blue (wordmark "tree") | `#1E7BFF` |
| Cyan (leaf highlights) | `#2FB4FF` |
| Dark-mode accent (tiny/small on dark) | `#7DB6FF` |
| App icon background | `#12338F` to `#071A55` (vertical gradient) |

The full-colour tree and wordmark are traced gradients; use the files rather than rebuilding them from these values.

## Generated outputs

- `png/`: raster exports of the logos (1024 px wide), the mark (16 to 1024 px), app icons (16 to 1024 px), the tiny mark for the title bar (64 px) and the social preview.
- `src-tauri/icons/`: Tauri bundle icons. 16/24/32 px come from the tiny icon, 48 px from the small icon, 64 px and up from the full app icon (`icon.ico` holds all of them).
- `frontend/public/`: `favicon.svg`, `logo.png`, `logo-dark.png`.

## Known limits

- The source image is 552 px, so very large prints show soft edges. Replace `source/concept.png` with a higher-resolution original and regenerate when available.
- The `-dark` variant is the light colour set lightened automatically; blues are slightly muted.
