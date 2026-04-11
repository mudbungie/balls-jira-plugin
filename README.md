# balls-plugin-jira

Jira integration plugin for [balls](https://github.com/mudbungie/balls), the git-native task tracker.

Provides bidirectional sync between balls tasks and Jira issues, supporting both Jira Cloud and Jira Server/Data Center.

## Auth Methods

- **PAT** — Personal Access Token. Works with both Cloud and Server.
- **OAuth 2.0** — Browser-based OAuth with PKCE. Cloud only.
- **device auth** — SAML device helper. Device-cert SAML flow for Jira instances behind LocalAuth SSO.

## Install

```bash
make install   # builds and copies to ~/.local/bin/
```

## Setup

### 1. Configure the plugin in your balls project

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

Create `.balls/plugins/jira.json`:

```json
{
  "url": "https://company.atlassian.net",
  "project": "PROJ",
  "auth_method": "pat",
  "server_type": "cloud",
  "status_map": {
    "open": "To Do",
    "in_progress": "In Progress",
    "review": "In Review",
    "blocked": "Blocked",
    "closed": "Done",
    "deferred": "Backlog"
  },
  "sync_filter": "project = PROJ AND status != Done",
  "create_in_remote": true,
  "close_in_remote": true
}
```

### 3. Authenticate

```bash
balls-plugin-jira auth-setup --auth-dir .balls/local/plugins/jira/
```

## Config Reference

| Field | Default | Description |
|-------|---------|-------------|
| `url` | (required) | Jira base URL |
| `project` | (required) | Jira project key |
| `auth_method` | `pat` | `pat`, `oauth`, or `device_auth` |
| `server_type` | `cloud` | `cloud` or `server` |
| `status_map` | (sensible defaults) | balls status -> Jira status name |
| `field_map` | (sensible defaults) | balls field -> Jira field name |
| `sync_filter` | `project = {key} AND status != Done` | JQL for sync |
| `create_in_remote` | `true` | Create Jira issues on `bl create` |
| `close_in_remote` | `true` | Transition Jira issues on `bl close` |
| `oauth_client_id` | — | Required for OAuth auth method |
| `device_auth_helper_path` | (auto-detected) | Path to device auth helper binary |

## Development

```bash
make test      # run tests
make check     # clippy + line lengths + coverage
```
