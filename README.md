# Kagi FastGPT CLI

A command-line interface for [Kagi's FastGPT API](https://help.kagi.com/kagi/api/fastgpt.html) written in Rust.

![image](https://github.com/user-attachments/assets/9cc9b519-5e51-487c-bc33-4798d71fe22b)


## Features

- Conversation History
- Support for JSON output format
- Session management commands (`/clear`, `/history`, `/help`)

## Installation

### Using Cargo

```bash
cargo install fastgpt
```

### From Source

1. Clone this repository:
   ```bash
   git clone https://github.com/0xgingi/kagi-fastgpt-cli
   cd kagi-fastgpt-cli
   ```
2. Build and install:
   ```bash
   cargo install --path .
   ```

## Quick Start

1. **Get your API key** from [Kagi](https://kagi.com):
   - Navigate to Settings → Advanced → API portal
   - Click "Generate API Token"
   - Top off your API credits (1.5¢ per query)

2. **Set your API key** (one time setup):
   ```bash
   fastgpt --config
   ```

3. **Start chatting**:
   ```bash
   fastgpt
   ```

## Usage

### API Key Management

```bash
# Set API key
fastgpt --set-api-key "your-api-key"

# Show current API key
fastgpt --show-api-key

# Reset/remove stored API key
fastgpt --reset-api-key
```

### Options

```bash
fastgpt [OPTIONS]

Options:
      --set-api-key <SET_API_KEY>  Set API key (will be saved for future use)
      --show-api-key               Show current API key
      --config                     Interactive configuraiton setup
      --cache                      Whether to allow cached responses [default: true]
      --json                       Output raw JSON response
      --reset-api-key              Reset stored API key
      --references                 Enable or disable showing references [default: true]
  -h, --help                       Print help
  -V, --version                    Print version
```

### Examples

#### First-time setup
```bash
fastgpt --config
```

#### Start interactive chat
```bash
fastgpt
```

#### Start with JSON output enabled
```bash
fastgpt --json
```

## Session Commands

While in interactive mode, you can use these special commands:

- `/exit` or `/quit` - Exit the session
- `/clear` - Clear conversation history and start fresh
- `/history` - Show your complete conversation history  
- `/help` - Display available commands

## Configuration

The API key is stored in your system's config directory:
- **Linux**: `~/.config/fastgpt/config.toml`
- **macOS**: `~/Library/Application Support/fastgpt/config.toml`  
- **Windows**: `%APPDATA%\fastgpt\config.toml`

The config file is created automatically when you set your API key.

## Pricing

- **1.5¢ per query** ($15 USD per 1000 queries) with web search enabled
- **Cached responses are free**
- **Note**: Follow-up questions include conversation context, which may result in longer queries
