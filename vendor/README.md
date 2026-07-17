# Vendored engine crates

`MERAWLER` and `ZERAWLER` live here so GitHub Actions / Windows / Linux builds
can compile without sibling Desktop checkouts.

Upstream working trees (optional, for engine development):

- `~/Desktop/MERAWLER`
- `~/Desktop/ZERAWLER`

When you change those, sync back into this folder before releasing:

```bash
rsync -a --delete --exclude target --exclude .git --exclude workers \
  ~/Desktop/MERAWLER/ vendor/MERAWLER/
rsync -a --delete --exclude target --exclude .git --exclude workers \
  ~/Desktop/ZERAWLER/ vendor/ZERAWLER/
```
