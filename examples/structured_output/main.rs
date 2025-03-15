use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage, CreateChatCompletionRequest,
        Role, structured::{StructuredInstructionConfig, StructuredOutput, StructuredInstructionConfigBuilder},
    },
    Client, StructuredInstructionGenerator, config::OpenAIConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Write};
use async_openai::types::CreateChatCompletionRequestArgs;
use async_openai::types::ResponseFormat;
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
struct Joke {
    /// The question part of the joke
    joke: String,
    /// The explanation or answer part of the joke
    explanation: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create OpenAI client
    dotenv::dotenv().ok();
    let client = Client::with_config(
        OpenAIConfig::default()
            .with_api_key(std::env::var("DEEPSEEK_API_KEY").unwrap())
            .with_api_base(std::env::var("DEEPSEEK_API_BASE").unwrap())
        );
    
    // Create structured instruction configuration
    let config = StructuredInstructionConfigBuilder::new()
        .prefix("Please generate a funny pun joke.")
        .suffix("Please ensure the joke is original and suitable for readers of all ages.")
        .schema(Joke::default())
        .add_description("joke", "The question part of the joke")
        .add_description("explanation", "The explanation or answer part of the joke")
        .validate(true)
        .build();
    
    // Create structured instruction generator
    let generator = StructuredInstructionGenerator::new(config);
    
    // Generate instruction
    let instruction = generator.generate_instruction();
    write!(io::stdout(), "Generated instruction:\n{}\n", instruction.instruction)?;
    
    // Create system message and user message
    let system_message = "You are a humorous assistant who specializes in creating pun jokes.".into();
    let user_message = instruction.instruction.as_str().into();
    
    // Create chat completion request
    let request = CreateChatCompletionRequestArgs::default()
        .model("deepseek-ai/DeepSeek-R1".to_string())
        .messages(vec![
            ChatCompletionRequestMessage::System(system_message),
            ChatCompletionRequestMessage::User(user_message),
        ])
        .response_format(ResponseFormat::Text)
        .build()?;
    
    // Send request
    let response = client.chat().create(request).await?;
    
    // Get model response
    if let Some(choice) = response.choices.first() {
        if let Some(reasoning_content) = &choice.message.reasoning_content {
            write!(io::stdout(), "Reasoning content:\n{}\n", reasoning_content)?;
        }
        if let Some(content) = &choice.message.content {
            write!(io::stdout(), "Model response:\n{}\n", content)?;
            
            // Parse response
            match generator.parse_response(content) {
                Ok(structured_response) => {
                    write!(io::stdout(), "Parsing result:\n")?;
                    write!(io::stdout(), "Joke: {}\n", structured_response.data.joke)?;
                    write!(io::stdout(), "Explanation: {}\n", structured_response.data.explanation)?;
                    
                    #[cfg(feature = "schema-validation")]
                    if let Some(messages) = structured_response.validation_messages {
                        write!(io::stdout(), "\nValidation messages:\n")?;
                        for msg in messages {
                            write!(io::stdout(), "- {}\n", msg)?;
                        }
                    }
                }
                Err(e) => {
                    write!(io::stdout(), "Parsing failed: {}\n", e)?;
                }
            }
        }
    }
    
    Ok(())
} 