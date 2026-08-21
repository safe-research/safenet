#!/usr/bin/env node
// Local harness for review.js — hits the real GitHub API for PR data but
// stubs createReview to print instead of posting, so prompt edits can be
// iterated on without deploying or leaving comments on the PR.
//
// Usage (public repos, no token needed — but you'll hit GitHub's 60 req/hr
// unauthenticated rate limit fast; set GITHUB_TOKEN to get 5000 req/hr):
//   REPO=owner/repo PR_NUMBER=123 \
//   API_KEY=sk-xxx API_ENDPOINT=https://generativelanguage.googleapis.com/v1beta/openai/ \
//   MODEL=gemini-3.6-flash CONTEXT_MODEL=gemini-3.5-flash-lite \
//   node .github/actions/ai-review/local-test.js

const review = require("./review.js");

const token = process.env.GITHUB_TOKEN;
const [owner, repo] = (process.env.REPO || "").split("/");

if (!owner || !repo || !process.env.PR_NUMBER) {
  console.error("Usage: REPO=owner/repo PR_NUMBER=123 API_KEY=... node local-test.js");
  process.exit(1);
}

async function ghRequest(path, { headers, ...opts } = {}) {
  const res = await fetch(`https://api.github.com${path}`, {
    ...opts,
    headers: {
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      Accept: "application/vnd.github+json",
      "User-Agent": "ai-review-local-test",
      ...headers,
    },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status} ${path}: ${await res.text()}`);
  return res;
}

const github = {
  rest: {
    pulls: {
      get: async ({ owner, repo, pull_number, mediaType }) => {
        const isDiff = mediaType?.format === "diff";
        const res = await ghRequest(`/repos/${owner}/${repo}/pulls/${pull_number}`, {
          headers: isDiff ? { Accept: "application/vnd.github.v3.diff" } : {},
        });
        return { data: isDiff ? await res.text() : await res.json() };
      },
      createReview: async (params) => {
        console.log("\n===== DRY RUN: would post this review =====");
        console.log(`event: ${params.event}`);
        console.log(`body:\n${params.body}`);
        for (const c of params.comments || []) {
          console.log(`\n-- ${c.path}:${c.line} --\n${c.body}`);
        }
        console.log("=============================================\n");
        return { data: {} };
      },
    },
    repos: {
      getContent: async ({ owner, repo, path, ref }) => {
        const res = await ghRequest(
          `/repos/${owner}/${repo}/contents/${path.split("/").map(encodeURIComponent).join("/")}?ref=${ref}`,
        );
        return { data: await res.json() };
      },
    },
  },
};

const context = { repo: { owner, repo } };

const core = {
  debug: (msg) => console.debug(msg),
  warning: (msg) => console.warn(msg),
  setFailed: (msg) => {
    console.error("[failed]", msg);
    process.exitCode = 1;
  },
};

review({ github, context, core }).catch((err) => {
  console.error(err);
  process.exit(1);
});
