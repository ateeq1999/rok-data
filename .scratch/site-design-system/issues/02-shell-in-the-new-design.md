# 02: the shell in the new design

**Blocked by:** 01
**Status:** done

Spec: decisions 2, 4 and 5.

Header, Footer, ThemeToggle and `__root.tsx` use the shadcn tokens through
Tailwind classes (`bg-background`, `text-foreground`, `text-muted-foreground`,
`border-border`, `bg-card`), square corners, headings in the heading font.
Copy the template's `src/components/ui/` components the shell needs (button,
separator, and so on) into `site/src/components/ui/`, with their dependencies
at the template's versions.

Done when the shell references no old variable, builds, and renders in light,
dark and auto.
