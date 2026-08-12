use crate::Cli;
use arxivcat_core::config;

pub async fn cmd_status(cli: &Cli) {
    let token = config::load_cached_token();
    match token {
        Some(t) => {
            let masked = if t.chars().count() > 8 {
                let head: String = t.chars().take(4).collect();
                let tail: String = t
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                format!("{head}...{tail}")
            } else {
                "***".to_string()
            };
            if cli.json {
                // docs/cli.md contract: {"configured","masked","response_time_ms","valid"}
                match validate_token_inner(&t).await {
                    Ok((true, elapsed_ms)) => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "configured": true,
                                "masked": masked,
                                "response_time_ms": elapsed_ms,
                                "valid": true,
                            })
                        );
                    }
                    Ok((false, elapsed_ms)) => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "configured": true,
                                "masked": masked,
                                "response_time_ms": elapsed_ms,
                                "valid": false,
                            })
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "configured": true,
                                "masked": masked,
                                "response_time_ms": serde_json::Value::Null,
                                "valid": false,
                                "error": e,
                            })
                        );
                    }
                }
                return;
            }
            println!("token configured: {masked}");

            match validate_token_inner(&t).await {
                Ok((true, elapsed_ms)) => {
                    println!("status: valid ({elapsed_ms}ms)");
                }
                Ok((false, _)) => println!("status: invalid"),
                Err(e) => println!("status: could not validate ({e})"),
            }
        }
        None => {
            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "configured": false,
                        "masked": serde_json::Value::Null,
                        "response_time_ms": serde_json::Value::Null,
                        "valid": false,
                    })
                );
                return;
            }
            println!("no token configured");
            println!("set with: arxivcat token set");
            println!("or set DEEPSEEK_API_KEY environment variable");
        }
    }
}

pub async fn cmd_set(cli: &Cli) {
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for token set",
        );
    }
    use std::io::{self, Write};

    print!("Enter DeepSeek API token: ");
    io::stdout().flush().ok();
    let mut token = String::new();
    if io::stdin().read_line(&mut token).is_err() {
        crate::commands::die(cli, crate::commands::EXIT_IO, "io", "error reading input");
    }
    let token = token.trim().to_string();
    if token.is_empty() {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "token cannot be empty",
        );
    }

    match config::save_token(&token) {
        Ok(()) => println!("token saved"),
        Err(e) => {
            crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
        }
    }
}

pub async fn cmd_validate(cli: &Cli) {
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for token validate",
        );
    }
    let token = config::load_cached_token();
    let token = match token {
        Some(t) => t,
        None => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                "no token configured",
            );
        }
    };

    match validate_token_inner(&token).await {
        Ok((true, elapsed_ms)) => println!("token is valid ({elapsed_ms}ms)"),
        Ok((false, _)) => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                "token is invalid",
            );
        }
        Err(e) => {
            crate::commands::die(
                cli,
                crate::commands::EXIT_CONFIG,
                "config",
                &format!("validation error: {e}"),
            );
        }
    }
}

async fn validate_token_inner(token: &str) -> Result<(bool, u64), String> {
    let start = std::time::Instant::now();
    let http = match arxivcat_core::net::HttpConfig::new() {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };
    let response = match http
        .client
        .get(http.deepseek_models_url())
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
    let elapsed_ms = (elapsed.as_secs_f64() * 1000.0).round() as u64;

    if response.status().is_success() {
        return Ok((true, elapsed_ms));
    }

    let msg = match response.status().as_u16() {
        401 => "authentication failed: invalid token",
        429 => "rate limit exceeded — wait and retry",
        403 => "access forbidden — token may lack permissions",
        code => return Err(format!("API returned HTTP {code}")),
    };
    Err(msg.into())
}
