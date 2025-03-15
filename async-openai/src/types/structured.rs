use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;

#[cfg(feature = "schema-validation")]
use schemars::JsonSchema;

/// Trait representing structured output
#[cfg(feature = "schema-validation")]
pub trait StructuredOutput: Serialize + for<'de> Deserialize<'de> + Send + Sync + JsonSchema {}

#[cfg(not(feature = "schema-validation"))]
pub trait StructuredOutput: Serialize + for<'de> Deserialize<'de> + Send + Sync {}

/// Automatically implement StructuredOutput for all types that implement the necessary traits
#[cfg(feature = "schema-validation")]
impl<T> StructuredOutput for T where T: Serialize + for<'de> Deserialize<'de> + Send + Sync + JsonSchema {}

#[cfg(not(feature = "schema-validation"))]
impl<T> StructuredOutput for T where T: Serialize + for<'de> Deserialize<'de> + Send + Sync {}

/// Structured data parsing error
#[derive(Debug, thiserror::Error)]
pub enum StructuredDataError {
    #[error("JSON parsing failed: {0}")]
    JsonParse(#[from] serde_json::Error),
    
    #[error("Unable to extract JSON from response: {0}")]
    Extraction(String),
    
    #[cfg(feature = "yaml")]
    #[error("YAML parsing failed: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    
    #[cfg(feature = "xml")]
    #[error("XML parsing failed: {0}")]
    XmlParse(String),
    
    #[cfg(feature = "schema-validation")]
    #[error("Validation failed: {0}")]
    ValidationError(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Structured instruction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-validation", derive(JsonSchema))]
#[serde(bound = "T: StructuredOutput")]
pub struct StructuredInstructionConfig<T: StructuredOutput> {
    /// Prefix text for generating instructions
    pub prefix: Option<String>,
    
    /// Suffix text for generating instructions
    pub suffix: Option<String>,
    
    /// Specified output format (default is JSON)
    pub format: OutputFormat,
    
    /// Schema data (for describing expected structure)
    pub schema: Option<T>,
    
    /// Schema descriptions (for describing field meanings)
    pub descriptions: Option<HashMap<String, String>>,
    
    /// Whether to perform schema validation (requires schema-validation feature)
    #[cfg(feature = "schema-validation")]
    pub validate: bool,
    
    /// Custom validation options
    #[cfg(feature = "schema-validation")]
    pub validation_options: Option<ValidationOptions>,
    
    /// Type marker (PhantomData)
    #[serde(skip)]
    pub _marker: PhantomData<T>,
}

/// Builder for StructuredInstructionConfig
#[derive(Debug, Clone)]
pub struct StructuredInstructionConfigBuilder<T: StructuredOutput> {
    /// Internal configuration
    config: StructuredInstructionConfig<T>,
}

impl<T: StructuredOutput> StructuredInstructionConfigBuilder<T> {
    /// Create new builder instance
    pub fn new() -> Self {
        Self {
            config: StructuredInstructionConfig {
                prefix: None,
                suffix: None,
                format: OutputFormat::default(),
                schema: None,
                descriptions: Some(HashMap::new()),
                #[cfg(feature = "schema-validation")]
                validate: false,
                #[cfg(feature = "schema-validation")]
                validation_options: None,
                _marker: PhantomData,
            }
        }
    }
    
    /// Set prefix text
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.prefix = Some(prefix.into());
        self
    }
    
    /// Set suffix text
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.config.suffix = Some(suffix.into());
        self
    }
    
    /// Set output format
    pub fn format(mut self, format: OutputFormat) -> Self {
        self.config.format = format;
        self
    }
    
    /// Set data structure template
    pub fn schema(mut self, schema: T) -> Self {
        self.config.schema = Some(schema);
        self
    }
    
    /// Add field description
    pub fn add_description(mut self, field: impl Into<String>, description: impl Into<String>) -> Self {
        let descriptions = self.config.descriptions.get_or_insert_with(HashMap::new);
        descriptions.insert(field.into(), description.into());
        self
    }
    
    /// Batch add field descriptions
    pub fn add_descriptions<K, V, I>(mut self, descriptions: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        let config_descriptions = self.config.descriptions.get_or_insert_with(HashMap::new);
        
        for (field, description) in descriptions {
            config_descriptions.insert(field.into(), description.into());
        }
        
        self
    }
    
    /// Set whether to perform data validation (requires schema-validation feature)
    #[cfg(feature = "schema-validation")]
    pub fn validate(mut self, validate: bool) -> Self {
        self.config.validate = validate;
        self
    }
    
    /// Set validation options (requires schema-validation feature)
    #[cfg(feature = "schema-validation")]
    pub fn validation_options(mut self, options: ValidationOptions) -> Self {
        self.config.validation_options = Some(options);
        self
    }
    
    /// Build and return configuration
    pub fn build(self) -> StructuredInstructionConfig<T> {
        // If descriptions are empty, set to None
        let mut config = self.config;
        if let Some(descriptions) = &config.descriptions {
            if descriptions.is_empty() {
                config.descriptions = None;
            }
        }
        
        config
    }
}

impl<T: StructuredOutput> Default for StructuredInstructionConfigBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported output formats
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema-validation", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// JSON format
    Json,
    
    /// YAML format (requires yaml feature)
    #[cfg(feature = "yaml")]
    Yaml,
    
    /// XML format (requires xml feature)
    #[cfg(feature = "xml")]
    #[serde(rename = "xml")]
    Xml,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Json
    }
}

/// Validation options
#[cfg(feature = "schema-validation")]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationOptions {
    /// Whether to allow additional properties
    pub allow_additional_properties: bool,
    
    /// Whether to require all required properties
    pub require_all_required_properties: bool,
}

#[cfg(feature = "schema-validation")]
impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            allow_additional_properties: false,
            require_all_required_properties: true,
        }
    }
}

/// Structured instruction generation result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-validation", derive(JsonSchema))]
pub struct StructuredInstruction {
    /// Generated complete instruction
    pub instruction: String,
    
    /// Format for parsing response
    pub format: OutputFormat,
    
    /// Generated JSON Schema string (if schema-validation feature is enabled)
    #[cfg(feature = "schema-validation")]
    pub json_schema: Option<String>,
}

/// Structured response parsing result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "T: StructuredOutput")]
pub struct StructuredResponse<T: StructuredOutput> {
    /// Parsed structured data
    pub data: T,
    
    /// Original response text
    pub raw_response: String,
    
    /// Validation messages (only valid when schema-validation feature is enabled)
    #[cfg(feature = "schema-validation")]
    pub validation_messages: Option<Vec<String>>,
}

impl<T: StructuredOutput> Default for StructuredInstructionConfig<T> 
where
    T: Default,
{
    fn default() -> Self {
        StructuredInstructionConfigBuilder::new()
            .schema(T::default())
            .build()
    }
} 