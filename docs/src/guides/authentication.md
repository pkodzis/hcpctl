# Authentication and Contexts

`hcpctl` needs to know which HCP Terraform / Terraform Enterprise host to connect to, and what API token to use. It supports multiple ways to provide these credentials.

## Resolution Order

When you run a command, `hcpctl` resolves the **Host** and **Token** in the following order (first match wins):

### Host Resolution
1. CLI argument (`-H` or `--host`)
2. Environment variable (`TFE_HOSTNAME`)
3. Active Context
4. Terraform Credentials file (`~/.terraform.d/credentials.tfrc.json`)

### Token Resolution
1. CLI argument (`-t` or `--token`)
2. Environment variables (`HCP_TOKEN`, `TFC_TOKEN`, `TFE_TOKEN`)
3. Active Context
4. Terraform Credentials file (`~/.terraform.d/credentials.tfrc.json`)

## Using Contexts

If you work with multiple organizations or multiple Terraform Enterprise instances, **Contexts** are the best way to manage your configuration. They work exactly like `kubectl` contexts.

### Creating a Context

You can create a context that stores your host, token, and default organization:

```bash
hcpctl config set-context prod \
  --host app.terraform.io \
  --token $HCP_TOKEN \
  --org my-production-org
```

### Switching Contexts

To use a context, set it as the current context:

```bash
hcpctl config use-context prod
```

Now, any command you run will automatically use the host, token, and organization from the `prod` context. You no longer need to pass `--org my-production-org` to every command!

### Managing Contexts

List all configured contexts:
```bash
hcpctl config get-contexts
```

Show the currently active context:
```bash
hcpctl config current-context
```

Delete a context:
```bash
hcpctl config delete-context prod
```

## Terraform Credentials File

If you already use the standard `terraform` CLI, you likely have a credentials file at `~/.terraform.d/credentials.tfrc.json` (or `%APPDATA%\terraform.d\credentials.tfrc.json` on Windows).

`hcpctl` automatically reads this file. If you only have one host configured in it, `hcpctl` will use it automatically. If you have multiple hosts, `hcpctl` will prompt you to select one interactively.
