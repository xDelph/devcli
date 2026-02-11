# DevCLI Website Setup Guide

## ✅ What's Been Created

A stunning, modern landing page for DevCLI with:

- **Hero Section** - Animated gradient background, floating orbs, installation command
- **Features Grid** - 6 key features with gradient icons and hover effects
- **Showcase** - Screenshot placeholders for TUI and terminal demos
- **Installation** - Multiple installation methods with step-by-step guide
- **Comparison** - DevCLI vs PM2, Docker Compose, and custom scripts
- **Footer** - Links, branding, and social connections

### Design Highlights

🎨 **Visual Design:**
- Dark theme with blue/cyan gradient accents
- Smooth animations and hover effects
- Glassmorphism (backdrop blur effects)
- Floating orbs and grid patterns
- Modern, professional aesthetic

⚡ **Interactions:**
- Smooth scroll navigation
- Hover scale effects on cards
- Animated gradient backgrounds
- Copy-to-clipboard for install commands
- Responsive design (mobile-friendly)

🚀 **Performance:**
- Static site generation (fast loading)
- Optimized Tailwind CSS
- No heavy dependencies
- SEO optimized

---

## 📁 Project Structure

```
website/
├── src/
│   ├── components/
│   │   ├── Hero.astro          # Hero section with terminal demo
│   │   ├── Features.astro      # Feature grid (6 cards)
│   │   ├── Showcase.astro      # Screenshot sections
│   │   ├── Installation.astro  # Install methods + quick start
│   │   ├── Comparison.astro    # vs other tools
│   │   └── Footer.astro        # Footer with links
│   ├── layouts/
│   │   └── Layout.astro        # Base layout with fonts & SEO
│   └── pages/
│       └── index.astro         # Main landing page
├── public/
│   └── favicon.svg             # Favicon (gradient "D")
├── astro.config.mjs            # Astro configuration
├── tailwind.config.mjs         # Tailwind with custom theme
├── package.json                # Dependencies
├── tsconfig.json               # TypeScript config
├── .gitignore                  # Git ignore rules
├── README.md                   # Development guide
├── IMAGES_NEEDED.md           # Complete image list
└── SETUP_GUIDE.md             # This file
```

---

## 🚀 Quick Start

### 1. Install Dependencies

```bash
cd website
npm install
```

### 2. Start Development Server

```bash
npm run dev
```

Open http://localhost:4321

### 3. Build for Production

```bash
npm run build
```

### 4. Preview Production Build

```bash
npm run preview
```

---

## 📸 Adding Images

### Priority Order:

1. **MUST HAVE** (before launch):
   - Hero terminal demo GIF
   - TUI dashboard screenshot
   - TUI logs screenshot
   - Config example screenshot

2. **SHOULD HAVE** (launch with if possible):
   - Terminal status output
   - Terminal start output
   - Terminal metrics output

3. **NICE TO HAVE** (can add later):
   - Auto-add interactive
   - Health check monitoring
   - Architecture diagram

### Where to Add Images:

1. Save images in `website/public/`
2. Replace placeholder `<div>` elements in components
3. See `IMAGES_NEEDED.md` for complete specifications

### Example Replacement:

In `src/components/Hero.astro`, find:
```html
<!-- [IMAGE PLACEHOLDER: Terminal demo animation or screenshot] -->
<div class="aspect-video bg-gradient-to-br...">
  <!-- placeholder content -->
</div>
```

Replace with:
```html
<img
  src="/hero-terminal-demo.gif"
  alt="DevCLI terminal demo"
  class="w-full rounded-xl shadow-2xl"
/>
```

---

## 🎨 Customization

### Colors

Edit `tailwind.config.mjs` to change the color scheme:

```js
colors: {
  primary: {
    500: '#0ea5e9',  // Main blue
    600: '#0284c7',
    // ...
  },
}
```

### Fonts

Fonts are loaded from Google Fonts in `Layout.astro`:
- **Inter** - Body text
- **JetBrains Mono** - Code/terminal text

To change fonts, edit the Google Fonts link in `src/layouts/Layout.astro`.

### Content

Edit component files directly:
- **Hero.astro** - Tagline, description
- **Features.astro** - Feature list and descriptions
- **Installation.astro** - Install commands
- **Footer.astro** - Links and branding

---

## 🚀 Deployment to Vercel

### Method 1: Vercel Dashboard (Easiest)

1. Go to [vercel.com](https://vercel.com)
2. Click "Add New Project"
3. Import your GitHub repository
4. Configure:
   - **Root Directory**: `website`
   - **Framework Preset**: Astro
   - **Build Command**: `npm run build`
   - **Output Directory**: `dist`
5. Click "Deploy"

### Method 2: Vercel CLI

```bash
# Install Vercel CLI
npm i -g vercel

# Deploy from website directory
cd website
vercel

# Follow prompts
```

### Environment Variables

No environment variables needed for this static site!

---

## 📱 Preview on Mobile

The site is fully responsive. To test on mobile:

### Local Network Testing:

```bash
# Start dev server
npm run dev

# Find your local IP
# macOS/Linux:
ipconfig getifaddr en0

# Visit from mobile:
# http://YOUR_IP:4321
```

### Use Vercel Preview:

Every push to GitHub creates a preview URL you can test on any device.

---

## 🎯 Next Steps

### Before Launch:
1. ✅ Create the required images (see `IMAGES_NEEDED.md`)
2. ✅ Add images to `public/` directory
3. ✅ Replace image placeholders in components
4. ✅ Test on desktop and mobile
5. ✅ Deploy to Vercel

### After Launch:
1. Add documentation pages (Getting Started, Configuration, etc.)
2. Add search functionality
3. Add analytics (Vercel Analytics or Plausible)
4. Consider adding:
   - Blog for tutorials
   - Interactive demo/playground
   - Video walkthroughs

---

## 🐛 Troubleshooting

### Port already in use:
```bash
# Kill process on port 4321
npx kill-port 4321

# Or use different port
npm run dev -- --port 3000
```

### Build fails:
```bash
# Clear cache and rebuild
rm -rf node_modules .astro dist
npm install
npm run build
```

### Images not loading:
- Ensure images are in `public/` directory
- Use `/image-name.png` path (leading slash)
- Check file names match exactly (case-sensitive)

---

## 📚 Resources

- [Astro Documentation](https://docs.astro.build)
- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
- [Vercel Documentation](https://vercel.com/docs)

---

## 🎨 Design Philosophy

This landing page follows modern design trends:

- **Brutalist Minimalism** - Clean, bold, no unnecessary elements
- **Glassmorphism** - Backdrop blur and transparency effects
- **Dark Mode First** - Perfect for developer tools
- **Gradient Accents** - Blue/cyan for energy and technology
- **Micro-interactions** - Hover effects, smooth transitions
- **Typography** - Clear hierarchy, readable fonts

The goal: Make visitors **immediately curious** about DevCLI and **excited to try it**.

---

## 💡 Tips for Screenshots

### Terminal Setup:
- Use a dark theme (Dracula, Nord, Tokyo Night)
- Increase font size (16-18pt for screenshots)
- Use a wider terminal window (120+ columns)
- Hide unnecessary UI elements

### TUI Screenshots:
- Run with actual processes (not empty)
- Show realistic data (not just test apps)
- Capture at high resolution (2x or 3x)
- Ensure text is crisp and readable

### Recording GIFs:
- Keep loops short (3-5 seconds)
- Use realistic typing speed
- Show only essential commands
- Optimize file size (< 5MB ideal)

---

## ✨ What Makes This Special

This isn't just another documentation site. It's designed to:

1. **Impress immediately** - Stunning hero with animation
2. **Explain clearly** - Features are concise and benefit-focused
3. **Show, don't tell** - Screenshots over descriptions
4. **Remove friction** - One-line install command front and center
5. **Build trust** - Professional design signals quality tool

Every element is crafted to convert curious visitors into excited users.

---

**Ready to launch! 🚀**

Once you add the screenshots, you'll have a world-class landing page for DevCLI.
