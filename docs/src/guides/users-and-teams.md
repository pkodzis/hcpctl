# Managing Users and Teams

`hcpctl` provides administrative commands to manage users, teams, and invitations within your HCP Terraform organization. This is especially useful for onboarding new team members or auditing access.

## Listing Organization Members

To see all members in your organization:

```bash
hcpctl get org-member --org my-org
```

### Filtering Members

You can filter members by their email address or their current status (`active` or `invited`):

```bash
# Find a specific user
hcpctl get org-member -f "alice@example.com" --org my-org

# List all pending invitations
hcpctl get org-member --status invited --org my-org
```

## Managing Teams

To list all teams in your organization:

```bash
hcpctl get team --org my-org
```

You can also filter teams by name:

```bash
hcpctl get team -f "platform" --org my-org
```

## Inviting Users

You can invite a new user to your organization and optionally assign them to specific teams immediately.

```bash
hcpctl invite --email new.user@example.com --org my-org
```

To invite a user and add them to the "Developers" and "Platform" teams:

```bash
hcpctl invite --email new.user@example.com --teams "Developers,Platform" --org my-org
```

`--teams` accepts comma-separated team references (name or ID).

## Removing Users

To remove a user from the organization, you can use their email address or their membership ID (`ou-...`):

```bash
hcpctl delete org-member old.user@example.com --org my-org
```

Skip confirmation with `-y`/`--yes` (or global `--batch` mode):

```bash
hcpctl delete org-member old.user@example.com --org my-org --yes
```

## Viewing VCS Connections (OAuth Clients)

When setting up workspaces that connect to version control systems (like GitHub or GitLab), you often need the OAuth Client ID. You can list all configured VCS connections in your organization:

```bash
hcpctl get oc --org my-org
```

This will display the names, IDs (`oc-...`), and the service provider (e.g., `github`, `gitlab_hosted`) for each connection.

## Related guides

- [Authentication and Contexts](authentication.md)
- [Managing Workspaces and Projects](workspaces.md)
