use reqwest::{Client, Result};

#[derive(Debug)]
pub struct MastodonApiConfig {
    pub base_url: String,
    pub token: String,
}

pub async fn toot(config: &MastodonApiConfig, text: &String) -> Result<()> {
    let url = format!("{}/statuses", config.base_url);
    let req_form = [("status", text)];
    let res = Client::new()
        .post(&url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", config.token),
        )
        .form(&req_form)
        .send()
        .await?;

    if !res.status().is_success() {
        panic!("Toot failed.");
    }

    Ok(())
}
