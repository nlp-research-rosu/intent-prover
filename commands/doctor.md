---
description: Show Intent Prover connection and configuration diagnostics
allowed-tools: Bash(intent-prover doctor:*), Bash(true)
---

!`intent-prover doctor --data "${CLAUDE_PLUGIN_DATA}" --plugin-root "${CLAUDE_PLUGIN_ROOT}" 2>&1 || true`

Tell the user the lines above, word for word, and nothing else.
