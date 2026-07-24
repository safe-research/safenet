# Workflows

## Gemini Code Review (`gemini_review.yml`)

Runs the [`ai-review`](../actions/ai-review) action against Google's Gemini
OpenAI-compatible endpoint. Triggers automatically when a PR is opened or
reopened, and can also be run manually via `workflow_dispatch` with a
`pr-number` input.

### Setup

Add a repository secret:

- `GEMINI_API_KEY` — an API key for the Gemini API, from
  [Google AI Studio](https://aistudio.google.com/app/apikey).

No other configuration is required; the action defaults to a Gemini model
and endpoint.

## Tailnet AI Code Review (`tailnet_ai_review.yml`)

Runs the same [`ai-review`](../actions/ai-review) action, but against a
self-hosted OpenAI-compatible server reachable only inside our Tailscale
tailnet. The runner joins the tailnet via
[`tailscale/github-action`](https://github.com/tailscale/github-action)
before the review step runs. Unlike the Gemini review, it currently (for
now) also re-runs on every new push to a PR, not just on open/reopen — it
triggers automatically when a PR is opened, or reopened, and
can also be run manually via `workflow_dispatch` with a `pr-number` input.
Since it depends on a private server that may not always be reachable, keep
an eye on this workflow failing if the tailnet server is down.

### Setup

1. **Create a Tailscale trust credential** in the
   [Tailscale admin console](https://login.tailscale.com/admin/settings/trust-credentials)
   configured for [workload identity federation](https://tailscale.com/kb/1581/workload-identity-federation)
   (OIDC) rather than a client secret, and grant it a tag (e.g. `tag:ci`)
   that your tailnet ACL policy allows the credential to issue. The runner's
   ephemeral node will be tagged with this, so make sure your ACLs grant it
   access to the AI server. This requires the workflow to request the
   `id-token: write` permission (already set in `tailnet_ai_review.yml`),
   which is how GitHub Actions issues the OIDC token the credential trusts —
   no long-lived secret is stored for authentication.
   - Set the OIDC scope to `repo:safe-research/safenet:*`.
   - Grant the credential write access to the `Auth Keys` permission,
     scoped to the tag `tag:ci`.
2. Add the following repository secrets. Everything here besides the model
   names is a secret rather than a repository variable — the tailnet
   hostname/IP and endpoint URL are treated as sensitive, since they reveal
   internal network layout, so they must not be exposed as plain variables:
   - `TS_OAUTH_CLIENT_ID` — the client ID of the trust credential created
     above.
   - `TS_AUDIENCE` — the audience configured on that trust credential for
     OIDC federation.
   - `TAILNET_AI_API_KEY` — the API key expected by your self-hosted
     OpenAI-compatible server (use any placeholder value if the server
     doesn't check one).
   - `TAILNET_AI_ENDPOINT` — the base URL of the server, e.g.
     `http://your-host.your-tailnet.ts.net:port/v1`. Do **not** include a
     trailing `/chat/completions`; the action appends that itself.
   - `TAILNET_AI_HOST` — the bare hostname or IP of the AI server (e.g.
     `your-host.your-tailnet.ts.net` or `100.x.y.z`, no scheme/port/path).
     Used by the `tailscale/github-action` step's `ping` input to confirm
     the runner can actually reach the server over the tailnet before the
     review step runs, since newly joined nodes can take a moment to
     propagate to peers.
3. Add the following repository (or environment) variables — these are just
   model names, not sensitive, so they don't need to be secrets:
   - `TAILNET_AI_MODEL` — the model name served by your endpoint.
   - `TAILNET_AI_CONTEXT_MODEL` — model used to decide whether extra context
     files are needed (can be the same as `TAILNET_AI_MODEL`).
   - `TAILNET_AI_FALLBACK_MODEL` — optional; leave unset to disable
     fallback-on-error retries.
4. If your tailnet ACLs restrict which tags can reach the AI server, make
   sure `tag:ci` (or whatever tag you used in step 1) is permitted — update
   the `tags:` input in `tailnet_ai_review.yml` to match if you used a
   different tag.
