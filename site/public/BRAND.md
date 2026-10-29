# rok Brand Guidelines

> **Run One. Know All.**

---

## Logo

### Primary Logo

![rok Logo](/logo.svg)

The rok logo features:
- **Abstract "R"** — Represents the runtime/execution engine
- **Orbit rings** — Symbolize automation and continuous execution
- **Center dot** — Represents "One JSON"
- **Accent dots** — Represent "All Changes"
- **Gradient** — Cyan to Emerald, representing transformation

### Favicon

![Favicon](/favicon.svg)

Simplified version for small sizes (32x32).

### Usage

- **Minimum size**: 24px height
- **Clear space**: Equal to the height of the "R" on all sides
- **Background**: Use on light or dark backgrounds
- **Don't**: Stretch, rotate, or modify colors

---

## Color Palette

### Primary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| `rok-cyan` | `#06b6d4` | `rgb(6, 182, 212)` | Primary gradient start |
| `rok-teal` | `#14b8a6` | `rgb(20, 184, 166)` | Primary gradient middle |
| `rok-emerald` | `#10b981` | `rgb(16, 185, 129)` | Primary gradient end |

### Secondary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| `rok-deep-cyan` | `#0891b2` | `rgb(8, 145, 178)` | Hover states |
| `rok-deep-teal` | `#0d9488` | `rgb(13, 148, 136)` | Active states |
| `rok-deep-emerald` | `#059669` | `rgb(5, 150, 105)` | Success states |

### Neutral Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| `rok-ink` | `#0f172a` | `rgb(15, 23, 42)` | Primary text (light mode) |
| `rok-ink-soft` | `#475569` | `rgb(71, 85, 105)` | Secondary text |
| `rok-line` | `#e2e8f0` | `rgb(226, 232, 240)` | Borders (light mode) |
| `rok-bg` | `#ffffff` | `rgb(255, 255, 255)` | Background (light mode) |

### Dark Mode Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| `rok-ink-dark` | `#f1f5f9` | `rgb(241, 245, 249)` | Primary text (dark mode) |
| `rok-ink-soft-dark` | `#94a3b8` | `rgb(148, 163, 184)` | Secondary text |
| `rok-line-dark` | `#1e293b` | `rgb(30, 36, 51)` | Borders (dark mode) |
| `rok-bg-dark` | `#0f172a` | `rgb(15, 23, 42)` | Background (dark mode) |

### Accent Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| `rok-error` | `#ef4444` | `rgb(239, 68, 68)` | Errors, destructive actions |
| `rok-warning` | `#f59e0b` | `rgb(245, 158, 11)` | Warnings |
| `rok-success` | `#10b981` | `rgb(16, 185, 129)` | Success states |
| `rok-info` | `#3b82f6` | `rgb(59, 130, 246)` | Information |

---

## Typography

### Font Family

```css
font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
```

### Font Sizes

| Name | Size | Line Height | Usage |
|------|------|-------------|-------|
| `text-xs` | `0.75rem` (12px) | `1rem` | Captions, labels |
| `text-sm` | `0.875rem` (14px) | `1.25rem` | Body small |
| `text-base` | `1rem` (16px) | `1.5rem` | Body default |
| `text-lg` | `1.125rem` (18px) | `1.75rem` | Lead text |
| `text-xl` | `1.25rem` (20px) | `1.75rem` | H3 |
| `text-2xl` | `1.5rem` (24px) | `2rem` | H2 |
| `text-3xl` | `1.875rem` (30px) | `2.25rem` | H1 small |
| `text-4xl` | `2.25rem` (36px) | `2.5rem` | H1 |
| `text-6xl` | `3.75rem` (60px) | `1` | Display |

### Font Weights

| Name | Weight | Usage |
|------|--------|-------|
| `font-normal` | `400` | Body text |
| `font-medium` | `500` | Emphasis |
| `font-semibold` | `600` | Headings |
| `font-bold` | `700` | Strong emphasis |

---

## Theme Tokens

### CSS Variables (Light Mode)

```css
:root {
  /* Primary Gradient */
  --rok-cyan: #06b6d4;
  --rok-teal: #14b8a6;
  --rok-emerald: #10b981;
  
  /* Primary Gradient (Deep) */
  --rok-deep-cyan: #0891b2;
  --rok-deep-teal: #0d9488;
  --rok-deep-emerald: #059669;
  
  /* Text */
  --sea-ink: #0f172a;
  --sea-ink-soft: #475569;
  
  /* UI */
  --line: #e2e8f0;
  --header-bg: rgba(255, 255, 255, 0.8);
  --chip-bg: #ffffff;
  --chip-line: #e2e8f0;
  
  /* Accents */
  --error: #ef4444;
  --warning: #f59e0b;
  --success: #10b981;
  --info: #3b82f6;
}
```

### CSS Variables (Dark Mode)

```css
[data-theme="dark"] {
  /* Text */
  --sea-ink: #f1f5f9;
  --sea-ink-soft: #94a3b8;
  
  /* UI */
  --line: #1e293b;
  --header-bg: rgba(15, 23, 42, 0.8);
  --chip-bg: #1e293b;
  --chip-line: #334155;
  
  /* Background */
  --bg: #0f172a;
}
```

---

## Component Styles

### Buttons

```css
/* Primary Button */
.btn-primary {
  background: linear-gradient(135deg, var(--rok-cyan), var(--rok-teal));
  color: white;
  border-radius: 9999px;
  padding: 0.5rem 1.25rem;
  font-weight: 600;
  transition: transform 0.15s ease;
}

.btn-primary:hover {
  transform: translateY(-2px);
  background: linear-gradient(135deg, var(--rok-deep-cyan), var(--rok-deep-teal));
}

/* Secondary Button */
.btn-secondary {
  background: rgba(79, 184, 178, 0.1);
  color: var(--sea-ink);
  border: 1px solid var(--line);
  border-radius: 9999px;
  padding: 0.5rem 1.25rem;
  font-weight: 600;
  transition: all 0.15s ease;
}

.btn-secondary:hover {
  background: rgba(79, 184, 178, 0.15);
  border-color: var(--rok-teal);
}
```

### Cards

```css
.feature-card {
  background: var(--chip-bg);
  border: 1px solid var(--line);
  border-radius: 1rem;
  padding: 1.25rem;
  transition: all 0.2s ease;
}

.feature-card:hover {
  border-color: var(--rok-teal);
  box-shadow: 0 8px 24px rgba(30, 90, 72, 0.08);
}
```

### Code Blocks

```css
pre {
  background: var(--rok-bg-dark);
  border-radius: 0.5rem;
  padding: 1rem;
  overflow-x: auto;
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 0.875rem;
  line-height: 1.6;
}
```

---

## Usage Examples

### Gradient Text

```css
.gradient-text {
  background: linear-gradient(90deg, var(--rok-cyan), var(--rok-teal), var(--rok-emerald));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}
```

### Glow Effect

```css
.glow {
  box-shadow: 0 0 20px rgba(20, 184, 166, 0.4);
}
```

### Orbit Animation

```css
@keyframes orbit {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.orbit {
  animation: orbit 20s linear infinite;
}
```

---

## Brand Voice

### Tone

- **Confident** — We know what we're doing
- **Direct** — No fluff, just facts
- **Empowering** — Enable developers to build faster
- **Technical** — Speak the language of developers

### Taglines

- **Primary**: "Run One. Know All."
- **Secondary**: "One JSON. All Changes."
- **Descriptive**: "AI-native execution engine"

### Do's

- ✅ Use active voice
- ✅ Keep sentences short
- ✅ Include code examples
- ✅ Link to documentation

### Don'ts

- ❌ Use jargon without explanation
- ❌ Write walls of text
- ❌ Make claims without proof
- ❌ Use passive voice

---

## File Structure

```
docs/public/
├── logo.svg          # Primary logo
├── logo-dark.svg     # Logo for dark backgrounds
├── logo-icon.svg     # Icon only (no text)
├── favicon.svg       # Browser favicon
├── favicon-32.png    # PNG fallback
├── og-image.png      # Social sharing image
└── brand/
    ├── logo-guide.pdf    # Logo usage guide
    └── color-palette.pdf # Color reference
```

---

**rok** — Run One. Know All.
