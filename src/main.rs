use chrono::NaiveDate;
use clap::Parser;
use dotenvy::{dotenv, var};
use reqwest::Result;

mod domain;
use domain::{fitbit, mastodon, view};

struct AppConfig {
    fitbit_api_url: String,
    fitbit_client_id: String,
    fitbit_client_secret: String,
    mastodon_api_url: String,
    mastodon_access_token: String,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// Date to fetch (from)
    #[arg(short, long)]
    since: String,

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
            var("MASTODON_ACCESS_TOKEN").expect("Faild to get MASTODON_ACCESS_TOKEN.");

        Self {
            fitbit_api_url,
            fitbit_client_id,
            fitbit_client_secret,
            mastodon_api_url,
            mastodon_access_token,
        }
    }
}

struct AppContext<'a> {
    config: &'a AppConfig,
    arguments: &'a CliArgs
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

    let mastodon_api_config = mastodon::MastodonApiConfig {
        base_url: ctx.config.mastodon_api_url.to_owned(),
        token: ctx.config.mastodon_access_token.to_owned(),
    };
    let text = view::get(run.unwrap());

    if text.is_err() {
        println!("Failed to create text. {}", text.err().unwrap());
        return Ok(());
    }

    if ctx.arguments.preview {
        println!("==== PREVIEW MODE ====");
        println!("{}", text.unwrap());
    } else {
        mastodon::toot(&mastodon_api_config, &text.unwrap()).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = AppConfig::load();
    let arguments = CliArgs::parse();
    let ctx = AppContext { config: &config, arguments: &arguments };

    run(&ctx).await
}
