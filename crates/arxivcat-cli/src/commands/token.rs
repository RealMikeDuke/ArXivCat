use arxivcat_core::config;
use crate::Cli;

pub async fn cmd_status(_cli: &Cli) {
    let token = config::load_cached_token();
    match token {
        Some(t) => {
            let masked = if t.len() > 8 {
                format!("{}...{}", &t[..4], &t[t.len() - 4..])
            } else {
                "***".to_string()
            };
            println!("token configured: {masked}");

            match validate_token_inner(&t).await {
                Ok(true) => println!("status: valid"),
                Ok(false) => println!("status: invalid"),
                Err(e) => println!("status: could not validate ({e})"),
            }
        }
        None => {
            println!("no token configured");
            println!("set with: arxivcat token set");
            println!("or set DEEPSEEK_API_KEY environment variable");
        }
    }
}

pub async fn cmd_set(_cli: &Cli) {
    use std::io::{self, Write};

    print!("Enter DeepSeek API token: ");
    io::stdout().flush().ok();
    let mut token = String::new();
    if io::stdin().read_line(&mut token).is_err() {
        eprintln!("error reading input");
        std::process::exit(1);
    }
    let token = token.trim().to_string();
    if token.is_empty() {
        eprintln!("error: token cannot be empty");
        std::process::exit(1);
    }

    match config::save_token(&token) {
        Ok(()) => println!("token saved"),
        Err(e) => {
            eprintln!("error saving token: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn cmd_validate(_cli: &Cli) {
    let token = config::load_cached_token();
    let token = match token {
        Some(t) => t,
        None => {
            eprintln!("error: no token configured");
            std::process::exit(1);
        }
    };

    match validate_token_inner(&token).await {
        Ok(true) => println!("token is valid"),
        Ok(false) => {
            eprintln!("token is invalid");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("validation error: {e}");
            std::process::exit(1);
        }
    }
}

async fn validate_token_inner(token: &str) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.deepseek.com/models")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.status().is_success())
}
