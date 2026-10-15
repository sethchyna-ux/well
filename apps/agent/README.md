# Well Local Agent

A local coding and systems agent that uses LM Studio for inference. It confines
all file operations to one selected workspace, automatically permits read-only
tools, and asks before applying patches or running commands.

## Prerequisites

- Node.js 22 or newer
- LM Studio with its CLI installed
- The model \`qwen/qwen3-4b-2507\`

Install and load the model:

\`\`\`sh
lms get qwen/qwen3-4b-2507
lms load qwen/qwen3-4b-2507 \
  --context-length 8192 --gpu max --ttl 900
\`\`\`

Install the agent dependencies:

\`\`\`sh
npm install
npm run build
\`\`\`

## Commands

\`\`\`sh
npm start -- chat --workspace /absolute/path/to/project
npm start -- eval --suite evals/tasks.jsonl
npm start -- rate SESSION_ID good
npm start -- export-data --output training/data
\`\`\`

The default model is \`well-agent-base\`. Override it with \`--model\`. Runtime
configuration can be stored at
\`~/Library/Application Support/well-agent/config.json\` or supplied with
\`--config\`.

Session traces stay under the same application-support directory. The exporter
requires 200 positively rated sessions by default, splits sessions by task
identity before transformation, and writes MLX-LM tool-use JSONL files.

See [training/README.md](training/README.md) for the optional QLoRA workflow.
