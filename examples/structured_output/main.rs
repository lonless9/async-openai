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
use futures::StreamExt;
// Correctly import Generator
use async_openai::structured::Generator;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, Write};
use async_openai::types::OutputFormat;

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
struct Joke {
    id: i32,
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
    let generator = Generator::<Vec<Joke>>::with_schema(vec![Joke {
        id: 1,
        joke: "Why did the programmer quit his job?".to_string(),
        explanation: "He didn't get arrays!".to_string(),
    }])
        .prefix("Please generate 3 funny pun jokes.")
        .suffix("Please ensure the joke is original and suitable for readers of all ages.")
        // Fields will be displayed in this exact order
        .describe("id", "The id of the joke")
        .describe("joke", "The question part of the joke")
        .describe("explanation", "The explanation or answer part of the joke")
        .format(OutputFormat::JsonArray);

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
    let mut full_response = String::new();
    let mut lock = std::io::stdout().lock();
    let mut stream = client.chat().create_stream(request).await?;
    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                response.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.reasoning_content {
                        write!(lock, "{}", content).unwrap();
                    }
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{}", content).unwrap();
                        full_response.push_str(content);
                    }
                });
            }
            Err(e) => {
                write!(lock, "Error: {}\n", e).unwrap();
            }
        }
        std::io::stdout().flush()?;
    }

    println!("full_response: {}", full_response);
    let serde_json = generator.parse_data(&full_response)?;
    println!("serde_json: {:?}", serde_json);

    Ok(())
}
