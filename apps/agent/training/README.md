# Optional QLoRA Training

Run this stage after collecting at least 200 positively rated sessions. The
exporter groups sessions by task identity and performs the 80/10/10 split before
turning traces into model inputs.

## Prepare data

From \`apps/agent\`:

\`\`\`sh
npm start -- export-data --output training/data
\`\`\`

Inspect \`training/data/export-report.json\` and sample records from all three
splits. Records use MLX-LM's tool-use format with messages, tool calls, tool
results, and JSON tool definitions.

## Create the environment and train

\`\`\`sh
cd training
uv sync --locked
uv run mlx_lm.lora --config config.yaml
\`\`\`

The fixed seed, validation split, prompt masking, and saved checkpoints make the
run reproducible and reveal overfitting before the adapter is promoted.

## Evaluate and export

\`\`\`sh
uv run mlx_lm.lora \
  --model mlx-community/Qwen3-4B-Instruct-2507-4bit \
  --adapter-path adapters \
  --data data \
  --test

uv run mlx_lm.fuse \
  --model mlx-community/Qwen3-4B-Instruct-2507-4bit \
  --adapter-path adapters \
  --export-gguf

lms import fused_model/ggml-model-f16.gguf
\`\`\`

Run the same 30-task suite against the base and imported models. Promote the
adapter only when held-out task success improves by at least ten percentage
points, valid tool calls do not regress, and all workspace-boundary checks pass.
