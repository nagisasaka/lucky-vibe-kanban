# Product website

This directory contains the static Lucky Vibe Kanban product website.
It uses HTML, CSS and JavaScript directly; no package installation or application
build is required.

## Edit and preview

Edit `index.html`, `styles.css`, `app.js` and the files in `assets/`.
Keep local asset references relative so the site works under the GitHub Pages
project path.

From the repository root, preview with any static HTTP server, for example:

```sh
python3 -m http.server 4173 --directory website
```

Open `http://localhost:4173/`. Check desktop and narrow layouts, keyboard
navigation, the illustrative workflow, reduced-motion behavior and outbound links.

The main copy and section order follow LOCAL-439. Keep the hero in sync with the
repository README. Operational instructions remain in the linked OpenWiki pages.

## Publish

The [Publish website workflow](../.github/workflows/publish-website.yml) publishes
this directory when changes to `website/` or the workflow reach `main`.
It can also be run manually from `main`.

For the initial deployment, select **GitHub Actions** as the source in the
repository's **Settings → Pages → Build and deployment**.

The expected project URL is:
https://nagisasaka.github.io/lucky-vibe-kanban/

The custom domain is intentionally not connected. Do not add a `CNAME` file or
change DNS as part of ordinary website updates.

## Assets and attribution

The site uses the repository's official clover logo and LVK Rabbit by Masato
Fujino. The Rabbit artwork is unmodified and licensed under CC BY 4.0; retain
the attribution, source and license links and `assets/LVK-Rabbit-LICENSE.txt`. Keep explanatory illustrations distinguishable from
screenshots of the running product.

