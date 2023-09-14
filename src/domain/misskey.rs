use reqwest::{Client, Result};

#[derive(Debug)]
pub struct MisskeyApiConfig {
    pub base_url: String,
    pub token: String,
}

pub async fn post(config: &MisskeyApiConfig, text: &String) -> Result<()> {
    let url = format!("{}/notes/create", &config.base_url);
    let req_json = serde_json::json!({
        "text": text,
        "i": &config.token,
    });
    let res = Client::new().post(&url).json(&req_json).send().await?;

    if !res.status().is_success() {
        panic!("Post failed.");
    }

    Ok(())
}
