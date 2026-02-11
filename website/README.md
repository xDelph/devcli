# DevCLI Website

Landing page and documentation site for DevCLI, built with Astro and Tailwind CSS.

## Setup

```bash
# Install dependencies
npm install

# Start dev server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Deployment

This site is configured to deploy to Vercel from the `website/` directory.

### Vercel Configuration

In your Vercel project settings:
- **Root Directory**: `website/`
- **Build Command**: `npm run build`
- **Output Directory**: `dist`
- **Install Command**: `npm install`

## Structure

```
website/
├── src/
│   ├── components/     # Reusable components
│   │   ├── Hero.astro
│   │   ├── Features.astro
│   │   ├── Showcase.astro
│   │   ├── Installation.astro
│   │   ├── Comparison.astro
│   │   └── Footer.astro
│   ├── layouts/
│   │   └── Layout.astro
│   └── pages/
│       └── index.astro  # Main landing page
├── public/             # Static assets (add images here)
├── astro.config.mjs
├── tailwind.config.mjs
└── package.json
```

## Adding Images

Place all screenshots and images in the `public/` directory:

```
public/
├── hero-terminal-demo.gif
├── tui-dashboard.png
├── tui-logs.png
├── terminal-status.png
├── config-example.png
└── ...
```

Then update the image placeholders in the components by replacing:

```html
<!-- [IMAGE PLACEHOLDER: Description] -->
<div class="...placeholder...">
```

With:

```html
<img src="/image-name.png" alt="Description" class="rounded-xl" />
```

## Required Images

See `IMAGES_NEEDED.md` for the complete list of required images and specifications.
