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
Invoke-RestMethod `
  https://raw.githubusercontent.com/pkodzis/hcpctl/main/scripts/install.ps1 `
  | Invoke-Expression
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

## Development Environment

### Prerequisites

#### Linux / macOS (Development)

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version

# Install development tools
cargo install cross        # Cross-compilation support
cargo install cargo-edit   # Adds `cargo add`, `cargo upgrade` commands
```

#### Windows

```powershell
# Install Rust toolchain from https://rustup.rs
# Download and run rustup-init.exe

# Verify installation (in new terminal)
rustc --version
cargo --version

# Install development tools
cargo install cross
cargo install cargo-edit
```

### Build & Test

```bash
# Clone the repository
git clone https://github.com/pkodzis/hcpctl.git
cd hcpctl

# Install pre-commit hooks
pip install pre-commit
pre-commit install
pre-commit install --hook-type pre-push

# Build
cargo build

# Run tests
cargo test
```

## License

[MIT](LICENSE)
