# balls-plugin-jira

Jira integration plugin for [balls](https://github.com/mudbungie/balls), the git-native task tracker.

Bidirectional sync between balls tasks and Jira issues. Works with both Jira Cloud (REST API v3) and Jira Server / Data Center (REST API v2). The plugin auto-discovers server type, statuses, priorities, and issue types from the live Jira instance every time it runs — the only required config is `url` and `project`.

## Auth Methods

- **PAT** — API token or Personal Access Token. Works with both Cloud and Server.
- **OAuth 2.0** — Browser-based 3LO with PKCE. Cloud only. Requires an Atlassian OAuth app (set `oauth_client_id`).
- **Device Auth** — SAML authentication via a local helper binary using native-messaging framing. For environments where your organization provides a device-certificate signing tool and you authenticate through a SAML IdP.

## Install

```bash
make install   # builds in release mode and copies to ~/.local/bin/
```

## Setup

### 1. Enable the plugin in your balls project

Add to `.balls/config.json`:

```json
{
  "plugins": {
    "jira": {
      "enabled": true,
      "sync_on_change": true,
      "config_file": ".balls/plugins/jira.json"
    }
  }
}
```

### 2. Create plugin config

Minimal `.balls/plugins/jira.json`:

```json
{
  "url": "https://company.atlassian.net",
  "project": "PROJ"
}
```

Everything else is discovered at runtime. A more complete config with overrides:

```json
{
  "url": "https://company.atlassian.net",
  "project": "PROJ",
  "auth_method": "pat",
  "status_map": {
    "review": "Code Review"
  },
  "sync_filter": "project = PROJ AND assignee = currentUser()",
  "create_in_remote": true,
  "close_in_remote": true
}
```

### 3. Authenticate

```bash
balls-plugin-jira auth-setup --auth-dir .balls/local/plugins/jira/
```

This walks you through the auth method of your choice. Credentials are written with `0600` permissions to the auth directory (which is itself `0700`).

## Config Reference

| Field | Default | Description |
|-------|---------|-------------|
| `url` | **required** | Jira base URL |
| `project` | **required** | Jira project key |
| `auth_method` | `pat` | `pat`, `oauth`, or `device_auth` |
| `server_type` | auto-detect | `cloud` or `server`. If omitted, detected via `GET /rest/api/2/serverInfo` |
| `status_map` | auto-discover | balls status -> Jira status name override. Discovery uses Jira's `statusCategory` to build this automatically |
| `sync_filter` | `project = {key} AND status != Done` | JQL used for pulling remote issues |
| `create_in_remote` | `true` | Create a Jira issue when `bl create` runs locally |
| `close_in_remote` | `true` | Transition the Jira issue when `bl close` runs locally |
| `oauth_client_id` | — | **Required** if `auth_method = "oauth"`. The client ID of your registered Atlassian OAuth 2.0 (3LO) app |
| `oauth_callback_port` | `19472` | Local port for the OAuth callback listener |
| `device_auth_helper_path` | — | **Required** if `auth_method = "device_auth"`. Absolute path to the helper binary |

### Status mapping

When syncing, the plugin picks the first Jira status it finds for each balls status according to this precedence:

1. An explicit entry in `status_map` config
2. A Jira status whose `statusCategory` matches (`new`→open, `indeterminate`→in_progress, `done`→closed)
3. A hardcoded fallback (`open`→"To Do", `closed`→"Done", etc.)

Priority mapping is positional: Jira's priority list is assumed to be ordered highest-to-lowest, so balls priority `1` maps to the first discovered priority, `4` maps to the fourth (or the last if fewer exist).

### Device Auth

`device_auth` supports environments where a local helper binary handles device-certificate-based SAML authentication. The plugin performs a five-step flow:

1. GET the Jira login page and parse the HTML for the IdP endpoint, `SAMLRequest`, and `RelayState`.
2. Call the helper binary over stdin/stdout using the native-messaging framing (4-byte little-endian length prefix, then a JSON payload). The plugin sends a single `getAuthData` request and reads a single response.
3. POST the SAML request to the IdP with the helper-provided `deviceToken`/`signature` in the `Authorization: Signature` header.
4. Follow the IdP redirect and parse the SAML response form.
5. POST the SAML response to the service consumer URL to obtain session cookies.

The helper must reply with a JSON object containing `deviceToken`, `signature`, and `sessionCookie` (newline-delimited `key=value` pairs). Set `device_auth_helper_path` to the absolute path of your org's helper binary.

## Plugin Protocol

When invoked by the balls core, the plugin reads task data on stdin and writes JSON responses on stdout. Command-line surface:

```
balls-plugin-jira auth-setup  --auth-dir DIR
balls-plugin-jira auth-check  --auth-dir DIR
balls-plugin-jira push        --task ID --config PATH --auth-dir DIR
balls-plugin-jira sync       [--task ID] --config PATH --auth-dir DIR
```

- `push` reads a single Task JSON object on stdin; emits a `PushResponse` JSON object on stdout.
- `sync` reads an array of Task JSON objects on stdin; emits a `SyncReport` JSON object on stdout.

## Development

```bash
make test      # run the test suite
make check     # test + clippy (deny warnings) + line-length check + coverage
```

`make check` requires `cargo-tarpaulin` for the coverage gate:

```bash
cargo install cargo-tarpaulin
```

Pre-commit hooks that block commits with clippy warnings or source files ≥ 300 lines can be installed via:

```bash
scripts/install-hooks.sh
```
