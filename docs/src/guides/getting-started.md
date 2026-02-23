# Getting Started with hcpctl

`hcpctl` is a command-line interface for HCP Terraform (formerly Terraform Cloud) and Terraform Enterprise, designed to feel familiar to users of `kubectl`.

## Installation

You can install `hcpctl` using the provided installation scripts:

**Linux / macOS:**
```bash
curl -sSL https://hcpctl.com/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://hcpctl.com/install.ps1 | iex
```

## First Steps

Before you can use `hcpctl`, you need to authenticate with HCP Terraform.

The recommended way is to use the standard Terraform CLI authentication. If you have `terraform` installed, simply run:

```bash
terraform login
```

This will open your browser, generate an API token, and save it to `~/.terraform.d/credentials.tfrc.json`. `hcpctl` will automatically read this file and use the token.

Alternatively, you can set the `HCP_TOKEN` environment variable directly:

```bash
export HCP_TOKEN="your-terraform-cloud-token"
```

*Tip: You can generate a token manually in the HCP Terraform UI under User Settings -> Tokens.*

## Basic Usage

The core command pattern is `hcpctl <verb> <resource> [name] [flags]`.

### Listing Resources

To list all organizations you have access to:
```bash
hcpctl get org
```

To list workspaces in a specific organization:
```bash
hcpctl get ws --org my-organization
```

### Getting Specific Resources

To get details about a specific workspace:
```bash
hcpctl get ws my-workspace --org my-organization
```

### Output Formats

Like `kubectl`, you can change the output format using the `-o` or `--output` flag. Supported formats are `table` (default), `json`, `yaml`, and `csv`.

```bash
hcpctl get ws my-workspace --org my-organization -o yaml
```

## Next Steps

- Learn more about [Authentication and Contexts](authentication.md) to manage multiple environments.
- Explore [Workspace Management](workspaces.md) to see how to organize your infrastructure.
- See [Runs and Logs](runs-and-logs.md) to learn how to monitor Terraform executions.
