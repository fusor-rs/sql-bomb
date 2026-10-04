# sql-bomb website

The landing page at `/` and documentation at `/docs/` are Fusor browser apps in
this Cargo workspace, separate from the native terminal. They follow Hypercmd's
`apps/` setup and share its pinned `docs-base` components. Documentation in
`../docs/` describes the behavior in the repository README. Brand assets come
from `../assets/brand/`; the builds copy them into ignored public directories.
The landing build also copies the root `install.sh` to serve at `/install.sh` on
`https://boom.fusor.build`. Edit the root script; the public copy is generated.

## Build and preview

Install Rust, Node.js 22 and [just](https://just.systems/), then run from the
repository root:

```sh
just setup-browser
just site
just preview
```

Open `http://127.0.0.1:4187`. The combined static output is `apps/dist/`.
`just landing-dev` and `just docs-dev` serve individual apps. Restart the docs
server after changing `docs/`, which is outside its watched app directory.
`FUSOR_BIN` selects an alternate Fusor CLI. `just site-check` runs formatting,
Clippy and tests for both apps; `just site-browser` builds and exercises the
combined site in Chromium. The Check workflow runs both alongside native checks.

The landing walkthrough displays SVG captures of the native application: its
workspace, results, cell inspector, query library and action menu. Stages rotate
automatically, pause while hovered, and respect reduced-motion preferences.
Regenerate captures on macOS or Linux with Python 3 and `just capture-preview` after UI
changes. The tool compiles the same native modules and templates as `boom`,
executes the sample SQL through a local Flight SQL server backed by SQLite,
and records terminal colors and glyphs. Its query library lives in a temporary
directory; it does not use your saved connections or history. The captures are
committed assets, so website builds do not start a server or require Python.

Docs use the same navigation, search, theme, Markdown downloads and history
routing as Hypercmd. Add guides to `../docs/` and register their slugs in
`docs/navigation.json`. Both apps require JavaScript and WebAssembly; the docs
do not prerender a separate HTML document for each route.

## Vercel deployment

Create a Vercel project for sql-bomb with the repository root as its root
directory. Create a GitHub environment named `vercel` with:

| Kind | Name | Value |
| --- | --- | --- |
| Secret | `VERCEL_TOKEN` | A Vercel token with access to the project |
| Variable | `VERCEL_ORG_ID` | The project's Vercel team/account ID |
| Variable | `VERCEL_PROJECT_ID` | The sql-bomb Vercel project ID |

Run **Deploy site** from GitHub Actions and choose production or preview.
Like Hypercmd, deployment is manual: pushes run checks and Vercel Git deployments
are disabled. Rust builds run in GitHub Actions; Vercel receives prebuilt files.
`vercel.json` redirects `/docs` to `/docs/`, serves documentation deep links,
and caches versioned Fusor assets. The deploy recipe removes Fusor build
manifests containing local paths from the upload.

For a local deployment, run the setup above, install the Vercel CLI version
pinned in `../.github/workflows/deploy-site.yml`, then authenticate and link
this directory to the sql-bomb project:

```sh
vercel login
vercel link
vercel pull --yes --environment=production
just deploy production
```

For a preview, use `preview` in both commands. Vercel's
[prebuilt deployment documentation](https://vercel.com/docs/cli/deploy#prebuilt)
describes the build/upload flow. Project credentials and generated output stay
outside version control.
