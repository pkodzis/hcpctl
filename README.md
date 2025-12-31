# hcpctl

A CLI for HCP Terraform (formerly Terraform Cloud/Enterprise).

```bash
hcpctl get ws                    # List all workspaces
hcpctl get ws my-workspace       # Get specific workspace
hcpctl get prj -o yaml           # List projects as YAML
hcpctl get org                   # List organizations
```

## Documentation

```bash
hcpctl --help           # General help
hcpctl get --help       # Get command help
hcpctl get ws --help    # Workspace-specific options
```

## Installation

### Linux / macOS

```bash
curl -fsSL \
  https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.sh | bash
```

Or with custom install directory:

```bash
INSTALL_DIR=/usr/local/bin \
  curl -fsSL \
  https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.sh | bash
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.ps1 | iex
```

### From Source

Requires [Rust](https://rustup.rs/):

```bash
git clone https://github.com/pkodzis/hcpctl.git
cd hcpctl
cargo install --path .
```

## Configuration

Set your HCP Terraform token:

```bash
export TFE_TOKEN="your-token-here"
```

Optionally set default host and organization:

```bash
export TFE_HOST="app.terraform.io"
export TFE_ORG="my-organization"
```

Or use Terraform CLI credentials file (`~/.terraform.d/credentials.tfrc.json`).

Run `hcpctl get --help` for full credential resolution details.

## License

MIT
