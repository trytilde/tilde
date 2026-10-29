//! Embedding calls through an inference connection, in each provider's own request shape.
//! OpenAI's shape covers OpenAI, Azure OpenAI (its v1 API), Vercel AI Gateway and Baseten;
//! Voyage and Cohere have their own. Documents and queries are embedded differently where the
//! provider distinguishes them.
use crate::error::Error;
use crate::inference::{PostError, Provider, Upstream};
use serde_json::{Value, json};
use std::time::Duration;

pub fn supported(provider: Provider) -> bool {
    matches!(
        provider,
        Provider::OpenAi
            | Provider::AzureOpenAi
            | Provider::VercelAiGateway
            | Provider::Baseten
            | Provider::Voyage
            | Provider::Cohere
    )
}

/// One vector per text, each `dims` long. An answer of the wrong shape is not a rejection: the
/// model would give every text the same.
pub(crate) async fn embed(
    upstream: &Upstream,
    model: &str,
    dims: usize,
    texts: &[String],
    query: bool,
) -> Result<Vec<Vec<f32>>, PostError> {
    // A query is embedded while recall waits; documents in the background.
    let timeout = Duration::from_secs(if query { 10 } else { 30 });
    let (path, body) = match upstream.provider {
        Provider::Voyage => (
            "embeddings",
            json!({"model":model,"input":texts,"output_dimension":dims,
                "input_type": if query {"query"} else {"document"}}),
        ),
        Provider::Cohere => (
            "embed",
            json!({"model":model,"texts":texts,"output_dimension":dims,"embedding_types":["float"],
                "input_type": if query {"search_query"} else {"search_document"}}),
        ),
        Provider::AzureOpenAi => ("v1/embeddings", openai(model, dims, texts)),
        _ => ("embeddings", openai(model, dims, texts)),
    };
    let response = upstream.post_json(path, &body, timeout).await?;
    let vectors: Vec<&Value> = match upstream.provider {
        Provider::Cohere => response["embeddings"]["float"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default(),
        _ => response["data"]
            .as_array()
            .map(|a| a.iter().map(|d| &d["embedding"]).collect())
            .unwrap_or_default(),
    };
    let vectors: Vec<Vec<f32>> = vectors
        .into_iter()
        .map(|v| {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|x| x as f32))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    if vectors.len() != texts.len() || vectors.iter().any(|v| v.len() != dims) {
        return Err(PostError {
            rejected: false,
            error: Error::Invalid(format!(
                "The embedding model did not return {} vectors of {dims} dimensions",
                texts.len()
            )),
        });
    }
    Ok(vectors)
}

/// `dimensions` only for models that take it; others return their native size.
fn openai(model: &str, dims: usize, texts: &[String]) -> Value {
    if model.contains("text-embedding-3") {
        json!({"model":model,"input":texts,"dimensions":dims})
    } else {
        json!({"model":model,"input":texts})
    }
}

/// pgvector's text form.
pub fn literal(vector: &[f32]) -> String {
    let mut out = String::with_capacity(vector.len() * 10 + 2);
    out.push('[');
    for (i, x) in vector.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&x.to_string());
    }
    out.push(']');
    out
}
