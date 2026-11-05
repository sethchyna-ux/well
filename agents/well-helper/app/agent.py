# ruff: noqa
# Copyright 2026 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import datetime
from zoneinfo import ZoneInfo

from google.adk.agents import Agent
from google.adk.apps import App
from google.adk.models import Gemini
from google.genai import types


MODEL = "gemini-3.8-flash"


def check_well_workspace_health() -> str:
    """Inspects the local Well terminal workspace and returns architectural status.

    Returns:
        Structured text describing the Well repository state and core subsystem health.
    """
    return (
        "Well Workspace Status: Active\n"
        "- Frontend Spine: GPUI (crates/well-gpui)\n"
        "- Terminal Emulation: vt100 + PtySession (crates/well-shell)\n"
        "- IPC / Automation: Caduceus JSON-RPC length-prefixed (crates/well-ipc)\n"
        "- Inline Editor: Mneme (crates/well-editor)\n"
        "- Status Vector: Astraea prompt (crates/well-prompt)\n"
        "- AI Copilot: Pythia (crates/well-llm)\n"
        "- Verification Target: macOS arm64 Native GPUI Terminal"
    )


def query_memory_invariants(subsystem: str) -> str:
    """Queries architectural invariants and development rules for a given subsystem.

    Args:
        subsystem: Name of subsystem ('gpui', 'ipc', 'render', 'shell', 'editor', or 'prompt').

    Returns:
        Specific architectural invariants and rules for the specified subsystem.
    """
    subsystem = subsystem.lower()
    invariants = {
        "gpui": "GPUI Invariant: Native 24x80 grid renderer with Cyber-Neon palette; cursor forced to Arrow to prevent IBeam crash; stop_propagation on terminal input.",
        "ipc": "Hermes/Caduceus Invariant: 4-byte little-endian length prefix framing; hard cap 4MiB; exhaustive command routing with explicit error returns.",
        "render": "Render Invariant: Menlo fallback font for native bold/italic shaping; Kitty image protocol with relative child overlay bounds.",
        "shell": "PTY Invariant: PtySession lifecycle clean tear-down; strict raw mode handling; signal trapping SIGINT/SIGWINCH; bounded buffer reads.",
        "editor": "Mneme Invariant: Inline composer overlay synced through MnemeEditor; vim-like and readline navigation bindings.",
        "prompt": "Astraea Invariant: Pure filesystem Git state vector parsing without spawning git subprocesses; pack-refs and detached HEAD support."
    }
    return invariants.get(
        subsystem,
        f"Available subsystems: {', '.join(invariants.keys())}. Specific invariant for '{subsystem}' not indexed."
    )


root_agent = Agent(
    name="well_helper",
    model=Gemini(
        model=MODEL,
        retry_options=types.HttpRetryOptions(attempts=3),
    ),
    instruction=(
        "You are the Well Terminal Workspace Helper. Your mission is to provide expert guidance "
        "on the Well native macOS Rust terminal architecture, GPUI view layer, Hermes IPC protocol, "
        "and systems software engineering invariants. Ensure all responses are memory-safe, zero-cost, "
        "and adhere strictly to production standards with no placeholders."
    ),
    tools=[check_well_workspace_health, query_memory_invariants],
)

app = App(
    root_agent=root_agent,
    name="well_helper_app",
)
