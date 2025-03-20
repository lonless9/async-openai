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
    id: String,
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
    let generator = Generator::<Vec<Joke>>::default()
        .prefix("Please generate 10 funny pun jokes.")
        .suffix("Please ensure the joke is original and suitable for readers of all ages.")
        // Fields will be displayed in this exact order
        .describe("id", "The id of the joke")
        .describe("joke", "The question part of the joke")
        .describe("explanation", "The explanation or answer part of the joke")
        ;

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

    let raw_json = r#"
    [
      {
        "id": "1",
    "joke": "Why did the bread go to the doctor?",
    "explanation": "Because it was feeling crumby! (A pun on 'crummy' and the crumby texture of bread.)"
  },
  {
    "id": "2",
    "joke": "What did the grape say when it got stepped on?",
    "explanation": "Nothing—it just let out a little wine! (A pun on 'whine' and the beverage wine.)"
  },
  {
    "id": "3",
    "joke": "Why don’t eggs tell jokes?",
    "explanation": "They’d crack up! (A pun on eggs cracking and laughing too hard.)"
  },
  {
    "id": "4",
    "joke": "What do you call a bear with no teeth?",
    "explanation": "A gummy bear! (A pun on gummy candies and toothless gums.)"
  },
  {
    "id": "5",
    "joke": "Why did the math book look sad?",
    "explanation": "It had too many problems. (A pun on math problems and emotional problems.)"
  },
  {
    "id": "6",
    "joke": "What do you call a dinosaur with an extensive vocabulary?",
    "explanation": "A thesaurus! (A pun on the dinosaur name and the reference book.)"
  },
  {
    "id": "7",
    "joke": "Why did the cookie go to the nurse?",
    "explanation": "It was feeling crumbly! (A pun on feeling unwell and cookie crumbs.)"
  },
  {
    "id": "8",
    "joke": "Why did the smartphone bring headphones to the party?",
    "explanation": "It wanted to stay plugged in! (A pun on being social and charging devices.)"
  },
  {
    "id": "9",
    "joke": "What do you call a cat that loves baking?",
    "explanation": "A whisker-wizard! (A pun on cat whiskers and kitchen whisks.)"
  },
  {
    "id": "10",
    "joke": "Why did the bicycle fall asleep?",
    "explanation": "It was two-tired! (A pun on bicycle tires and feeling exhausted.)"
  }
    ]
    "#;

    // let serde_json = serde_json::from_str::<Vec<Joke>>(raw_json)?;
    // println!("{:?}", serde_json);
    let serde_json_2 = generator.parse_data(raw_json)?;
    println!("{:?}", serde_json_2);

    // Chain style to create request messages
    // let request = CreateChatCompletionRequestArgs::default()
    //     .model("deepseek-ai/DeepSeek-R1".to_string())
    //     .messages([
    //         ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
    //             content: "You are a humorous assistant who specializes in creating pun jokes."
    //                 .into(),
    //             name: None,
    //         }),
    //         ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
    //             content: ChatCompletionRequestUserMessageContent::Text(
    //                 instruction.content().to_string(),
    //             ),
    //             name: None,
    //         }),
    //     ])
    //     .response_format(ResponseFormat::Text)
    //     .build()?;
    // let mut lock = std::io::stdout().lock();
    // let mut stream = client.chat().create_stream(request).await?;
    // while let Some(result) = stream.next().await {
    //     match result {
    //         Ok(response) => {
    //             response.choices.iter().for_each(|chat_choice| {
    //                 if let Some(ref content) = chat_choice.delta.reasoning_content {
    //                     write!(lock, "{}", content).unwrap();
    //                 }
    //                 if let Some(ref content) = chat_choice.delta.content {
    //                     write!(lock, "{}", content).unwrap();
    //                 }
    //             });
    //         }
    //         Err(e) => {
    //             write!(lock, "Error: {}\n", e).unwrap();
    //         }
    //     }
    //     std::io::stdout().flush()?;
    // }

    // // Send request
    // let response = client.chat().create(request).await?;

    // // Get model response
    // if let Some(choice) = response.choices.first() {
    //     if let Some(reasoning_content) = &choice.message.reasoning_content {
    //         write!(io::stdout(), "Reasoning content:\n{}\n", reasoning_content)?;
    //     }
    //     if let Some(content) = &choice.message.content {
    //         write!(io::stdout(), "Model response:\n{}\n", content)?;

    //         // Simplified parsing approach
    //         match generator.parse_data(content) {
    //             Ok(joke) => {
    //                 write!(io::stdout(), "Parsing result (simplified):\n")?;
    //                 write!(io::stdout(), "Joke: {}\n", joke.joke)?;
    //                 write!(io::stdout(), "Explanation: {}\n", joke.explanation)?;
    //             }
    //             Err(e) => {
    //                 write!(io::stdout(), "Parsing failed: {}\n", e)?;
    //             }
    //         }

    //         // Or use the complete parsing API to access validation information
    //         match generator.parse_response(content) {
    //             Ok(structured_response) => {
    //                 write!(io::stdout(), "Parsing result:\n")?;
    //                 write!(io::stdout(), "Joke: {}\n", structured_response.data.joke)?;
    //                 write!(
    //                     io::stdout(),
    //                     "Explanation: {}\n",
    //                     structured_response.data.explanation
    //                 )?;

    //                 if let Some(messages) = structured_response.validation_messages {
    //                     write!(io::stdout(), "\nValidation messages:\n")?;
    //                     for msg in messages {
    //                         write!(io::stdout(), "- {}\n", msg)?;
    //                     }
    //                 }
    //             }
    //             Err(e) => {
    //                 write!(io::stdout(), "Parsing failed: {}\n", e)?;
    //             }
    //         }
    //     }
    // }
    Ok(())
}
