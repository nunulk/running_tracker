use std::error::Error;

use crate::fitbit;
use chrono::DateTime;
use handlebars::{Handlebars, RenderError};
use serde::Serialize;

#[derive(Serialize)]
struct ActivityViewModel {
    start_time: String,
    distance: String,
    duration_in_min: String,
    duration_per_km: String,
    calories: u32,
    heart_rate_average: u32,
    heart_rate_max: u32,
    heart_rate_zone_min_pairs: Vec<(String, u32)>,
}

impl ActivityViewModel {
    fn from_output(output: fitbit::ActivityOutput) -> Self {
        let start_time = DateTime::parse_from_rfc3339(&output.start_time)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        let distance = output.distance.unwrap();
        let duration = output.duration as f32 / 60.0 / 1000.0;

        Self {
            start_time,
            distance: format!("{:.1$}", distance, 3),
            duration_in_min: format!("{:.1$}", duration, 3),
            duration_per_km: format!("{:.1$}", duration / distance, 3),
            calories: output.calories,
            heart_rate_average: output.heart_rate_average,
            heart_rate_max: output.heart_rate_max,
            heart_rate_zone_min_pairs: output
                .heart_rate_details
                .iter()
                .map(|(range, value)| (range.to_owned(), value / 60u32))
                .collect(),
        }
    }
}

const TEMPLATE_PATH: &str = "./templates";

/// Handlebars helper for padding left.
/// usage: {{pad_left value width}}
fn pad_left_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let value = h
        .param(0)
        .and_then(|v| v.value().as_u64())
        .ok_or(RenderError::new("Param 0 is required for format helper."))?;
    let width = h
        .param(1)
        .and_then(|v| v.value().as_u64())
        .ok_or(RenderError::new("Param 1 is required for format helper."))?;
    out.write(&format!("{:>1$}", value, width as usize))?;
    Ok(())
}

pub fn get(
    output: fitbit::ActivityOutput,
    template_name: &String,
) -> Result<String, Box<dyn Error>> {
    if output.distance.is_none() {
        return Ok(String::new());
    }
    let mut handlebars = Handlebars::new();
    handlebars.register_template_file(
        "template",
        format!("{}/{}.hbs", TEMPLATE_PATH, template_name),
    )?;
    handlebars.register_helper("pad_left", Box::new(pad_left_helper));
    let view_model = ActivityViewModel::from_output(output);
    let view = handlebars.render("template", &view_model)?;
    Ok(view)
}
