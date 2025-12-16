//! Weather tool using Open-Meteo API (free, no API key required).

use crate::error::ToolError;
use crate::types::{FunctionDefinition, Tool, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

/// Weather tool using Open-Meteo API.
pub struct WeatherTool {
    client: Client,
}

#[derive(Deserialize)]
struct WeatherArgs {
    location: String,
}

#[derive(Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    admin1: Option<String>,
}

#[derive(Deserialize)]
struct WeatherResponse {
    current_weather: CurrentWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    weathercode: i32,
}

impl WeatherTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn weather_code_to_description(code: i32) -> &'static str {
        match code {
            0 => "Clear sky",
            1 | 2 | 3 => "Partly cloudy",
            45 | 48 => "Foggy",
            51 | 53 | 55 => "Drizzle",
            61 | 63 | 65 => "Rain",
            66 | 67 => "Freezing rain",
            71 | 73 | 75 => "Snow",
            77 => "Snow grains",
            80 | 81 | 82 => "Rain showers",
            85 | 86 => "Snow showers",
            95 => "Thunderstorm",
            96 | 99 => "Thunderstorm with hail",
            _ => "Unknown",
        }
    }
}

impl Default for WeatherTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WeatherTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_weather".into(),
                description: "Get current weather for a location. Returns temperature, conditions, and wind speed.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "City name (e.g., 'San Francisco', 'London', 'Tokyo')"
                        }
                    },
                    "required": ["location"]
                }),
            },
        }
    }

    fn name(&self) -> &str {
        "get_weather"
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: WeatherArgs = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let location = args.location.trim();
        if location.is_empty() {
            return Err(ToolError::InvalidArguments("Empty location".into()));
        }

        // Step 1: Geocode the location
        debug!(location = %location, "Geocoding location");
        let geocode_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
            urlencoding::encode(location)
        );

        let geo_response: GeocodingResponse = self
            .client
            .get(&geocode_url)
            .send()
            .await?
            .json()
            .await?;

        let geo = geo_response
            .results
            .and_then(|r| r.into_iter().next())
            .ok_or_else(|| ToolError::ExternalService(format!("Location '{}' not found", location)))?;

        // Step 2: Get weather data
        debug!(lat = geo.latitude, lon = geo.longitude, "Fetching weather");
        let weather_url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
            geo.latitude, geo.longitude
        );

        let weather: WeatherResponse = self
            .client
            .get(&weather_url)
            .send()
            .await?
            .json()
            .await?;

        // Format nice location name
        let location_name = match (&geo.admin1, &geo.country) {
            (Some(admin), Some(country)) => format!("{}, {}, {}", geo.name, admin, country),
            (None, Some(country)) => format!("{}, {}", geo.name, country),
            _ => geo.name,
        };

        let description = Self::weather_code_to_description(weather.current_weather.weathercode);
        let temp_f = weather.current_weather.temperature * 9.0 / 5.0 + 32.0;

        Ok(format!(
            "Weather in {}: {:.1}°C ({:.1}°F), {}. Wind: {:.1} km/h",
            location_name,
            weather.current_weather.temperature,
            temp_f,
            description,
            weather.current_weather.windspeed
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_code_descriptions() {
        assert_eq!(WeatherTool::weather_code_to_description(0), "Clear sky");
        assert_eq!(WeatherTool::weather_code_to_description(61), "Rain");
        assert_eq!(WeatherTool::weather_code_to_description(95), "Thunderstorm");
    }

    #[test]
    fn test_definition() {
        let tool = WeatherTool::new();
        let def = tool.definition();

        assert_eq!(def.tool_type, "function");
        assert_eq!(def.function.name, "get_weather");
    }

    // Integration test - requires network
    #[tokio::test]
    #[ignore] // Run with: cargo test -p tools -- --ignored
    async fn test_weather_integration() {
        let tool = WeatherTool::new();
        let result = tool.execute(r#"{"location": "San Francisco"}"#).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("San Francisco"));
        assert!(content.contains("°C"));
    }
}
