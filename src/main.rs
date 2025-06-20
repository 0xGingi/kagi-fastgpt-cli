use anyhow::{Context, Result};
use clap::Parser;
use colored::*;
use dirs::config_dir;
use html_escape::decode_html_entities;
use regex::Regex;
use reqwest::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "fastgpt")]
#[command(about = "Kagi FastGPT CLI client")]
#[command(version = "0.1.2")]
struct Cli {
    #[arg(long, help = "Set API key (will be saved for future use)")]
    set_api_key: Option<String>,

    #[arg(long, help = "Show current API key")]
    show_api_key: bool,

    #[arg(long, help = "Interactive configuration setup")]
    config: bool,

    #[arg(long, default_value = "true", help = "Whether to allow cached responses")]
    cache: bool,

    #[arg(long, help = "Output raw JSON response")]
    json: bool,

    #[arg(long, help = "Reset stored API key")]
    reset_api_key: bool,

    #[arg(long, help = "Enable or disable showing references (saves to config)")]
    references: Option<bool>,

    #[arg(help = "Query to send to FastGPT")]
    query: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    api_key: Option<String>,
    show_references: Option<bool>,
}

#[derive(Serialize)]
struct FastGPTRequest {
    query: String,
    cache: bool,
    web_search: bool,
}

#[derive(Deserialize, Serialize)]
struct FastGPTResponse {
    meta: Meta,
    data: Data,
}

#[derive(Deserialize, Serialize)]
struct Meta {
    id: String,
    node: String,
    ms: u64,
}

#[derive(Deserialize, Serialize)]
struct Data {
    output: String,
    references: Vec<Reference>,
    tokens: u64,
}

#[derive(Deserialize, Serialize)]
struct Reference {
    title: String,
    snippet: String,
    url: String,
}

#[derive(Clone)]
struct ConversationEntry {
    query: String,
    response: String,
}

struct Session {
    id: String,
    history: Vec<ConversationEntry>,
    client: Client,
    api_key: String,
    cache: bool,
    json_mode: bool,
    show_references: bool,
}

impl Session {
    fn new(api_key: String, cache: bool, json_mode: bool, show_references: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            history: Vec::new(),
            client: Client::new(),
            api_key,
            cache,
            json_mode,
            show_references,
        }
    }

    fn build_contextual_query(&self, current_query: &str) -> String {
        if self.history.is_empty() {
            return current_query.to_string();
        }

        let mut context = String::new();
        context.push_str("Previous conversation context:\n");
        
        for (i, entry) in self.history.iter().take(5).enumerate() {
            context.push_str(&format!("Q{}: {}\nA{}: {}\n\n", i + 1, entry.query, i + 1, entry.response));
        }
        
        context.push_str(&format!("Current question: {}", current_query));
        context
    }

    async fn ask_question(&mut self, query: &str) -> Result<FastGPTResponse> {
        let contextual_query = self.build_contextual_query(query);
        
        let request_body = FastGPTRequest {
            query: contextual_query,
            cache: self.cache,
            web_search: true,
        };

        let response = self.client
            .post("https://kagi.com/api/v0/fastgpt")
            .header("Authorization", format!("Bot {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to FastGPT API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API request failed with status {}: {}", status, error_text);
        }

        let fastgpt_response: FastGPTResponse = response
            .json()
            .await
            .context("Failed to parse response from FastGPT API")?;

        self.history.push(ConversationEntry {
            query: query.to_string(),
            response: fastgpt_response.data.output.clone(),
        });

        Ok(fastgpt_response)
    }

    fn clear_history(&mut self) {
        self.history.clear();
        print!("\x1B[2J\x1B[3J\x1B[H");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        println!("{}", "=".repeat(80).bright_blue());
        println!("{}", "Kagi FastGPT CLI".bright_green().bold());
        println!("{} {}", "Session ID:".dimmed(), self.id.bright_cyan());
        println!("{}", "=".repeat(80).bright_blue());
        println!();
        println!("{}", "Commands:".bright_yellow().bold());
        println!("  {} - Exit the session", "/exit or /quit".bright_cyan());
        println!("  {} - Clear conversation history and screen", "/clear".bright_cyan());
        println!("  {} - Show conversation history", "/history".bright_cyan());
        println!("  {} - Show this help", "/help".bright_cyan());
        println!();
        println!("{} Just start typing your question!", "Tip:".bright_magenta().bold());
        println!();
        println!("{}", "Conversation history cleared and screen reset.".bright_yellow());
    }

    fn show_history(&self) {
        if self.history.is_empty() {
            println!("{}", "No conversation history.".dimmed());
            return;
        }

        println!("{}", "Conversation History:".bright_blue().bold());
        println!("{}", "=".repeat(50).bright_blue());
        
        for (i, entry) in self.history.iter().enumerate() {
            println!("{}. {}: {}", 
                (i + 1).to_string().bright_cyan(),
                "Q".bright_green().bold(), 
                entry.query.white()
            );
            println!("   {}: {}", 
                "A".bright_magenta().bold(), 
                entry.response.dimmed()
            );
            println!();
        }
    }
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = config_dir()
        .context("Could not determine config directory")?;
    
    let app_config_dir = config_dir.join("fastgpt");
    fs::create_dir_all(&app_config_dir)
        .context("Failed to create config directory")?;
    
    Ok(app_config_dir.join("config.toml"))
}

fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    
    if !config_path.exists() {
        return Ok(Config::default());
    }
    
    let config_content = fs::read_to_string(&config_path)
        .context("Failed to read config file")?;
    
    let config: Config = toml::from_str(&config_content)
        .context("Failed to parse config file")?;
    
    Ok(config)
}

fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    
    let config_content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;
    
    fs::write(&config_path, config_content)
        .context("Failed to write config file")?;
    
    Ok(())
}

async fn interactive_config_setup() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    
    println!("{}", "=".repeat(60).bright_blue());
    println!("{}", "Kagi FastGPT CLI Configuration".bright_green().bold());
    println!("{}", "=".repeat(60).bright_blue());
    println!();
    
    println!("{}", "Let's set up your FastGPT CLI configuration!".bright_cyan());
    println!();
    
    let api_key = loop {
        match rl.readline("Enter your Kagi API key: ") {
            Ok(input) => {
                let key = input.trim();
                if key.is_empty() {
                    println!("{}", "API key cannot be empty. Please try again.".bright_red());
                    continue;
                }
                if key.len() < 10 {
                    println!("{}", "API key seems too short. Are you sure this is correct? (y/n)".bright_yellow());
                    match rl.readline("") {
                        Ok(confirm) => {
                            if confirm.trim().to_lowercase().starts_with('y') {
                                break key.to_string();
                            } else {
                                continue;
                            }
                        }
                        Err(_) => continue,
                    }
                } else {
                    break key.to_string();
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "\nConfiguration cancelled.".bright_yellow());
                return Ok(());
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "\nConfiguration cancelled.".bright_yellow());
                return Ok(());
            }
            Err(err) => {
                println!("{} {:?}", "Error reading input:".bright_red(), err);
                continue;
            }
        }
    };
    
    println!();
    println!("{}", "Do you want to show references in responses?".bright_cyan());
    println!("{}", "References provide source links for the information in responses.".dimmed());
    
    let show_references = loop {
        match rl.readline("Show references? (y/n) [default: y]: ") {
            Ok(input) => {
                let response = input.trim().to_lowercase();
                if response.is_empty() || response.starts_with('y') {
                    break true;
                } else if response.starts_with('n') {
                    break false;
                } else {
                    println!("{}", "Please enter 'y' for yes or 'n' for no.".bright_yellow());
                    continue;
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "\nConfiguration cancelled.".bright_yellow());
                return Ok(());
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "\nConfiguration cancelled.".bright_yellow());
                return Ok(());
            }
            Err(err) => {
                println!("{} {:?}", "Error reading input:".bright_red(), err);
                continue;
            }
        }
    };
    
    let config = Config {
        api_key: Some(api_key),
        show_references: Some(show_references),
    };
    
    save_config(&config)?;
    
    println!();
    println!("{}", "✓ Configuration saved successfully!".bright_green().bold());
    println!();
    println!("{}", "Your settings:".bright_blue().bold());
    println!("  {} {}", "API Key:".dimmed(), "Set (hidden for security)".bright_green());
    println!("  {} {}", "Show References:".dimmed(), 
        if show_references { "Enabled".bright_green() } else { "Disabled".bright_red() });
    println!();
    println!("{}", "You can now start using FastGPT by running: fastgpt".bright_cyan());
    println!("{}", "To change these settings later, run: fastgpt --config".dimmed());
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.reset_api_key {
        let config = Config::default();
        save_config(&config)?;
        println!("{}", "API key has been reset.".bright_yellow());
        return Ok(());
    }

    if cli.config {
        interactive_config_setup().await?;
        return Ok(());
    }

    if let Some(api_key) = cli.set_api_key {
        let config = Config {
            api_key: Some(api_key.clone()),
            show_references: None,
        };
        save_config(&config)?;
        println!("{}", "API key has been saved successfully!".bright_green());
        return Ok(());
    }

    let mut config = load_config()?;

    if let Some(show_references) = cli.references {
        config.show_references = Some(show_references);
        save_config(&config)?;
        let status = if show_references { "enabled" } else { "disabled" };
        println!("{} References display has been {}.", "✓".bright_green(), status.bright_cyan());
        return Ok(());
    }

    if cli.show_api_key {
        match &config.api_key {
            Some(key) => {
                let masked_key = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len()-4..])
                } else {
                    "*".repeat(key.len())
                };
                println!("{} {}", "Current API key:".bright_blue(), masked_key.bright_cyan());
            }
            None => println!("{}", "No API key is currently set.".bright_yellow()),
        }
        return Ok(());
    }

    let api_key = config.api_key
        .context("No API key found. Set one with: fastgpt --set-api-key YOUR_KEY")?;
    
    let show_references = config.show_references.unwrap_or(true);

    run_interactive_session(api_key, cli.cache, cli.json, show_references).await?;

    Ok(())
}

async fn run_interactive_session(api_key: String, cache: bool, json_mode: bool, show_references: bool) -> Result<()> {
    let mut session = Session::new(api_key, cache, json_mode, show_references);
    let mut rl = DefaultEditor::new()?;

    print!("\x1B[2J\x1B[3J\x1B[H");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    println!("{}", "=".repeat(80).bright_blue());
    println!("{}", "Kagi FastGPT CLI".bright_green().bold());
    println!("{} {}", "Session ID:".dimmed(), session.id.bright_cyan());
    println!("{}", "=".repeat(80).bright_blue());
    println!();
    println!("{}", "Commands:".bright_yellow().bold());
    println!("  {} - Exit the session", "/exit or /quit".bright_cyan());
    println!("  {} - Clear conversation history and screen", "/clear".bright_cyan());
    println!("  {} - Show conversation history", "/history".bright_cyan());
    println!("  {} - Show this help", "/help".bright_cyan());
    println!();
    println!("{} Just start typing your question!", "Tip:".bright_magenta().bold());
    println!();

    loop {
        match rl.readline(" ") {
            Ok(line) => {
                let input = line.trim();
                
                if input.is_empty() {
                    continue;
                }

                rl.add_history_entry(input).ok();

                match input {
                    "/exit" | "/quit" => {
                        println!("{}", "Goodbye!".bright_green());
                        break;
                    }
                    "/clear" => {
                        session.clear_history();
                        continue;
                    }
                    "/history" => {
                        session.show_history();
                        continue;
                    }
                    "/help" => {
                        println!("{}", "Available commands:".bright_yellow().bold());
                        println!("  {} - Exit the session", "/exit or /quit".bright_cyan());
                        println!("  {} - Clear conversation history and screen", "/clear".bright_cyan());
                        println!("  {} - Show conversation history", "/history".bright_cyan());
                        println!("  {} - Show this help", "/help".bright_cyan());
                        continue;
                    }
                    _ if input.starts_with('/') => {
                        println!("{} Unknown command: {}. Type /help for available commands.", 
                            "Error:".bright_red().bold(), input.bright_red());
                        continue;
                    }
                    _ => {
                        match session.ask_question(input).await {
                            Ok(response) => {
                                if session.json_mode {
                                    println!("{}", serde_json::to_string_pretty(&response)?);
                                } else {
                                    print_formatted_response(&response, input, session.show_references);
                                }
                                println!();
                            }
                            Err(e) => {
                                println!("{} {}", "Error:".bright_red().bold(), e);
                            }
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "Use /exit or /quit to exit.".bright_yellow());
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "Goodbye!".bright_green());
                break;
            }
            Err(err) => {
                println!("{} {:?}", "Error:".bright_red().bold(), err);
                break;
            }
        }
    }

    Ok(())
}

fn format_markdown_text(text: &str) -> String {
    let decoded = decode_html_entities(text).to_string();
    
    let bold_regex = Regex::new(r"\*\*(.*?)\*\*").unwrap();
    let italic_regex = Regex::new(r"\*(.*?)\*").unwrap();
    let code_regex = Regex::new(r"`(.*?)`").unwrap();
    
    let mut formatted = decoded;
    
    formatted = bold_regex.replace_all(&formatted, |caps: &regex::Captures| {
        caps[1].bright_white().bold().to_string()
    }).to_string();
    
    formatted = italic_regex.replace_all(&formatted, |caps: &regex::Captures| {
        caps[1].italic().to_string()
    }).to_string();
    
    formatted = code_regex.replace_all(&formatted, |caps: &regex::Captures| {
        caps[1].on_bright_black().bright_white().to_string()
    }).to_string();
    
    formatted
}

fn remove_reference_numbers(text: &str) -> String {
    let reference_regex = Regex::new(r"【\d+】").unwrap();
    reference_regex.replace_all(text, "").to_string()
}

fn print_formatted_response(response: &FastGPTResponse, query: &str, show_references: bool) {
    println!("{}", "=".repeat(80).bright_blue());
    println!("{} {}", "Query:".bright_green().bold(), query.white());
    println!("{}", "=".repeat(80).bright_blue());
    println!();
    
    let output_text = if show_references {
        response.data.output.clone()
    } else {
        remove_reference_numbers(&response.data.output)
    };
    
    println!("{}", format_markdown_text(&output_text));
    println!();

    if show_references && !response.data.references.is_empty() {
        println!("{}", "References:".bright_yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        for (i, reference) in response.data.references.iter().enumerate() {
            println!("{}. {}", (i + 1).to_string().bright_cyan(), format_markdown_text(&reference.title).bright_white().bold());
            println!("   {}", reference.url.blue().underline());
            if !reference.snippet.is_empty() {
                println!("   {}", format_markdown_text(&reference.snippet).dimmed());
            }
            println!();
        }
    }

    println!("{}", "-".repeat(80).bright_black());
    println!(
        "{} {} | {} {} | {} {}ms",
        "Tokens:".dimmed(),
        response.data.tokens.to_string().bright_magenta(),
        "Node:".dimmed(),
        response.meta.node.bright_magenta(),
        "Time:".dimmed(),
        response.meta.ms.to_string().bright_magenta()
    );
}
