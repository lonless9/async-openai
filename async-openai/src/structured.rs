use crate::types::structured::{
    Config, Instruction, OutputFormat, ParseError, Response, Structured, ValidationOptions,
};
use regex::Regex;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
use serde_json;
#[allow(unused_imports)]
use std::collections::HashMap;
use indexmap::IndexMap;

use std::sync::LazyLock;

// Import validation libraries by default
use {
    jsonschema::{JSONSchema, SchemaResolver, SchemaResolverError},
    schemars::{schema_for, JsonSchema},
    std::sync::Arc,
    url::Url,
};

#[cfg(feature = "yaml")]
use serde_yaml;

#[cfg(feature = "xml")]
use quick_xml::de::from_str as xml_from_str;

/// Regular expressions for extracting structured data
static JSON_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```(?:json)?\s*(\{[\s\S]*?\})\s*```").unwrap());

#[cfg(feature = "yaml")]
static YAML_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```(?:ya?ml)?\s*([\s\S]*?)\s*```").unwrap());

#[cfg(feature = "xml")]
static XML_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```(?:xml)?\s*(<[\s\S]*?>)\s*```").unwrap());

/// Empty schema resolver for JSON Schema validation
struct EmptyResolver;

impl SchemaResolver for EmptyResolver {
    fn resolve(
        &self,
        _value: &serde_json::Value,
        _url: &Url,
        _fragment: &str,
    ) -> Result<Arc<serde_json::Value>, SchemaResolverError> {
        Err(SchemaResolverError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Schema not found",
        )))
    }
}

/// Generator for structured instructions and responses
pub struct Generator<T>
where
    T: Structured + for<'de> Deserialize<'de> + JsonSchema,
{
    config: Config<T>,
    validator: Option<JSONSchema>,
}

// Common implementation for all generators
#[allow(clippy::implicit_hasher)]
impl<T> Generator<T>
where
    T: Structured + for<'de> Deserialize<'de> + JsonSchema,
{
    /// Access the configuration
    #[inline]
    pub fn config(&self) -> &Config<T> {
        &self.config
    }

    /// Generate structured instruction
    #[inline]
    pub fn build_instruction(&self) -> Instruction {
        self.config.to_instruction()
    }

    /// Generate instruction and immediately convert to string
    #[inline]
    pub fn build_instruction_text(&self) -> String {
        self.build_instruction().text().to_string()
    }

    /// Create a new generator with just a schema
    #[inline]
    pub fn with_schema(schema: T) -> Self {
        Self::new(Config::with_schema(schema))
    }

    /// Create a new generator with prefix text and schema
    #[inline]
    pub fn with_prefix_schema(prefix: impl Into<String>, schema: T) -> Self {
        Self::new(Config::with_prefix_schema(prefix, schema))
    }

    /// Add a prefix to the generator's configuration
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.prefix = Some(prefix.into());
        self
    }

    /// Add a suffix to the generator's configuration
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.config.suffix = Some(suffix.into());
        self
    }

    /// Set the output format
    pub fn format(mut self, format: OutputFormat) -> Self {
        self.config.format = format;
        self
    }

    /// Add a field description
    pub fn describe(mut self, field: impl Into<String>, description: impl Into<String>) -> Self {
        let descriptions = self.config.descriptions.get_or_insert_with(IndexMap::new);
        descriptions.insert(field.into(), description.into());
        self
    }

    /// Enable validation
    pub fn validate(mut self, enable: bool) -> Self {
        self.config.validate = enable;
        self
    }

    /// Set validation options
    pub fn validation_options(mut self, options: ValidationOptions) -> Self {
        self.config.validation_options = Some(options);
        self
    }

    /// Parse model response
    pub fn parse_response(&self, response: &str) -> Result<Response<T>, ParseError> {
        match self.config.format {
            OutputFormat::Json => self.parse_json_response(response),
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => self.parse_yaml_response(response),
            #[cfg(feature = "xml")]
            OutputFormat::Xml => self.parse_xml_response(response),
            #[cfg(not(any(feature = "yaml", feature = "xml")))]
            #[allow(unreachable_patterns)]
            _ => Err(ParseError::Other(format!(
                "Unsupported format: {:?}, enable required feature",
                self.config.format
            ))),
        }
    }

    /// Create a new structured generator with validation
    pub fn new(config: Config<T>) -> Self {
        let validator = if config.validate {
            config
                .schema
                .as_ref()
                .and_then(|_| serde_json::to_value(schema_for!(T)).ok())
                .and_then(|schema| {
                    JSONSchema::options()
                        .with_resolver(EmptyResolver)
                        .compile(&schema)
                        .ok()
                })
        } else {
            None
        };

        Self { config, validator }
    }

    /// Parse JSON response with validation
    fn parse_json_response(&self, response: &str) -> Result<Response<T>, ParseError> {
        let data = extract_json(response)?;
        self.validate_and_create_response(data, response)
    }

    #[cfg(feature = "yaml")]
    fn parse_yaml_response(&self, response: &str) -> Result<Response<T>, ParseError> {
        let data = extract_yaml(response)?;
        self.validate_and_create_response(data, response)
    }

    #[cfg(feature = "xml")]
    fn parse_xml_response(&self, response: &str) -> Result<Response<T>, ParseError> {
        let data = extract_xml(response)?;
        self.validate_and_create_response(data, response)
    }

    /// Validate data and create response
    fn validate_and_create_response(
        &self,
        data: T,
        response: &str,
    ) -> Result<Response<T>, ParseError> {
        if !self.config.validate || self.validator.is_none() {
            return Ok(Response {
                data,
                raw_response: response.to_string(),
                validation_messages: None,
            });
        }

        let validator = self.validator.as_ref().unwrap();
        let data_value = serde_json::to_value(&data).map_err(|e| {
            ParseError::ValidationError(format!("Serialization for validation failed: {}", e))
        })?;

        // Execute validation first and store the result
        let validation_result = validator.validate(&data_value);

        // Build response based on validation result
        match validation_result {
            Ok(_) => Ok(Response {
                data,
                raw_response: response.to_string(),
                validation_messages: None,
            }),
            Err(errors) => {
                let validation_messages: Vec<_> =
                    errors.into_iter().map(|e| e.to_string()).collect();

                if self
                    .config
                    .validation_options
                    .as_ref()
                    .map_or(false, |opts| opts.require_all_required_properties)
                {
                    return Err(ParseError::ValidationError(format!(
                        "Validation failed: {:?}",
                        validation_messages
                    )));
                }

                Ok(Response {
                    data,
                    raw_response: response.to_string(),
                    validation_messages: Some(validation_messages),
                })
            }
        }
    }

    /// Parse response and return only the data if successful
    pub fn parse_data(&self, response: &str) -> Result<T, ParseError> {
        self.parse_response(response).map(|r| r.data)
    }

    /// Create a new generator with validation enabled
    pub fn with_validation(schema: T) -> Self {
        Self::with_schema(schema).validate(true)
    }

    /// Helper method to quickly build a generator with common settings
    pub fn quick_build(
        schema: T,
        prefix: impl Into<String>,
        suffix: Option<impl Into<String>>,
        validate: bool,
    ) -> Self {
        let mut gen = Self::with_schema(schema).prefix(prefix).validate(validate);

        if let Some(suffix_text) = suffix {
            gen = gen.suffix(suffix_text);
        }

        gen
    }
}

// Extract common parsing functions to reduce code duplication
/// Extract JSON data from a response string
fn extract_json<T: for<'de> Deserialize<'de>>(response: &str) -> Result<T, ParseError> {
    JSON_REGEX
        .captures(response)
        .and_then(|captures| captures.get(1))
        .map(|json_str| serde_json::from_str(json_str.as_str()))
        .unwrap_or_else(|| serde_json::from_str(response))
        .map_err(|e| ParseError::Extraction(format!("Unable to extract JSON: {}", e)))
}

#[cfg(feature = "yaml")]
/// Extract YAML data from a response string
fn extract_yaml<T: for<'de> Deserialize<'de>>(response: &str) -> Result<T, ParseError> {
    YAML_REGEX
        .captures(response)
        .and_then(|captures| captures.get(1))
        .map(|yaml_str| serde_yaml::from_str(yaml_str.as_str()))
        .unwrap_or_else(|| serde_yaml::from_str(response))
        .map_err(|e| ParseError::Extraction(format!("Unable to extract YAML: {}", e)))
}

#[cfg(feature = "xml")]
/// Extract XML data from a response string
fn extract_xml<T: for<'de> Deserialize<'de>>(response: &str) -> Result<T, ParseError> {
    XML_REGEX
        .captures(response)
        .and_then(|captures| captures.get(1))
        .map(|xml_str| {
            xml_from_str(xml_str.as_str()).map_err(|e| ParseError::XmlParse(e.to_string()))
        })
        .unwrap_or_else(|| {
            xml_from_str(response)
                .map_err(|e| ParseError::XmlParse(format!("Unable to extract XML: {}", e)))
        })
}
