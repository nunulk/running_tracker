use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use reqwest::{Client, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
};

#[derive(Debug, Clone)]
pub struct FitbitApiConfig {
    pub base_url: String,
    pub client_id: String,
    pub client_secret: String,
}

pub struct FitbitApi {
    config: FitbitApiConfig,
    client: Client,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthorizationResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct AuthorizationTokens {
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActivityOutput {
    pub start_time: String,
    pub distance: Option<f32>,
    pub duration: u32,
    pub calories: u32,
    pub heart_rate_average: u32,
    pub heart_rate_max: u32,
    pub heart_rate_details: Vec<(String, u32)>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Activity {
    logId: u64,
    activityName: String,
    activityTypeId: u32,
    pub startTime: String,
    pub distance: Option<f32>,
    pub duration: u32,
    pub calories: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Activities {
    activities: Vec<Activity>,
}

const TOKEN_FILE_PATH: &str = "credentials.json";

impl AuthorizationTokens {
    fn from_authorization_response(response: AuthorizationResponse) -> Self {
        let expires_at = Utc::now() + Duration::seconds(response.expires_in as i64);
        Self {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            expires_at,
        }
    }
}

impl ActivityOutput {
    fn new(activity: &Activity, heart_rate_summary: &activity::HeartRateSummary) -> Self {
        Self {
            start_time: activity.startTime.clone(),
            distance: activity.distance,
            duration: activity.duration,
            calories: activity.calories,
            heart_rate_average: heart_rate_summary.average,
            heart_rate_max: heart_rate_summary.max,
            heart_rate_details: heart_rate_summary.details.clone(),
        }
    }
}

impl FitbitApi {
    pub fn new(config: FitbitApiConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn access_token(self: &Self) -> Result<Option<String>> {
        let tokens = load_tokens(TOKEN_FILE_PATH);
        if let Some(tokens) = tokens {
            // 1分余裕をみておく
            if tokens.expires_at > Utc::now() + Duration::seconds(60) {
                return Ok(Some(tokens.access_token));
            }
            let res = self.refresh_token(&tokens.refresh_token).await?;
            if let None = res {
                return Ok(None);
            }
            let tokens = AuthorizationTokens::from_authorization_response(res.unwrap());
            store_tokens(TOKEN_FILE_PATH, &tokens);
            return Ok(Some(tokens.access_token));
        }

        print!("Enter code > ");
        let _ = io::stdout().flush();
        let mut code = String::new();
        io::stdin()
            .read_line(&mut code)
            .expect("Failed to read line.");
        let res = self.authorize(&code.trim_end().to_owned()).await?;
        let tokens = AuthorizationTokens::from_authorization_response(res);
        store_tokens(TOKEN_FILE_PATH, &tokens);
        Ok(Some(tokens.access_token))
    }

    pub async fn fetch_latest_run_activity(
        self: &Self,
        after_date: &NaiveDate,
        token: &String,
    ) -> Result<Option<ActivityOutput>> {
        let query_params = [
            ("afterDate", after_date.format("%Y-%m-%d").to_string()),
            ("sort", "desc".to_owned()),
            ("offset", "0".to_owned()),
            ("limit", "100".to_owned()),
        ];
        let fitbit_url = format!("{}/1/user/-/activities/list.json", &self.config.base_url);
        let res = self
            .client
            .get(&fitbit_url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
            .query(&query_params)
            .send()
            .await?;

        let activities = res.json::<Activities>().await?.activities;
        let run_activity = activities.iter().find(|a| a.activityName == "Run");
        if let Some(activity) = run_activity {
            let xml = self
                .fetch_activity_log(&activity.logId.to_string(), token)
                .await?;
            let content = activity::collect_summary(&xml);
            if content.is_none() {
                println!("Failed to parse activity log: {}", activity.logId);
                return Ok(Some(ActivityOutput::new(&activity, &activity::HeartRateSummary {
                    average: 0,
                    max: 0,
                    details: vec![],
                })));
            }
            Ok(Some(ActivityOutput::new(&activity, &content.unwrap())))
        } else {
            Ok(None)
        }
    }

    async fn fetch_activity_log(self: &Self, log_id: &String, token: &String) -> Result<String> {
        let url = format!(
            "{}/1/user/-/activities/{}.tcx",
            &self.config.base_url, log_id
        );
        let res = self
            .client
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
            .send()
            .await?;
        Ok(res.text().await?)
    }

    async fn refresh_token(
        self: &Self,
        refresh_token: &String,
    ) -> Result<Option<AuthorizationResponse>> {
        let basic_auth = general_purpose::STANDARD.encode(
            format!("{}:{}", &self.config.client_id, &self.config.client_secret).as_bytes(),
        );
        let fitbit_url = format!("{}/oauth2/token", &self.config.base_url);

        let req_form = [
            ("client_id", &self.config.client_id),
            ("grant_type", &"refresh_token".to_owned()),
            ("refresh_token", &refresh_token),
        ];

        let res = self
            .client
            .post(fitbit_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Basic {}", basic_auth),
            )
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .form(&req_form)
            .send()
            .await?;

        match res.error_for_status() {
            Err(e) => Err(e),
            Ok(res) => Ok(Some(res.json::<AuthorizationResponse>().await?)),
        }
    }

    async fn authorize(self: &Self, code: &String) -> Result<AuthorizationResponse> {
        let basic_auth = general_purpose::STANDARD.encode(
            format!("{}:{}", &self.config.client_id, &self.config.client_secret).as_bytes(),
        );
        let fitbit_url = format!("{}/oauth2/token", &self.config.base_url);

        let req_form = [
            ("client_id", &self.config.client_id),
            ("grant_type", &"authorization_code".to_owned()),
            ("code", code),
        ];

        let res = self
            .client
            .post(fitbit_url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Basic {}", basic_auth),
            )
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .form(&req_form)
            .send()
            .await?;

        match res.error_for_status() {
            Err(e) => Err(e),
            Ok(res) => Ok(res.json::<AuthorizationResponse>().await?),
        }
    }
}

fn load_tokens(path: &str) -> Option<AuthorizationTokens> {
    let path = Path::new(path);
    let file = match OpenOptions::new().read(true).open(path) {
        Err(_) => return None,
        Ok(file) => file,
    };
    match serde_json::from_reader(file) {
        Err(_) => None,
        Ok(tokens) => Some(tokens),
    }
}

fn store_tokens(path: &str, tokens: &AuthorizationTokens) {
    let path = Path::new(path);
    let mut file = match OpenOptions::new().read(true).write(true).open(path) {
        Err(_) => File::create(path).expect("Failed to create credentials.json."),
        Ok(file) => file,
    };
    file.write_all(serde_json::to_string_pretty(tokens).unwrap().as_bytes())
        .expect("Failed to write to credentials.json.");
}

mod activity {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct HeartRateBpm {
        value: u32,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Trackpoint {
        heart_rate_bpm: HeartRateBpm,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Trackpoints {
        trackpoint: Vec<Trackpoint>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Lap {
        track: Trackpoints,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Activity {
        id: String,
        lap: Option<Lap>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Activities {
        activity: Vec<Activity>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct TrainingCenterDatabase {
        activities: Activities,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct HeartRateSummary {
        pub average: u32,
        pub max: u32,
        pub details: Vec<(String, u32)>,
    }

    pub fn collect_summary(content: &String) -> Option<HeartRateSummary> {
        let database: TrainingCenterDatabase =
            quick_xml::de::from_str(&content).expect("Failed to parse XML.");
        let lap = &database
        .activities
        .activity
        .get(0)
        .unwrap()
        .lap;
        if lap.is_none() {
            return None;
        }
        let heart_rates = lap.as_ref().unwrap()
            .track.trackpoint
            .iter()
            .map(|p| p.heart_rate_bpm.value)
            .collect::<Vec<u32>>();
        let average = (heart_rates.iter().sum::<u32>() as f32 / heart_rates.len() as f32) as u32;
        let max = *heart_rates.iter().max().unwrap();
        let mut details: Vec<(String, u32)> = Vec::new();
        for rate in heart_rates.iter() {
            let range = match *rate {
                r if r < 115 => "<115",
                r if r >= 115 && r < 150 => "-150",
                _ => ">150",
            }
            .to_owned();
            let el = details.iter().find(|d| d.0 == range);
            match el {
                Some(e) => {
                    let index = details.iter().position(|d| d.0 == range).unwrap();
                    details[index] = (e.0.clone(), e.1 + 1);
                }
                None => details.push((range, 1)),
            };
        }
        return Some(HeartRateSummary {
            average,
            max,
            details,
        });
    }
}

#[cfg(test)]
mod test {
    use std::fs::read_to_string;

    use super::*;

    #[test]
    fn test_load_tokens() {
        let tokens = load_tokens("credentials.json");
        assert!(tokens.is_some());
        assert!(tokens.unwrap().access_token.len() > 0);
    }

    #[test]
    fn test_collect_summary() {
        let path = "data/55326309608.xml";
        let content =
            read_to_string(path).expect(format!("Failed to read from file: {}", path).as_str());
        let summary = activity::collect_summary(&content);
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert_eq!(summary.average, 131);
        assert_eq!(summary.max, 166);
        assert_eq!(
            summary.details.get(0).unwrap(),
            &("<115".to_owned(), 265u32)
        );
        assert_eq!(
            summary.details.get(1).unwrap(),
            &("-150".to_owned(), 1802u32)
        );
        assert_eq!(
            summary.details.get(2).unwrap(),
            &(">150".to_owned(), 418u32)
        );
    }
}
