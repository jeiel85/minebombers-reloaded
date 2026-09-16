# 07. Deployment Guide

## Prerequisites

- GitHub repository;
- Node.js current LTS or compatible modern version;
- Cloudflare account;
- Wrangler CLI authenticated for the server project.

## A. Deploy backend first

From repository root:

```bash
npm install
npm run deploy:server
```

After deploy, note the Worker URL, e.g.:

```text
https://minebombers-api.<account>.workers.dev
```

Set allowed origin in Worker configuration to your final GitHub Pages origin.

## B. Configure client API endpoint

For Vite:

```text
VITE_API_BASE=https://minebombers-api.<account>.workers.dev
```

Do not place Cloudflare API tokens or other secrets in `VITE_*` variables; Vite embeds them in client JS.

## C. GitHub Pages

GitHub Pages serves the Vite build output. The supplied starter workflow uses GitHub Actions Pages deployment.

Repository Settings -> Pages should use **GitHub Actions** as the source.

Add a repository variable:

```text
VITE_API_BASE = https://minebombers-api.<account>.workers.dev
```

If using a project site such as:

```text
https://user.github.io/minebombers-web/
```

set Vite's `base` correctly. The starter uses a relative `./` base to avoid hard-coding a repository name; test asset URLs before release.

## D. Local development

Terminal 1:

```bash
npm run dev:server
```

Terminal 2:

```bash
npm run dev:client
```

The client may use:

```text
VITE_API_BASE=http://localhost:8787
```

When `GameSocket` receives `http://`, it converts to `ws://`; production `https://` converts to `wss://`.

## E. Protocol rollout

For incompatible protocol changes:

1. deploy server that accepts old + new version if feasible;
2. deploy client;
3. observe old client population;
4. remove old protocol after grace period.

For a small private project, a hard version gate is acceptable: server rejects old builds with a clear refresh message.

## F. GitHub Pages limitations relevant to this project

As of the verification date, GitHub documents Pages as static hosting and states that server-side languages such as PHP, Ruby, or Python are unsupported. GitHub also documents site/repository and soft bandwidth limits. Therefore all real-time authoritative code lives outside Pages.

Official references:

- https://docs.github.com/en/pages/getting-started-with-github-pages/creating-a-github-pages-site
- https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits
