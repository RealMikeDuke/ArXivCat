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
    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let response = match client
        .get("https://api.deepseek.com/models")
        .header("Authorization", format!("Bearer {token}"))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if e.is_timeout() {
                return Err("request timed out (15s)".into());
            }
            return Err(format!("connection failed: {e}"));
        }
    };

    let elapsed = start.elapsed();

    if response.status().is_success() {
        println!("response time: {:.0}ms", elapsed.as_secs_f64() * 1000.0);
        return Ok(true);
    }

    let msg = match response.status().as_u16() {
        401 => "authentication failed: invalid token",
        429 => "rate limit exceeded — wait and retry",
        403 => "access forbidden — token may lack permissions",
        code => return Err(format!("API returned HTTP {code}")),
    };
    Err(msg.into())
}
