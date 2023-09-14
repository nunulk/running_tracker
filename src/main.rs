use chrono::NaiveDate;
use clap::{Parser, ValueEnum};
use dotenvy::{dotenv, var};
use reqwest::Result;

mod domain;
use domain::{fitbit, mastodon, misskey, view};

struct AppConfig {
    fitbit_api_url: String,
    fitbit_client_id: String,
    fitbit_client_secret: String,
    mastodon_api_url: String,
    mastodon_access_token: String,
    misskey_api_url: String,
    misskey_access_token: String,
}

#[derive(Clone, ValueEnum, Debug)]
enum Platform {
    Mastodon,
    Misskey,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Date to fetch (from)
    #[arg(short, long)]
    since: String,

    /// Platform name to post the report
    #[arg(value_enum, default_value_t = crate::Platform::Misskey)]
    platform: Platform,

    /// is preview mode ON
    #[arg(long, default_value_t = false)]
    preview: bool,
}

impl AppConfig {
    fn load() -> Self {
        dotenv().expect("Failed to load .env.");

        let fitbit_api_url = var("FITBIT_API_URL").expect("Failed to get FITBIT_API_URL.");
        let fitbit_client_id = var("FITBIT_CLIENT_ID").expect("Failed to get FITBIT_CLIENT_ID.");
        let fitbit_client_secret =
            var("FITBIT_CLIENT_SECRET").expect("Failed to get FITBIT_CLIENT_SECRET.");
        let mastodon_api_url = var("MASTODON_API_URL").expect("Failed to get MASTODON_API_URL.");
        let mastodon_access_token =
            var("MASTODON_ACCESS_TOKEN").expect("Failed to get MASTODON_ACCESS_TOKEN.");
        let misskey_api_url = var("MISSKEY_API_URL").expect("Failed to get MISSKEY_API_URL.");
        let misskey_access_token =
            var("MISSKEY_ACCESS_TOKEN").expect("Failed to get MISSKEY_ACCESS_TOKEN.");

        Self {
            fitbit_api_url,
            fitbit_client_id,
            fitbit_client_secret,
            mastodon_api_url,
            mastodon_access_token,
            misskey_api_url,
            misskey_access_token,
        }
    }
}

struct AppContext<'a> {
    config: &'a AppConfig,
    arguments: &'a CliArgs,
}

async fn run<'a>(ctx: &'a AppContext<'a>) -> Result<()> {
    let fitbit_api = fitbit::FitbitApi::new(fitbit::FitbitApiConfig {
        base_url: ctx.config.fitbit_api_url.to_owned(),
        client_id: ctx.config.fitbit_client_id.to_owned(),
        client_secret: ctx.config.fitbit_client_secret.to_owned(),
    });

    let access_token = fitbit_api.access_token().await?;
    if access_token.is_none() || access_token.as_ref().unwrap().is_empty() {
        println!("Failed to get access token.");
        return Ok(());
    }

    let arg_since = &ctx.arguments.since;
    let since_date =
        NaiveDate::parse_from_str(&arg_since, "%Y-%m-%d").expect("since must be YYYY-MM-DD.");

    let run = fitbit_api
        .fetch_latest_run_activity(&since_date, &access_token.unwrap())
        .await?;
    if run.is_none() {
        println!("No run activity found.");
        return Ok(());
    }

    let text = view::get(run.unwrap());

    if text.is_err() {
        println!("Failed to create text. {}", text.err().unwrap());
        return Ok(());
    }

    if ctx.arguments.preview {
        println!("==== PREVIEW MODE ====");
        println!("{}", text.unwrap());
    } else {
        post_report(&ctx.arguments.platform, &ctx.config, text.unwrap()).await?;
    }

    Ok(())
}

async fn post_report(platform: &Platform, config: &AppConfig, text: String) -> Result<()> {
    match platform {
        Platform::Mastodon => {
            let mastodon_api_config = mastodon::MastodonApiConfig {
                base_url: config.mastodon_api_url.to_owned(),
                token: config.mastodon_access_token.to_owned(),
            };
            mastodon::post(&mastodon_api_config, &text).await?;
        }
        Platform::Misskey => {
            let misskey_api_config = misskey::MisskeyApiConfig {
                base_url: config.misskey_api_url.to_owned(),
                token: config.misskey_access_token.to_owned(),
            };
            misskey::post(&misskey_api_config, &text).await?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load();
    let arguments = CliArgs::parse();
    let ctx = AppContext {
        config: &config,
        arguments: &arguments,
    };

    run(&ctx).await
}
