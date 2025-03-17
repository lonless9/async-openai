use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use indexmap::IndexMap;

#[allow(unused_imports)]
use schemars::{schema_for, JsonSchema};

/// Trait for structured output types
///
/// This trait marks types that can be used as structured output for LLM responses
/// It requires the type to be serializable, deserializable, cloneable, and debuggable
pub trait Structured: Clone + std::fmt::Debug + Serialize {}

// Implement the trait for all types that meet the requirements
impl<T> Structured for T where T: Clone + std::fmt::Debug + Serialize {}

/// Output format for structured data
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// JSON format
    Json,
    /// YAML format (requires yaml feature)
    #[cfg(feature = "yaml")]
    Yaml,
    /// XML format (requires xml feature)
    #[cfg(feature = "xml")]
    Xml,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Json
    }
}

/// Configuration for validating structured data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationOptions {
    /// Whether all required properties must be present
    pub require_all_required_properties: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            require_all_required_properties: true,
        }
    }
}

/// Configuration for structured instructions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: for<'a> Deserialize<'a>"))]
pub struct Config<T: Serialize + for<'a> Deserialize<'a> + Clone + std::fmt::Debug> {
    /// Optional prefix text
    pub prefix: Option<String>,

    /// Optional suffix text
    pub suffix: Option<String>,

    /// Output format for the structured data
    pub format: OutputFormat,

    /// Sample schema (example)
    pub schema: Option<T>,

    /// Optional descriptions for schema fields (ordered by insertion)
    pub descriptions: Option<IndexMap<String, String>>,

    /// Whether to validate the response against the schema
    pub validate: bool,

    /// Validation options
    pub validation_options: Option<ValidationOptions>,

    /// Phantom data for T
    pub _marker: PhantomData<T>,
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone + std::fmt::Debug> Default for Config<T> {
    fn default() -> Self {
        Self {
            prefix: None,
            suffix: None,
            format: OutputFormat::default(),
            schema: None,
            descriptions: None,
            validate: false,
            validation_options: None,
            _marker: PhantomData,
        }
    }
}

impl<T: Serialize + for<'a> Deserialize<'a> + Clone + std::fmt::Debug> Config<T> {
    /// Create a configuration with schema only
    pub fn with_schema(schema: T) -> Self {
        Self {
            schema: Some(schema),
            ..Default::default()
        }
    }

    /// Create a configuration with prefix and schema
    pub fn with_prefix_schema(prefix: impl Into<String>, schema: T) -> Self {
        Self {
            prefix: Some(prefix.into()),
            schema: Some(schema),
            ..Default::default()
        }
    }

    /// Add a prefix to the configuration
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Add a suffix to the configuration
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Set the output format
    pub fn format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Add a field description
    pub fn describe(mut self, field: impl Into<String>, description: impl Into<String>) -> Self {
        let descriptions = self.descriptions.get_or_insert_with(IndexMap::new);
        descriptions.insert(field.into(), description.into());
        self
    }

    /// Enable validation
    pub fn validate(mut self, enable: bool) -> Self {
        self.validate = enable;
        self
    }

    /// Set validation options
    pub fn validation_options(mut self, options: ValidationOptions) -> Self {
        self.validation_options = Some(options);
        self
    }

    /// Convert the configuration to an instruction
    pub fn to_instruction(&self) -> Instruction {
        let mut content = String::new();

        // Add prefix if available
        if let Some(ref prefix) = self.prefix {
            content.push_str(prefix);
            content.push_str("\n\n");
        }

        // Add example
        if let Some(schema) = &self.schema {
            // Add field descriptions if available
            if let Some(descriptions) = &self.descriptions {
                content.push_str("The response should include:\n");

                // Serialize schema to value to extract field names
                if let Ok(value) = serde_json::to_value(schema) {
                    if let serde_json::Value::Object(map) = value {
                        // Use descriptions order for fields when possible
                        for (field, description) in descriptions {
                            if map.contains_key(field) {
                                content.push_str(&format!("- {}: {}\n", field, description));
                            }
                        }
                        
                        // Add any fields from schema that weren't in descriptions
                        for (field, _) in map {
                            if !descriptions.contains_key(&field) {
                                content.push_str(&format!("- {}\n", field));
                            }
                        }
                    }
                }

                content.push_str("\n");
            }

            // Add format specification
            match self.format {
                OutputFormat::Json => {
                    content.push_str("Please return the response in JSON format.\n\n");

                    if let Ok(json) = serde_json::to_string_pretty(schema) {
                        content.push_str(&format!("Example format:\n```json\n{}\n```\n", json));
                    }
                }
                #[cfg(feature = "yaml")]
                OutputFormat::Yaml => {
                    content.push_str("Please return the response in YAML format.\n\n");

                    if let Ok(yaml) = serde_yaml::to_string(schema) {
                        content.push_str(&format!("Example format:\n```yaml\n{}\n```\n", yaml));
                    }
                }
                #[cfg(feature = "xml")]
                OutputFormat::Xml => {
                    content.push_str("Please return the response in XML format.\n\n");

                    content.push_str("Example format:\n```xml\n<root>\n");

                    if let Ok(value) = serde_json::to_value(schema) {
                        if let serde_json::Value::Object(map) = value {
                            for (field, value) in map {
                                let value_str = match value {
                                    serde_json::Value::String(s) => s,
                                    _ => value.to_string(),
                                };
                                content
                                    .push_str(&format!("  <{}>{}</{}>\n", field, value_str, field));
                            }
                        }
                    }

                    content.push_str("</root>\n```\n");
                }
            }
        }

        // Add suffix if available
        if let Some(ref suffix) = self.suffix {
            content.push_str("\n");
            content.push_str(suffix);
        }

        Instruction { content }
    }
}

/// Structured instruction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instruction {
    /// Instruction content
    pub content: String,
}

impl Instruction {
    /// Create a new instruction
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }

    /// Get the instruction text
    pub fn text(&self) -> &str {
        &self.content
    }

    /// Convert the instruction to string
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl<S: Into<String>> From<S> for Instruction {
    fn from(content: S) -> Self {
        Self::new(content)
    }
}

/// Response from structured instruction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: for<'a> Deserialize<'a>"))]
pub struct Response<T: Serialize + for<'a> Deserialize<'a> + Clone + std::fmt::Debug> {
    /// Parsed data
    pub data: T,

    /// Raw response
    pub raw_response: String,

    /// Validation messages (if validation was performed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_messages: Option<Vec<String>>,
}

/// Error types for parsing structured data
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Unable to extract data from response
    #[error("Data extraction error: {0}")]
    Extraction(String),

    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// XML parsing error
    #[cfg(feature = "xml")]
    #[error("XML parsing error: {0}")]
    XmlParse(String),

    /// Other errors
    #[error("Error: {0}")]
    Other(String),
}
