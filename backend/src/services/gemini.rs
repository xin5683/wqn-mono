use serde_json::{Value, json};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

const GEMINI_MODEL: &str = "gemini-2.0-flash";

pub async fn generate_json(
    state: &AppState,
    system_prompt: &str,
    user_prompt: &str,
) -> AppResult<Value> {
    let api_key =
        state.config.gemini_api_key.as_deref().ok_or_else(|| {
            AppError::Configuration("GEMINI_API_KEY is not configured".to_owned())
        })?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{GEMINI_MODEL}:generateContent?key={api_key}"
    );
    let response = state
        .http
        .post(url)
        .json(&json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_prompt }]
            }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "temperature": 0.2
            }
        }))
        .send()
        .await
        .map_err(|err| AppError::External(format!("Gemini request failed: {err}")))?;

    if !response.status().is_success() {
        return Err(AppError::External(format!(
            "Gemini request failed with status {}",
            response.status()
        )));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|err| AppError::External(format!("Invalid Gemini response: {err}")))?;
    let text = body
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::External("Gemini response did not include text".to_owned()))?;

    serde_json::from_str(text)
        .map_err(|err| AppError::External(format!("Invalid Gemini JSON: {err}")))
}

pub async fn extract_problem(state: &AppState, payload: Value) -> AppResult<Value> {
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    generate_json(
        state,
        "Extract a wrong-question notebook problem. Return JSON only with title, content, problem_type, correct_answer, solution_text, suggested_tags.",
        &format!("Extract the problem from this content:\n\n{text}"),
    )
    .await
}

pub async fn categorise_error(state: &AppState, payload: Value) -> AppResult<Value> {
    generate_json(
        state,
        "Categorise a student's error. Return JSON only with broad_category, granular_tag, topic_label, confidence, reasoning.",
        &format!("Categorise this attempt/problem context:\n\n{}", payload),
    )
    .await
}
