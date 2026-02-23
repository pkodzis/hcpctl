# Managing Workspaces and Projects

Workspaces are the core resource in HCP Terraform. `hcpctl` provides powerful tools to list, filter, and modify workspaces and their associated projects.

## Listing and Filtering Workspaces

You can list all workspaces in an organization:

```bash
hcpctl get ws --org my-org
```

### Filtering

Use the `-f` or `--filter` flag to search for workspaces by name. This performs a partial match:

```bash
hcpctl get ws -f "network" --org my-org
```

### Sorting

You can sort the output using the `-s` or `--sort` flag. For workspaces, you can sort by `name`, `created-at`, or `updated-at`. Use `-r` to reverse the sort order.

```bash
hcpctl get ws --sort updated-at -r --org my-org
```

## Workspace Subresources

You can fetch specific subresources of a workspace using the `--subresource` flag. This is useful for getting the current state version, current run, or configuration version.

```bash
hcpctl get ws my-workspace --org my-org --subresource current-state-version
```

## Downloading Configuration

If you need to inspect the actual Terraform code that is currently loaded into a workspace (the Configuration Version), you can download it directly:

```bash
hcpctl download config my-workspace --org my-org
```

This will download the `.tar.gz` archive containing the Terraform configuration files that were uploaded for the current run.

## Modifying Workspaces

The `set` command allows you to modify workspace properties.

### Moving a Workspace to a Project

To move a workspace to a different project:

```bash
hcpctl set ws my-workspace --project "Core Infrastructure" --org my-org
```

## Managing Tags

Tags in HCP Terraform are managed as separate bindings. You can get, set, and delete tags on workspaces and projects.

### Viewing Tags

```bash
hcpctl get tag ws my-workspace --org my-org
```

### Adding Tags

You can add multiple tags at once:

```bash
hcpctl set tag ws my-workspace --tags env:prod,team:platform --org my-org
```

### Removing Tags

```bash
hcpctl delete tag ws my-workspace --tags env:prod --org my-org
```

## Purging State (Danger Zone)

If you need to completely reset a workspace's state (making Terraform "forget" all resources without destroying them in the cloud provider), you can use the `purge state` command.

This requires the exact Workspace ID (`ws-...`) to prevent accidental deletion.

```bash
hcpctl purge state ws-1234567890abcdef
```

*Note: To bypass the interactive confirmation prompt, you must use the `--my-resume-is-updated` flag instead of the standard `--batch` flag.*
