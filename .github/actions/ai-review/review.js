const MAX_ADDITIONAL_FILES = 15;

const SYSTEM_PROMPT = `The current year is ${new Date().getFullYear()}. Newer versions of software, GitHub Actions, and model names than you're aware of from training may still be valid and correct. Do not flag version strings, action references (e.g. actions/checkout@vN), or model names as errors solely because they are unfamiliar to you.`;

const CONTEXT_INSTRUCTIONS = `You are preparing to perform a thorough code review. Your current task is to collect required context by identifying if any additional repository files are needed to understand the scope of the changes.
Analyze the diff and adhere strictly to the JSON schema. You must step through the schema properties sequentially to determine if external files, types, or references are used that require inspection.`;

const FILES_SCHEMA = {
  type: "object",
  properties: {
    touched_files: {
      type: "array",
      items: { type: "string" },
      description: "List every file that has been modified according to the diff."
    },
    missing_references: {
      type: "array",
      items: { type: "string" },
      description: "List any functions, structs, traits, or modules utilized in the diff that might be defined outside of it. Erring on the side of caution, include anything that seems external."
    },
    reasoning: {
      type: "string",
      description: "Based on the missing_references, deduce which ADDITIONAL repository files need to be requested. Do not discuss files that are already in touched_files."
    },
    files: {
      type: "array",
      items: { type: "string" },
      description: "The final, combined list of files. You MUST include EVERY file listed in 'touched_files', PLUS any new files identified in your 'reasoning'. Do not omit any touched files."
    }
  },
  required: ["touched_files", "missing_references", "reasoning", "files"],
};

const REVIEW_INSTRUCTIONS = `You are performing a strict code review on the provided PR diff.
Your goal is to provide a concise overall summary and identify actionable improvements.

Strict Rules for Comments:
1. ONLY comment on lines that were actually added or modified in the diff.
2. ONLY leave a comment where the code must be fixed, could be improved, or is questionable. Do not leave comments just to praise or acknowledge code.
3. Every comment must be highly actionable.
4. Line numbers must correspond exactly to the line number in the NEW version of the file.

Analyze the diff sequentially using the provided JSON schema to brainstorm issues before formatting your final comments.`;

const REVIEW_SCHEMA = {
  type: "object",
  properties: {
    summary: {
      type: "string",
      description: "A concise, high-level summary of the overall changes made in the PR."
    },
    code_opinions: {
      type: "array",
      items: { type: "string" },
      description: "Evaluate the code and list all comments that come to your mind. Nothing is out of scope, everything should be looked at."
    },
    issue_scratchpad: {
      type: "array",
      items: { type: "string" },
      description: "Base on code_opinons brainstorm any potential bugs, edge cases, or logic flaws specifically in the modified lines. Think step-by-step about what needs fixing."
    },
    refined_issues: {
      type: "array",
      items: { type: "string" },
      description: "Rate very issue listed in issue_scratchpad and filter pure comments or praise that don't require a code change. For the remaining issues provide as much detail as possible and make sure it is still relevant."
    },
    comments: {
      type: "array",
      items: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "The exact file path from the diff."
          },
          line: {
            type: "integer",
            description: "The exact line number in the NEW version of the file where the issue occurs."
          },
          body: {
            type: "string",
            description: "The actionable feedback based on the issue_scratchpad. It should be concise. Do not include praise."
          },
        },
        required: ["path", "line", "body"],
      },
      description: "The final formatted review comments. This must be empty if the issue_scratchpad is empty."
    },
    follow_ups: {
      type: "array",
      items: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description: "The exact repo file path that requires a follow up"
          },
          line: {
            type: "integer",
            description: "The exact line number in the file that requires a follow up"
          },
          question: {
            type: "string",
            description: "The follow up question that should be evaluated."
          },
        },
        required: ["path", "line", "question"],
      },
      description: "Based on code_opinions, issue_scratchpad and refined_issues list any items that could not be evaluated with confidence and require further follow ups for a thorough review."
      },
      research_links: {
        type: "array",
        items: { type: "string" },
        description: "Any URLs or documentation links retrieved via Google Search used to justify the code review comments. Return an empty array if no search was needed."
      }
  },
  required: ["summary", "code_opinions", "issue_scratchpad", "refined_issues", "comments", "follow_ups", "research_links"],
};

async function callOpenAi(core, endpoint, apiKey, model, prompt, schemaName, schema, tools) {
    if (tools !== undefined) core.warning("Tools not supported with open ai api!")
    const url = `${endpoint.replace(/\/+$/, "")}/chat/completions`;
    const body = {
        model,
        messages: [
            { role: "system", content: SYSTEM_PROMPT },
            { role: "user", content: prompt },
        ],
        response_format: {
            type: "json_schema",
            json_schema: { name: schemaName, schema, strict: true },
        }
    };
    const res = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${apiKey}`
      },
      body: JSON.stringify(body),
    });
    const rawBody = await res.text();
    core.debug(`ai-review: raw response from model "${model}" (${schemaName}, status ${res.status}): ${rawBody}`);

    const data = JSON.parse(rawBody);
    return JSON.parse(data.choices[0].message.content);
}

async function callGemini(core, endpoint, apiKey, model, prompt, schemaName, schema, tools) {
  const url = `${endpoint.replace(/\/+$/, "")}/chat/completions`;
  const body = {
    // 1. System Prompt moves to its own dedicated object
    system_instruction: {
      parts: [{ text: SYSTEM_PROMPT }]
    },
    // 2. 'messages' becomes 'contents' and 'parts'
    contents: [
      {
        role: "user",
        parts: [{ text: prompt }]
      }
    ],
    // 3. 'response_format' becomes 'generationConfig'
    generationConfig: {
      responseMimeType: "application/json",
      // Notice: The schema goes directly here, no 'type: "json_schema"' wrapper
      responseSchema: schema
    },
    // 4. Grounding tool is now natively recognized
    tools
  };
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-goog-api-key": apiKey
    },
    body: JSON.stringify(body),
  });
  const rawBody = await res.text();
  core.debug(`ai-review: raw response from model "${model}" (${schemaName}, status ${res.status}): ${rawBody}`);

  const data = JSON.parse(rawBody);
  return JSON.parse(data.candidates[0].content.parts[0].text);
}

async function callModel(core, endpoint, apiKey, model, prompt, schemaName, schema, tools) {
  if (endpoint === "https://generativelanguage.googleapis.com/v1beta/models/") {
      return callGemini(core, endpoint, apiKey, model, prompt, schemaName, schema, tools);
  } else {
      return callOpenAi(core, endpoint, apiKey, model, prompt, schemaName, schema, tools)
  }
}

async function callModelWithFallback(core, endpoint, apiKey, model, fallbackModel, prompt, schemaName, schema, tools) {
  try {
    return await callModel(core, endpoint, apiKey, model, prompt, schemaName, schema, tools);
  } catch (err) {
    if (!fallbackModel || fallbackModel === model) {
      throw err;
    }
    core.warning(`ai-review: model "${model}" failed (${err.message}); retrying with fallback model "${fallbackModel}".`);
    return callModel(core, endpoint, apiKey, fallbackModel, prompt, schemaName, schema, tools);
  }
}

module.exports = async ({ github, context, core }) => {
  const apiKey = process.env.API_KEY;
  const endpoint = process.env.API_ENDPOINT;
  const model = process.env.MODEL;
  const contextModel = process.env.CONTEXT_MODEL;
  const fallbackModel = process.env.FALLBACK_MODEL;
  const title = process.env.TITLE || "AI Code Review";
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

  const contextPrompt = [
    `You are reviewing a GitHub pull request titled "${pr.title}".`,
    `PR description:\n${description}`,
    `PR diff:\n${diff}`,
    CONTEXT_INSTRUCTIONS,
  ].join("\n\n")
  const { files: requestedFiles } = await callModelWithFallback(
    core,
    endpoint,
    apiKey,
    contextModel,
    fallbackModel,
    contextPrompt,
    "requested_files",
    FILES_SCHEMA,
  );

  core.debug({requestedFiles})
  const additionalFiles = [];
  for (const path of (requestedFiles || []).slice(0, MAX_ADDITIONAL_FILES)) {
    try {
      const { data } = await github.rest.repos.getContent({ owner, repo, path, ref: pr.head.sha });
      if (typeof data.content === "string") {
        core.debug(`Added ${path} to context`)
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
      REVIEW_INSTRUCTIONS,
    ]
      .filter(Boolean)
      .join("\n\n"),
    "pr_review",
    REVIEW_SCHEMA,
    [
      { google_search: {} }
    ]
  );

  const body = `## ${title}\n\n${summary || "AI review"}`;
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
