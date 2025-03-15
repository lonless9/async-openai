use crate::types::structured::{
    OutputFormat, StructuredDataError, StructuredInstruction, 
    StructuredInstructionConfig, StructuredOutput, StructuredResponse
};
use regex::Regex;
use serde_json;
use std::sync::LazyLock;
use std::collections::HashMap;

#[cfg(feature = "schema-validation")]
use {
    crate::types::structured::ValidationOptions,
    jsonschema::{JSONSchema, SchemaResolver, SchemaResolverError},
    schemars::{JsonSchema, schema_for},
    std::sync::Arc,
    url::Url,
    anyhow,
};

#[cfg(feature = "yaml")]
use serde_yaml;

#[cfg(feature = "xml")]
use quick_xml::de::from_str as xml_from_str;

/// Regular expressions for extracting structured data
static JSON_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```(?:json)?\s*(\{[\s\S]*?\})\s*```").unwrap());

#[cfg(feature = "yaml")]
static YAML_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```(?:ya?ml)?\s*([\s\S]*?)\s*```").unwrap());

#[cfg(feature = "xml")]
static XML_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"```(?:xml)?\s*(<[\s\S]*?>)\s*```").unwrap());

/// Empty schema resolver
#[cfg(feature = "schema-validation")]
struct EmptyResolver;

#[cfg(feature = "schema-validation")]
impl SchemaResolver for EmptyResolver {
    fn resolve(&self, _value: &serde_json::Value, _url: &Url, _fragment: &str) -> Result<Arc<serde_json::Value>, SchemaResolverError> {
        // Return a generic error
        Err(SchemaResolverError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound, 
            "Schema not found"
        )))
    }
}

/// Structured instruction generator
/// 
/// Used to generate structured instructions based on configuration and parse model responses
#[cfg(feature = "schema-validation")]
pub struct StructuredInstructionGenerator<T: StructuredOutput> {
    /// Instruction configuration
    config: StructuredInstructionConfig<T>,
    
    /// JSON Schema validator (if schema-validation feature is enabled)
    validator: Option<JSONSchema>,
}

#[cfg(not(feature = "schema-validation"))]
pub struct StructuredInstructionGenerator<T: StructuredOutput> {
    /// Instruction configuration
    config: StructuredInstructionConfig<T>,
}

#[cfg(feature = "schema-validation")]
impl<T: StructuredOutput> StructuredInstructionGenerator<T> {
    /// Create a new structured instruction generator
    pub fn new(config: StructuredInstructionConfig<T>) -> Self {
        let validator = if config.validate {
            config.schema.as_ref()
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

    /// Generate structured instruction
    pub fn generate_instruction(&self) -> StructuredInstruction {
        let mut instruction = String::new();
        
        // Add prefix text (if any)
        if let Some(prefix) = &self.config.prefix {
            instruction.push_str(&format!("{}\n\n", prefix));
        }
        
        // Add format description
        instruction.push_str(&format!("Please return the response in {} format.\n\n", self.format_name()));
        
        // Add schema description
        if let Some(schema) = &self.config.schema {
            let schema_json = serde_json::to_string_pretty(schema)
                .unwrap_or_else(|_| "{}".to_string());
            
            instruction.push_str(&format!(
                "Please return data that conforms to the following structure:\n```json\n{}\n```\n\n", 
                schema_json
            ));
        }
        
        // Add field descriptions (if any)
        if let Some(descriptions) = &self.config.descriptions {
            if !descriptions.is_empty() {
                instruction.push_str("Field descriptions:\n");
                let desc_text = descriptions.iter()
                    .map(|(field, desc)| format!("- {}: {}", field, desc))
                    .collect::<Vec<_>>()
                    .join("\n");
                instruction.push_str(&format!("{}\n\n", desc_text));
            }
        }
        
        // Add suffix text (if any)
        if let Some(suffix) = &self.config.suffix {
            instruction.push_str(suffix);
        }
        
        #[cfg(feature = "schema-validation")]
        {
            let json_schema = self.config.validate.then(|| 
                serde_json::to_string_pretty(&schema_for!(T)).unwrap_or_default()
            );
            
            return StructuredInstruction {
                instruction,
                format: self.config.format,
                json_schema,
            };
        }
        
        #[cfg(not(feature = "schema-validation"))]
        return StructuredInstruction {
            instruction,
            format: self.config.format,
        };
    }
    
    /// Parse model response
    pub fn parse_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        match self.config.format {
            OutputFormat::Json => self.parse_json_response(response),
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => self.parse_yaml_response(response),
            #[cfg(feature = "xml")]
            OutputFormat::Xml => self.parse_xml_response(response),
            #[cfg(not(any(feature = "yaml", feature = "xml")))]
            _ => Err(StructuredDataError::Other(format!(
                "Unsupported format: {:?}，Please enable the corresponding feature", self.config.format
            ))),
        }
    }
    
    /// Parse JSON response
    fn parse_json_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract JSON from code block
        let data = JSON_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|json_str| serde_json::from_str(json_str.as_str()))
            .unwrap_or_else(|| serde_json::from_str(response))
            .map_err(|e| StructuredDataError::Extraction(
                format!("Unable to extract valid JSON from response: {}", e)
            ))?;
        
        #[cfg(feature = "schema-validation")]
        return self.validate_and_create_response(data, response);
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    #[cfg(feature = "yaml")]
    /// Parse YAML response
    fn parse_yaml_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract YAML from code block
        let data = YAML_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|yaml_str| serde_yaml::from_str(yaml_str.as_str()))
            .unwrap_or_else(|| serde_yaml::from_str(response))
            .map_err(|e| StructuredDataError::Extraction(
                format!("Unable to extract valid YAML from response: {}", e)
            ))?;
        
        #[cfg(feature = "schema-validation")]
        return self.validate_and_create_response(data, response);
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    #[cfg(feature = "xml")]
    /// Parse XML response
    fn parse_xml_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract XML from code block
        let data = XML_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|xml_str| xml_from_str(xml_str.as_str())
                .map_err(|e| StructuredDataError::XmlParse(e.to_string())))
            .unwrap_or_else(|| xml_from_str(response)
                .map_err(|e| StructuredDataError::XmlParse(
                    format!("Unable to extract valid XML from response: {}", e)
                )))?;
        
        #[cfg(feature = "schema-validation")]
        return self.validate_and_create_response(data, response);
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    #[cfg(feature = "schema-validation")]
    /// Validate data and create response
    fn validate_and_create_response(&self, data: T, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        if self.config.validate {
            if let Some(validator) = &self.validator {
                // Convert data to value
                let data_value_result = serde_json::to_value(&data)
                    .map_err(|e| StructuredDataError::ValidationError(
                        format!("Unable to serialize data for validation: {}", e)
                    ));
                
                // Handle error cases early
                let data_value = match data_value_result {
                    Ok(value) => value,
                    Err(e) => return Err(e),
                };
                
                // Validate data
                let validation_result = validator.validate(&data_value);
                
                // Handle validation result
                if let Err(errors) = validation_result {
                    let validation_messages = errors
                        .into_iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>();
                    
                    // If configured to require validation to pass, return error
                    if let Some(options) = &self.config.validation_options {
                        if options.require_all_required_properties {
                            return Err(StructuredDataError::ValidationError(
                                format!("Data validation failed: {:?}", validation_messages)
                            ));
                        }
                    }
                    
                    // Otherwise return response with validation messages
                    return Ok(StructuredResponse {
                        data,
                        raw_response: response.to_string(),
                        validation_messages: Some(validation_messages),
                    });
                }
            }
        }
        
        Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
            validation_messages: None,
        })
    }
    
    /// Get format name
    fn format_name(&self) -> &'static str {
        match self.config.format {
            OutputFormat::Json => "JSON",
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => "YAML",
            #[cfg(feature = "xml")]
            OutputFormat::Xml => "XML",
            #[cfg(not(any(feature = "yaml", feature = "xml")))]
            _ => "Unknown format",
        }
    }
}

#[cfg(not(feature = "schema-validation"))]
impl<T: StructuredOutput> StructuredInstructionGenerator<T> {
    /// Create a new structured instruction generator
    pub fn new(config: StructuredInstructionConfig<T>) -> Self {
        Self { config }
    }

    /// Generate structured instruction
    pub fn generate_instruction(&self) -> StructuredInstruction {
        let mut instruction = String::new();
        
        // Add prefix text (if any)
        if let Some(prefix) = &self.config.prefix {
            instruction.push_str(&format!("{}\n\n", prefix));
        }
        
        // Add format description
        instruction.push_str(&format!("Please return the response in {} format.\n\n", self.format_name()));
        
        // Add schema description
        if let Some(schema) = &self.config.schema {
            let schema_json = serde_json::to_string_pretty(schema)
                .unwrap_or_else(|_| "{}".to_string());
            
            instruction.push_str(&format!(
                "Please return data that conforms to the following structure:\n```json\n{}\n```\n\n", 
                schema_json
            ));
        }
        
        // Add field descriptions (if any)
        if let Some(descriptions) = &self.config.descriptions {
            if !descriptions.is_empty() {
                instruction.push_str("Field descriptions:\n");
                let desc_text = descriptions.iter()
                    .map(|(field, desc)| format!("- {}: {}", field, desc))
                    .collect::<Vec<_>>()
                    .join("\n");
                instruction.push_str(&format!("{}\n\n", desc_text));
            }
        }
        
        // Add suffix text (if any)
        if let Some(suffix) = &self.config.suffix {
            instruction.push_str(suffix);
        }
        
        #[cfg(not(feature = "schema-validation"))]
        return StructuredInstruction {
            instruction,
            format: self.config.format,
        };
    }
    
    /// Parse model response
    pub fn parse_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        match self.config.format {
            OutputFormat::Json => self.parse_json_response(response),
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => self.parse_yaml_response(response),
            #[cfg(feature = "xml")]
            OutputFormat::Xml => self.parse_xml_response(response),
            #[cfg(not(any(feature = "yaml", feature = "xml")))]
            _ => Err(StructuredDataError::Other(format!(
                "Unsupported format: {:?}，Please enable the corresponding feature", self.config.format
            ))),
        }
    }
    
    /// Parse JSON response
    fn parse_json_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract JSON from code block
        let data = JSON_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|json_str| serde_json::from_str(json_str.as_str()))
            .unwrap_or_else(|| serde_json::from_str(response))
            .map_err(|e| StructuredDataError::Extraction(
                format!("Unable to extract valid JSON from response: {}", e)
            ))?;
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    #[cfg(feature = "yaml")]
    /// Parse YAML response
    fn parse_yaml_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract YAML from code block
        let data = YAML_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|yaml_str| serde_yaml::from_str(yaml_str.as_str()))
            .unwrap_or_else(|| serde_yaml::from_str(response))
            .map_err(|e| StructuredDataError::Extraction(
                format!("Unable to extract valid YAML from response: {}", e)
            ))?;
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    #[cfg(feature = "xml")]
    /// Parse XML response
    fn parse_xml_response(&self, response: &str) -> Result<StructuredResponse<T>, StructuredDataError> {
        // First try to extract XML from code block
        let data = XML_REGEX.captures(response)
            .and_then(|captures| captures.get(1))
            .map(|xml_str| xml_from_str(xml_str.as_str())
                .map_err(|e| StructuredDataError::XmlParse(e.to_string())))
            .unwrap_or_else(|| xml_from_str(response)
                .map_err(|e| StructuredDataError::XmlParse(
                    format!("Unable to extract valid XML from response: {}", e)
                )))?;
        
        #[cfg(not(feature = "schema-validation"))]
        return Ok(StructuredResponse {
            data,
            raw_response: response.to_string(),
        });
    }
    
    /// Get format name
    fn format_name(&self) -> &'static str {
        match self.config.format {
            OutputFormat::Json => "JSON",
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => "YAML",
            #[cfg(feature = "xml")]
            OutputFormat::Xml => "XML",
            #[cfg(not(any(feature = "yaml", feature = "xml")))]
            _ => "Unknown format",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
    #[cfg_attr(feature = "schema-validation", derive(schemars::JsonSchema))]
    struct TestJoke {
        joke: String,
        explanation: String,
    }
    
    #[test]
    fn test_generate_instruction() {
        let descriptions = HashMap::from([
            ("joke".to_string(), "The question part of a joke".to_string()),
            ("explanation".to_string(), "The explanation or answer part of a joke".to_string()),
        ]);
        
        let config = StructuredInstructionConfig {
            prefix: Some("Please generate a funny pun joke".to_string()),
            suffix: None,
            format: OutputFormat::Json,
            schema: Some(TestJoke {
                joke: "Question part".to_string(),
                explanation: "Answer part".to_string(),
            }),
            descriptions: Some(descriptions),
            #[cfg(feature = "schema-validation")]
            validate: false,
            #[cfg(feature = "schema-validation")]
            validation_options: None,
            _marker: std::marker::PhantomData,
        };
        
        let generator = StructuredInstructionGenerator::new(config);
        let instruction = generator.generate_instruction();
        
        assert!(instruction.instruction.contains("Please generate a funny pun joke"));
        assert!(instruction.instruction.contains("Please return the response in JSON format"));
        assert!(instruction.instruction.contains("joke"));
        assert!(instruction.instruction.contains("explanation"));
        assert!(instruction.instruction.contains("The question part of a joke"));
        assert!(instruction.instruction.contains("The explanation or answer part of a joke"));
    }
    
    #[test]
    fn test_parse_json_response() {
        let config = StructuredInstructionConfig::<TestJoke>::default();
        let generator = StructuredInstructionGenerator::new(config);
        
        // Test parsing JSON from code block
        let response = r#"
This is a joke:

```json
{
  "joke": "Why does a block of ice always get fired from work?",
  "explanation": "Because it's too much of a \"freezer\" (unprofessional)."
}
```

I hope you enjoy it!
        "#;
        
        let result = generator.parse_response(response).unwrap();
        assert_eq!(result.data, TestJoke {
            joke: "Why does a block of ice always get fired from work?".to_string(),
            explanation: "Because it's too much of a \"freezer\" (unprofessional).".to_string(),
        });
        
        // Test parsing pure JSON
        let response = r#"{"joke":"Why does a block of ice always get fired from work?","explanation":"Because it's too much of a \"freezer\" (unprofessional)."}"#;
        let result = generator.parse_response(response).unwrap();
        assert_eq!(result.data, TestJoke {
            joke: "Why does a block of ice always get fired from work?".to_string(),
            explanation: "Because it's too much of a \"freezer\" (unprofessional).".to_string(),
        });
    }
    
    #[cfg(feature = "yaml")]
    #[test]
    fn test_parse_yaml_response() {
        let mut config = StructuredInstructionConfig::<TestJoke>::default();
        config.format = OutputFormat::Yaml;
        let generator = StructuredInstructionGenerator::new(config);
        
        // Test parsing JSON from code block
        let response = r#"
This is a joke:

```yaml
joke: Why does a block of ice always get fired from work?
explanation: Because it's too much of a "freezer" (unprofessional).
```

I hope you enjoy it!
        "#;
        
        let result = generator.parse_response(response).unwrap();
        assert_eq!(result.data, TestJoke {
            joke: "Why does a block of ice always get fired from work?".to_string(),
            explanation: "Because it's too much of a "freezer" (unprofessional).".to_string(),
        });
    }
    
    #[cfg(feature = "schema-validation")]
    #[test]
    fn test_validation() {
        let mut config = StructuredInstructionConfig::<TestJoke>::default();
        config.validate = true;
        let generator = StructuredInstructionGenerator::new(config);
        
        // Test valid data
        let valid_response = r#"{"joke":"Valid joke","explanation":"Valid explanation"}"#;
        let valid_result = generator.parse_response(valid_response).unwrap();
        assert!(valid_result.validation_messages.is_none());
        
        // Test missing field data
        let invalid_response = r#"{"joke":"Invalid joke"}"#;
        let result = generator.parse_response(invalid_response);
        assert!(result.is_err());
    }
} 