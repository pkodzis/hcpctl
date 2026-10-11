#!/usr/bin/env bash
# Generate book/sitemap.xml from the built mdBook output so new pages are
# always listed. Paths are lowercased because Netlify 301-redirects
# mixed-case URLs (e.g. /CommandLineHelp.html -> /commandlinehelp).
set -euo pipefail

SITE_URL="https://hcpctl.com"
BOOK_DIR="${1:-book}"

{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">'
  echo "  <url><loc>${SITE_URL}/</loc></url>"
  find "$BOOK_DIR" -name '*.html' \
    ! -name 'index.html' ! -name '404.html' ! -name 'print.html' ! -name 'toc.html' \
    -printf '%P\n' | sort | while read -r page; do
      echo "  <url><loc>${SITE_URL}/$(echo "$page" | tr '[:upper:]' '[:lower:]')</loc></url>"
    done
  echo '</urlset>'
} > "$BOOK_DIR/sitemap.xml"
