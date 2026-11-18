# Troubleshoot Intent Prover

Check the setup where the user is running the agent. A local install does not
make the plugin, skill, or CLI available to a remote agent.
If the problem is that the agent cannot find the plugin or skill, start with
[plugin or skill not visible](#plugin-or-skill-not-visible).

## Diagnose the CLI and connection

Run the command for the intended channel in the agent's own environment. It
works outside a repository, even when the plugin is not loaded.

| Channel | Command | Installer if the command is missing or outdated |
|---|---|---|
| Production | `intent-prover doctor` | Use `install-intent-prover.sh` or `install-intent-prover.ps1` from the [latest stable release](https://github.com/nlp-research-rosu/intent-prover/releases/latest). |
| Staging | `intent-prover-staging doctor` | Use `install-intent-prover-staging.sh` or `install-intent-prover-staging.ps1` from the intended `v…-staging.…` [prerelease](https://github.com/nlp-research-rosu/intent-prover/releases), not the stable installer. |

Use [onboarding's CLI detection steps](onboarding.md#detect-the-cli) with the
executable from the table. Follow the installer's `PATH` instructions and verify
the version from the agent's shell. Use the same executable for `config` and
`login`. Do not switch endpoints, credentials, or channels to make the check pass.

Use the report to choose a fix:

| Report | Next step |
|---|---|
| Invalid configuration or endpoint | Check the effective settings and endpoint overrides with the matching CLI's `config` command. Correct only the setting responsible; don't replace the configuration. |
| Service reachable; no usable key | Check for a pending sign-in and reuse it. If sign-in is needed, follow [onboarding](onboarding.md#connect-to-intent-prover) and have the user complete browser approval. |
| Rejected environment key | Correct or unset the named override in the agent's environment, then rerun `doctor`. Signing in will not override it. Never ask for the key in chat. |
| Rejected saved key | Reconnect through onboarding. |
| Saved credentials could not be read | Address the reported file problem first. On Unix, if the report says others can read the credentials file, set that file's mode to `600`. Do not read, print, or delete the file to diagnose it. |
| Account not approved | Give the user the application link. Reinstalling or repeating sign-in does not grant approval. |
| Too many failed authentication attempts | Wait as directed by `doctor`, then verify the credential source before trying again. Do not retry or start sign-in while throttled. |
| Client update required | Use the matching release installer above. Updating the plugin or skill does not update the executable. |
| Health or account request failed | Check the intended endpoint and the agent's network permissions. If a permitted check still fails, or health passes but account access remains unresolved, contact support. Do not keep retrying sign-in or reinstalling. |

Rerun `doctor` after a fix. `Account: key accepted` verifies the connection and
account access, not plugin visibility or the Scan recovery path. A Scan opt-out
warning is separate; leave the opt-out unchanged unless the user wants to
enable Scan. Do not submit a Scan or Prove job to verify setup.

## Plugin or skill not visible

Check whether the expected plugin is installed and enabled, or whether the
complete skill folder (including `references/`) is in the host's global
discovery path. Use the host's documented workflow for the installed channel:

- **Claude Code:** If the plugin is disabled, enable it in `/plugin` or with
  `claude plugin enable <installed-plugin-id>`. Run `/reload-plugins` or restart
  the session. For desktop or VS Code, restart the app or reload the window.
  If an update is needed, run `claude plugin marketplace update <marketplace>`
  and `claude plugin update <installed-plugin-id>`, then reload.
- **Codex:** Use its native marketplace and plugin controls to check or update
  the plugin, then restart the CLI session, app, or extension. Ask for the
  Intent Prover skill by name; Claude's slash commands do not apply.
- **Antigravity:** Start a new conversation and invoke `/intent:intent-prover`.
  If an update is needed, refresh the plugin using Antigravity's native install
  workflow and the same source channel.
- **Other agents:** Check the configured discovery path and use that host's
  refresh or restart procedure. To update a standalone skill, refresh the
  complete folder from the source used to install it.

If setup is actually missing, use the
[homepage installation prompt](https://prover.intentcomputing.org/#install).
Apply supported fixes within the user's request and ask the user to do any
required approval or restart you cannot perform. Verify that the agent can
invoke the skill and read its references, then rerun `doctor`.

## Contact support

If the problem persists, direct the user to
[Intent Computing support](https://t.me/IntentComputingSupport). Give them a
short summary to share: agent and platform, CLI version and release channel,
the symptom, the action already tried, and relevant sanitized diagnostic lines.
Omit API keys, login codes, private paths, and raw configuration or logs.
