use crate::Cli;
use arxivcat_core::config;

pub async fn cmd_status(cli: &Cli) {
    let profile = match config::load_active_api_profile() {
        Ok(profile) => profile,
        Err(e) => crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", &e.to_string()),
    };
    let token = profile.api_key.clone();
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
                                "profile": profile.id,
                                "provider": &profile.name,
                                "model": &profile.model,
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
                                "profile": profile.id,
                                "provider": &profile.name,
                                "model": &profile.model,
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
                                "profile": profile.id,
                                "provider": &profile.name,
                                "model": &profile.model,
                                "response_time_ms": serde_json::Value::Null,
                                "valid": false,
                                "error": e,
                            })
                        );
                    }
                }
                return;
            }
            println!(
                "active provider: {} ({}) / {}",
                profile.id, profile.name, profile.model
            );
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
                        "profile": profile.id,
                        "provider": &profile.name,
                        "model": &profile.model,
                        "response_time_ms": serde_json::Value::Null,
                        "valid": false,
                    })
                );
                return;
            }
            println!("no token configured for {}", profile.name);
            println!("set with: arxivcat token set");
            if profile.id == config::DEEPSEEK_PROFILE {
                println!("or set DEEPSEEK_API_KEY environment variable");
            }
        }
    }
}

pub async fn cmd_set(cli: &Cli, profile_id: Option<u8>) {
    if cli.json {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "--json is not supported for token set",
        );
    }
    use std::io;

    let profile_id = profile_id.unwrap_or_else(|| {
        config::load_config()
            .api_profile
            .unwrap_or(config::DEEPSEEK_PROFILE)
    });
    match profile_id {
        config::DEEPSEEK_PROFILE => {
            let token = read_secret("Enter DeepSeek API token: ", cli);
            if let Err(e) = config::save_token(&token) {
                crate::commands::die(cli, crate::commands::EXIT_IO, "io", &e.to_string());
            }
            println!("token saved for DeepSeek (profile 1)");
        }
        config::CUSTOM_PROFILE => {
            let base_url = read_required(&mut io::stdin(), "Custom API base URL: ", cli);
            let model = read_required(&mut io::stdin(), "Custom API model: ", cli);
            let api_key = read_secret("Custom API token: ", cli);
            if let Err(e) = config::save_custom_api(&base_url, &model, &api_key) {
                crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", &e.to_string());
            }
            println!("custom API configuration saved (profile 2)");
        }
        _ => crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "unknown API profile",
        ),
    }
}

fn read_secret(prompt: &str, cli: &Cli) -> String {
    let value = match rpassword::prompt_password(prompt) {
        Ok(value) => value.trim().to_string(),
        Err(_) => crate::commands::die(cli, crate::commands::EXIT_IO, "io", "error reading input"),
    };
    if value.is_empty() {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "value cannot be empty",
        );
    }
    value
}

fn read_required(stdin: &mut std::io::Stdin, prompt: &str, cli: &Cli) -> String {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();
    let mut value = String::new();
    if stdin.read_line(&mut value).is_err() {
        crate::commands::die(cli, crate::commands::EXIT_IO, "io", "error reading input");
    }
    let value = value.trim().to_string();
    if value.is_empty() {
        crate::commands::die(
            cli,
            crate::commands::EXIT_USAGE,
            "usage",
            "value cannot be empty",
        );
    }
    value
}

pub async fn cmd_list(cli: &Cli) {
    let config_file = config::load_config();
    let active_id = config_file.api_profile.unwrap_or(config::DEEPSEEK_PROFILE);
    let custom = config_file.custom_api;
    if cli.json {
        println!(
            "{}",
            serde_json::json!({"profiles": [
                {"id": 1, "name": "DeepSeek", "base_url": "https://api.deepseek.com", "model": "deepseek-v4-flash", "configured": config_file.deepseek_api_key.is_some(), "active": active_id == 1},
                {"id": 2, "name": "Custom OpenAI-compatible API", "base_url": custom.as_ref().map(|c| &c.base_url), "model": custom.as_ref().map(|c| &c.model), "configured": custom.is_some(), "active": active_id == 2}
            ]})
        );
        return;
    }
    println!(
        "{} 1: DeepSeek (deepseek-v4-flash)",
        if active_id == 1 { "*" } else { " " }
    );
    println!("    https://api.deepseek.com");
    if let Some(custom) = custom {
        println!(
            "{} 2: Custom OpenAI-compatible API ({})",
            if active_id == 2 { "*" } else { " " },
            custom.model
        );
        println!("    {}", custom.base_url);
    } else {
        println!("  2: Custom OpenAI-compatible API (not configured)");
    }
}

pub async fn cmd_use(cli: &Cli, profile_id: u8) {
    if let Err(e) = config::use_api_profile(profile_id) {
        crate::commands::die(cli, crate::commands::EXIT_CONFIG, "config", &e.to_string());
    }
    let profile = config::load_active_api_profile().expect("saved profile must resolve");
    if cli.json {
        println!(
            "{}",
            serde_json::json!({
                "profile": profile.id,
                "provider": &profile.name,
                "model": &profile.model,
            })
        );
    } else {
        println!(
            "active provider: {} ({}) / {}",
            profile.id, profile.name, profile.model
        );
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
                "no token configured for the active API profile",
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
