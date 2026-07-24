const MAX_ADDITIONAL_FILES = 15;

const SYSTEM_PROMPT = `The current year is ${new Date().getFullYear()}. Newer versions of software, GitHub Actions, and model names than you're aware of from training may still be valid and correct. Do not flag version strings, action references (e.g. actions/checkout@vN), or model names as errors solely because they are unfamiliar to you.`;

const FILES_SCHEMA = {
  type: "object",
  properties: {
    files: { type: "array", items: { type: "string" } },
  },
  required: ["files"],
  additionalProperties: false,
};

const REVIEW_SCHEMA = {
  type: "object",
  properties: {
    summary: { type: "string" },
    comments: {
      type: "array",
      items: {
        type: "object",
        properties: {
          path: { type: "string" },
          line: { type: "integer" },
          body: { type: "string" },
        },
        required: ["path", "line", "body"],
        additionalProperties: false,
      },
    },
  },
  required: ["summary", "comments"],
  additionalProperties: false,
};

async function callModel(core, endpoint, apiKey, model, prompt, schemaName, schema) {
  const url = `${endpoint.replace(/\/+$/, "")}/chat/completions`;
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      messages: [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content: prompt },
      ],
      response_format: {
        type: "json_schema",
        json_schema: { name: schemaName, schema, strict: true },
      },
    }),
  });

  const rawBody = await res.text();
  core.debug(`ai-review: raw response from model "${model}" (${schemaName}, status ${res.status}): ${rawBody}`);

  if (!res.ok) {
    throw new Error(`Model API error ${res.status}: ${rawBody}`);
  }

  const data = JSON.parse(rawBody);
  return JSON.parse(data.choices[0].message.content);
}

async function callModelWithFallback(core, endpoint, apiKey, model, fallbackModel, prompt, schemaName, schema) {
  try {
    return await callModel(core, endpoint, apiKey, model, prompt, schemaName, schema);
  } catch (err) {
    if (!fallbackModel || fallbackModel === model) {
      throw err;
    }
    core.warning(`ai-review: model "${model}" failed (${err.message}); retrying with fallback model "${fallbackModel}".`);
    return callModel(core, endpoint, apiKey, fallbackModel, prompt, schemaName, schema);
  }
}

module.exports = async ({ github, context, core }) => {
  const apiKey = process.env.API_KEY;
  const endpoint = process.env.API_ENDPOINT;
  const model = process.env.MODEL;
  const contextModel = process.env.CONTEXT_MODEL;
  const fallbackModel = process.env.FALLBACK_MODEL;
  const prNumber = parseInt(process.env.PR_NUMBER);
  const { owner, repo } = context.repo;

  if (!prNumber) {
    core.setFailed("ai-review: no pull request number found.");
    return;
  }

  const { data: pr } = await github.rest.pulls.get({ owner, repo, pull_number: prNumber });
  const { data: diff } = await github.rest.pulls.get({
    owner,
    repo,
    pull_number: prNumber,
    mediaType: { format: "diff" },
  });
  const description = pr.body || "(no description provided)";

  const { files: requestedFiles } = await callModelWithFallback(
    core,
    endpoint,
    apiKey,
    contextModel,
    fallbackModel,
    [
      `You are reviewing a GitHub pull request titled "${pr.title}".`,
      `PR description:\n${description}`,
      `PR diff:\n${diff}`,
      "List any additional repository files (besides those already in the diff) whose contents you need to give a thorough code review. Only request files that are directly relevant. If none are needed, return an empty list.",
    ].join("\n\n"),
    "requested_files",
    FILES_SCHEMA,
  );

  const additionalFiles = [];
  for (const path of (requestedFiles || []).slice(0, MAX_ADDITIONAL_FILES)) {
    try {
      const { data } = await github.rest.repos.getContent({ owner, repo, path, ref: pr.head.sha });
      if (typeof data.content === "string") {
        additionalFiles.push({ path, content: Buffer.from(data.content, "base64").toString("utf8") });
      }
    } catch {
      // requested file may not exist, be binary, or exceed the API size limit
    }
  }
  const additionalContext = additionalFiles.map((f) => `File: ${f.path}\n${f.content}`).join("\n\n");

  const { summary, comments } = await callModelWithFallback(
    core,
    endpoint,
    apiKey,
    model,
    fallbackModel,
    [
      `You are reviewing a GitHub pull request titled "${pr.title}".`,
      `PR description:\n${description}`,
      `PR diff:\n${diff}`,
      additionalContext && `Additional file contents for context:\n${additionalContext}`,
      "Review this PR. Provide a concise overall summary and, where relevant, specific comments referencing a file path and line number from the diff (the line number in the new version of the file). Only comment on lines actually changed in the diff.",
    ]
      .filter(Boolean)
      .join("\n\n"),
    "pr_review",
    REVIEW_SCHEMA,
  );

  const body = summary || "AI review";
  const inlineComments = (comments || []).map((c) => ({ path: c.path, line: c.line, body: c.body, side: "RIGHT" }));

  try {
    await github.rest.pulls.createReview({ owner, repo, pull_number: prNumber, body, event: "COMMENT", comments: inlineComments });
  } catch (err) {
    core.warning(`ai-review: failed to post review with inline comments (${err.message}); folding them into the review body instead.`);
    const foldedComments = inlineComments.map((c) => `**${c.path}:${c.line}**\n${c.body}`).join("\n\n");
    const foldedBody = foldedComments ? `${body}\n\n---\n\n${foldedComments}` : body;
    await github.rest.pulls.createReview({ owner, repo, pull_number: prNumber, body: foldedBody, event: "COMMENT" });
  }
};
