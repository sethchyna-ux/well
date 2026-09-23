# AI safety, privacy, and credentials

Pythia is Well's optional AI command and image assistant. Its output is untrusted input: review every suggested command before using it.

## When AI is enabled

Cloud AI is opt-in. Well uses Gemini or Hugging Face only after the user selects that provider and supplies its credential. Selecting **Offline Rules** makes no AI network request. Selecting Ollama sends requests only to the configured Ollama endpoint; the default is local.

If a selected provider is unavailable, rejects a request, or has no usable credential, command translation can fall back to local rules. Image generation instead returns a clear failure: Well never presents a locally generated test pattern as provider-created artwork. `WELL_MOCK_LLM=1` is reserved for tests and visibly labels its mock PNG output.

Well does **not** automatically send terminal screen contents, scrollback, command history, environment variables, files, or file contents to any AI provider. Such data leaves the machine only if the user explicitly includes it in a prompt. The request context listed below is added automatically.

## Provider data flow

### Google Gemini

Gemini requests leave the machine and go to Google's Gemini API.

For command translation, Well sends:

- the user's prompt;
- the configured shell name or path;
- the current working-directory path; and
- the operating-system name used in the instruction.

For diagnostics, Well sends:

- the failed command;
- its exit code; and
- its captured standard-error text.

Diagnostic requests do not add terminal screen contents, history, the current shell, or the current working directory. However, stderr and the failed command can themselves contain sensitive values or paths; review them before requesting a diagnosis.

For image generation, Well sends:

- the image prompt; and
- the requested width and height, translated into the API's supported aspect-ratio and image-size settings.

The Gemini API key is transmitted to Google for authentication. It is not part of the model prompt.

### Hugging Face

Hugging Face requests leave the machine and go to Hugging Face's inference services.

For command translation, Well sends:

- the user's prompt;
- the configured shell name or path;
- the current working-directory path; and
- the operating-system name used in the instruction.

For image generation, Well sends the image prompt and requested dimensions to Hugging Face's Inference Providers image endpoint. Well uses the dedicated `stabilityai/stable-diffusion-3-medium-diffusers` image model rather than the configured text-translation model.

The Hugging Face token is transmitted to Hugging Face as an authorization header. It is not part of the model prompt. Well does not send diagnostics to Hugging Face.

### Ollama

Well first checks the configured endpoint's `/api/tags` route, then sends command-translation requests to its `/api/generate` route. The generation request contains:

- the user's prompt;
- the configured model name;
- the configured shell name or path;
- the current working-directory path; and
- the operating-system name used in the instruction.

The default endpoint is:

```text
http://localhost:11434
```

The default keeps request data on the machine, subject to the local Ollama installation. If the endpoint is changed to a LAN or internet address, the same prompt and context are sent to that host without a Well-managed application credential. Well does not use Ollama for diagnostics or image generation; image requests report that a supported image provider must be selected.

### Offline mode

Offline Rules performs command matching and diagnostic heuristics in process and makes no AI network request. It does not claim to generate AI images. Provider-created image artifacts are cached under `~/.well/generated/`; deterministic PNGs are available only to automated tests with `WELL_MOCK_LLM=1` and are labeled as mock output.

## Credential storage design

Phase 6 moves AI credentials to the operating-system credential store. On macOS this is Keychain, using:

| Purpose | Service | Account |
| --- | --- | --- |
| Gemini API key | `org.well.terminal.ai` | `gemini-api-key` |
| Hugging Face token | `org.well.terminal.ai` | `hugging-face-token` |

Credential resolution follows this precedence:

1. `GEMINI_API_KEY` or `HF_TOKEN` from the process environment;
2. the corresponding operating-system credential-store entry;
3. a legacy plaintext config value, only for migration compatibility; then
4. no cloud credential, causing cloud image generation to remain unavailable and command translation to use its documented local fallback when applicable.

Environment variables intentionally override Keychain so CI, development shells, and managed launch environments can supply temporary credentials without changing stored values. The settings UI can save or remove each credential-store entry. Removing an entry does not unset an environment variable, so an environment-provided credential can remain active.

Normal config saves, named profiles, and Hyprlang import/export never persist credentials. Older JSON configs containing plaintext `gemini_api_key` or `hf_token` fields may still load for migration compatibility. After loading one, use **Save to Keychain** for each credential, then save the Well configuration to rewrite it without plaintext secrets. Securely remove any remaining legacy copies and backups.

Never commit credentials, `.env` files, exported keychains, plaintext legacy configs, or screenshots containing secrets.

## Direct-execution safety gate

Every generated command is treated as untrusted. Immediately before **Run in Shell** injects a command, Well scans the exact command that will be executed; it does not rely only on a provider's safety label or an earlier scan. If that command is classified as potentially destructive, direct execution remains disabled until the user types exactly:

```text
RUN
```

Leading and trailing whitespace is ignored, but the token is case-sensitive and no additional text is accepted. A changed or newly generated command is revalidated and requires its own confirmation.

The detector examines each executable segment of compound commands and unwraps common launchers such as `sudo`, `doas`, `env`, `command`, `builtin`, and `nohup`. It also inspects explicit shell payloads, `eval` payloads, commands launched through `xargs`, and command substitutions using `$()` or backticks. The guarded categories include file deletion, destructive `find` actions, raw-disk and filesystem formatting, broad recursive permission changes, data-discarding Git operations, container or infrastructure deletion, process termination, shutdown, and fork bombs.

**Insert into Prompt** and **Copy Command** do not execute anything, so they intentionally bypass the typed `RUN` gate. After insertion or copying, the command is outside the direct-execution path: the user can edit it, paste it elsewhere, or run it manually, and Well cannot enforce the gate there.

## Limitations

- The command detector is a conservative safety aid, not a shell parser, sandbox, policy engine, or authorization boundary. False positives and false negatives are possible.
- Dynamic or obscured behavior can evade static inspection, including aliases, functions, scripts, encoded payloads, indirect variable expansion, unusual quoting, provider-specific tools, and commands whose effects depend on external state.
- Revalidation covers the exact text Well injects through **Run in Shell**. It cannot guarantee what aliases, functions, executables, shell startup hooks, or remote systems will do with that text.
- Inserted, copied, pasted, edited, or independently typed commands are the user's responsibility and may execute without Well's direct-execution gate.
- Provider transport, logging, training, abuse monitoring, and retention are controlled by the selected provider and the user's provider account or deployment policy.
- Shell and working-directory context can reveal usernames, project names, or sensitive paths. Use Offline Rules or a trusted local Ollama endpoint when that context must not leave the machine.
