use async_openai::types::CreateChatCompletionRequestArgs;
use async_openai::types::ResponseFormat;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    },
    Client,
};
// Correctly import Generator
use async_openai::structured::Generator;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, Write};

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
            .with_api_base(std::env::var("DEEPSEEK_API_BASE").unwrap()),
    );

    // Use chain style to create Generator with ordered field descriptions
    let generator = Generator::with_schema(Joke::default())
        .prefix("Please generate a funny pun joke.")
        .suffix("Please ensure the joke is original and suitable for readers of all ages.")
        // Fields will be displayed in this exact order
        .describe("joke", "The question part of the joke")
        .describe("explanation", "The explanation or answer part of the joke");

    // Alternative way to demonstrate order preservation with multiple fields
    // let generator = Generator::with_schema(Joke::default())
    //     .prefix("Please generate a funny pun joke.")
    //     .describe("explanation", "The explanation or answer part of the joke")
    //     .describe("joke", "The question part of the joke")
    //     .suffix("Please ensure the joke is original and suitable for readers of all ages.");

    // Generate instruction
    let instruction = generator.build_instruction();

    write!(
        io::stdout(),
        "Generated instruction:\n{}\n",
        instruction.content()
    )?;

    // Chain style to create request messages
    let request = CreateChatCompletionRequestArgs::default()
        .model("deepseek-ai/DeepSeek-R1".to_string())
        .messages([
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: "You are a humorous assistant who specializes in creating pun jokes."
                    .into(),
                name: None,
            }),
            ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(
                    instruction.content().to_string(),
                ),
                name: None,
            }),
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

            // Simplified parsing approach
            match generator.parse_data(content) {
                Ok(joke) => {
                    write!(io::stdout(), "Parsing result (simplified):\n")?;
                    write!(io::stdout(), "Joke: {}\n", joke.joke)?;
                    write!(io::stdout(), "Explanation: {}\n", joke.explanation)?;
                }
                Err(e) => {
                    write!(io::stdout(), "Parsing failed: {}\n", e)?;
                }
            }

            // Or use the complete parsing API to access validation information
            match generator.parse_response(content) {
                Ok(structured_response) => {
                    write!(io::stdout(), "Parsing result:\n")?;
                    write!(io::stdout(), "Joke: {}\n", structured_response.data.joke)?;
                    write!(
                        io::stdout(),
                        "Explanation: {}\n",
                        structured_response.data.explanation
                    )?;

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
