//! Call an [`LlmProvider`] and parse the completion into a [`model::Dag`].

use llm_gateway::{ChatRequest, LlmProvider, MessageRole};
use model::Dag;

use crate::context::{DagGenerateInputs, GenerationContext};
use crate::error::GenerateError;
use crate::parse::parse_dag_from_llm_content;
use crate::template::PromptTemplate;

/// Model id and message role for the completion request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagGenerateRequestOptions {
    pub model: String,
    pub role: MessageRole,
}

impl Default for DagGenerateRequestOptions {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".into(),
            role: MessageRole::User,
        }
    }
}

/// Assemble context, call `provider`, tolerate markdown wrappers, deserialize [`Dag`].
pub async fn generate_dag<P: LlmProvider + ?Sized>(
    provider: &P,
    template: &PromptTemplate,
    inputs: &DagGenerateInputs,
    options: &DagGenerateRequestOptions,
) -> Result<Dag, GenerateError> {
    let ctx = GenerationContext::assemble(inputs)?;
    let request: ChatRequest = ctx.redacted_chat_request(template, &options.model, options.role);
    let response = provider.chat(request).await?;
    parse_dag_from_llm_content(&response.content)
}
