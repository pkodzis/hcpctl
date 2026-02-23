# Contexts and Multi-Environment Workflows

If you work with multiple HCP Terraform or Terraform Enterprise environments (for example: `dev`, `stage`, `prod`), contexts are the safest way to avoid using the wrong host/org/token.

## Why and when to use

Use contexts when you:

- switch between multiple hosts,
- use different default organizations,
- want shorter commands without repeating `--host`, `--token`, `--org`.

## Common tasks

### 1. Create contexts

```bash
hcpctl config set-context dev --host app.terraform.io --org my-dev-org
hcpctl config set-context prod --host tfe.company.internal --org my-prod-org
```

You can include token directly:

```bash
hcpctl config set-context prod --token "$TFE_TOKEN"
```

### 2. Switch active context

```bash
hcpctl config use-context dev
hcpctl config current-context
```

### 3. List configured contexts

```bash
hcpctl config get-contexts
```

### 4. Use one-off context override

```bash
hcpctl --context prod get ws
```

## Advanced variants

### Environment-driven context selection

```bash
export HCPCTL_CONTEXT=prod
hcpctl get run --org my-prod-org
```

Active context name is resolved in this order:

1. `--context`
2. `HCPCTL_CONTEXT`
3. `current-context` in config

### Safe defaults + explicit overrides

Keep default org in context, but override for one command:

```bash
hcpctl --context prod get ws --org emergency-org
```

## Pitfalls and troubleshooting

### "Why did command use wrong host?"

Host resolution order is:

1. `--host`
2. `TFE_HOSTNAME`
3. active context host
4. Terraform credentials file

If behavior is unexpected, check env vars first:

```bash
env | grep -E 'HCPCTL_CONTEXT|TFE_HOSTNAME|HCP_TOKEN|TFC_TOKEN|TFE_TOKEN'
```

### Multiple hosts in credentials file + batch mode

In normal mode, hcpctl may ask you to choose host interactively.

In `--batch` mode, no prompt is shown. If multiple hosts are available and host is not explicitly set, command fails.

### Context missing after deletion

If you delete the current context, `current-context` becomes empty. Set a new one:

```bash
hcpctl config use-context dev
```

## Related guides

- [Authentication and Contexts](authentication.md)
- [Getting Started with hcpctl](getting-started.md)
- [Managing Workspaces and Projects](workspaces.md)
