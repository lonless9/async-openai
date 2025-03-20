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
    /// JSON Array format
    JsonArray,
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

    /// Helper function to determine if a schema value is an array
    fn is_array_schema(value: &serde_json::Value) -> bool {
        matches!(value, serde_json::Value::Array(_))
    }

    /// Helper function to extract the type string from a JSON value
    fn get_type_str(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(n) => {
                if n.is_i64() { "integer" } else { "number" }
            },
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }

    /// Helper function to extract type info for display
    fn get_type_info(value: &serde_json::Value) -> &'static str {
        match value {
            serde_json::Value::Null => " (type not specified)",
            serde_json::Value::Bool(_) => " (boolean)",
            serde_json::Value::Number(n) => {
                if n.is_i64() { " (integer)" } 
                else if n.is_f64() { " (float)" } 
                else { " (number)" }
            },
            serde_json::Value::String(_) => " (string)",
            serde_json::Value::Array(_) => " (array)",
            serde_json::Value::Object(_) => " (object)",
        }
    }

    /// Helper function to extract array item type information
    fn get_array_item_type(array: &[serde_json::Value]) -> &'static str {
        array.first().map_or("any", Self::get_type_str)
    }

    /// Convert the configuration to an instruction
    pub fn to_instruction(&self) -> Instruction {
        let mut content = String::new();

        // Add prefix if available
        if let Some(ref prefix) = self.prefix {
            content.push_str(prefix);
            content.push_str("\n\n");
        }

        // Process schema if available
        if let Some(schema) = &self.schema {
            self.process_schema(schema, &mut content);
        }

        // Add suffix if available
        if let Some(ref suffix) = self.suffix {
            content.push_str("\n");
            content.push_str(suffix);
        }

        Instruction { content }
    }

    /// Process schema and add to instruction content
    fn process_schema(&self, schema: &T, content: &mut String) {
        // Serialize schema to determine its type
        let schema_value = match serde_json::to_value(schema) {
            Ok(value) => value,
            Err(_) => return, // Can't process if serialization fails
        };

        let is_array = Self::is_array_schema(&schema_value);
        
        // Process field descriptions if available
        if let Some(descriptions) = &self.descriptions {
            self.add_field_descriptions(&schema_value, descriptions, is_array, content);
        }

        // Add format-specific content
        match self.format {
            OutputFormat::Json => self.add_json_format(&schema_value, schema, is_array, content),
            OutputFormat::JsonArray => self.add_json_array_format(&schema_value, schema, is_array, content),
            #[cfg(feature = "yaml")]
            OutputFormat::Yaml => self.add_yaml_format(&schema_value, schema, is_array, content),
            #[cfg(feature = "xml")]
            OutputFormat::Xml => self.add_xml_format(&schema_value, is_array, content),
        }
    }

    /// Add field descriptions based on schema type
    fn add_field_descriptions(
        &self,
        schema_value: &serde_json::Value,
        descriptions: &IndexMap<String, String>,
        is_array: bool,
        content: &mut String
    ) {
        content.push_str("The response should include:\n");

        if !is_array {
            // Object type handling
            if let serde_json::Value::Object(map) = schema_value {
                // Process fields with descriptions first
                for (field, description) in descriptions {
                    if let Some(value) = map.get(field) {
                        let type_info = Self::get_type_info(value);
                        content.push_str(&format!("- {}{}: {}\n", field, type_info, description));
                    }
                }
                
                // Add remaining fields without descriptions
                for (field, value) in map {
                    if !descriptions.contains_key(field) {
                        let type_info = Self::get_type_info(value);
                        content.push_str(&format!("- {}{}\n", field, type_info));
                    }
                }
            }
        } else {
            // Array type handling
            if let serde_json::Value::Array(array) = schema_value {
                if let Some(first) = array.first() {
                    let item_type = Self::get_type_str(first);
                    content.push_str(&format!("- An array of {} items\n", item_type));
                    
                    // If first item is an object, describe its structure
                    if let serde_json::Value::Object(map) = first {
                        content.push_str("  Each item should have:\n");
                        
                        for (field, value) in map {
                            let type_info = Self::get_type_info(value);
                            
                            if let Some(desc) = descriptions.get(field) {
                                content.push_str(&format!("  - {}{}: {}\n", field, type_info, desc));
                            } else {
                                content.push_str(&format!("  - {}{}\n", field, type_info));
                            }
                        }
                    }
                } else {
                    content.push_str("- An empty array\n");
                }
            }
        }
        
        content.push_str("\n");
    }

    /// Add properties schema with proper indentation
    fn add_properties_schema(
        &self,
        map: &serde_json::Map<String, serde_json::Value>,
        content: &mut String,
        indent: usize
    ) {
        // Use a Vec to collect all properties first, then join them
        let properties: Vec<String> = map.iter()
            .map(|(field, value)| {
                let type_str = Self::get_type_str(value);
                let indent_str = " ".repeat(indent);
                format!("{}\"{}\":{{\"type\":\"{}\"}}", indent_str, field, type_str)
            })
            .collect();
        
        // Join all properties with commas
        content.push_str(&properties.join(",\n"));
        
        // Add a newline if we added any properties
        if !properties.is_empty() {
            content.push('\n');
        }
    }

    /// Detect possible format for string values
    fn detect_string_format(s: &str) -> Option<&'static str> {
        if s.parse::<i64>().is_ok() {
            Some("numeric-string")
        } else if s.contains('@') && s.contains('.') {
            Some("email")
        } else if s.starts_with("http://") || s.starts_with("https://") {
            Some("uri")
        } else if s.matches('-').count() == 2 && s.len() == 10 {
            // Simple date format like YYYY-MM-DD
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() == 3 
               && parts[0].len() == 4 
               && parts[1].len() == 2 
               && parts[2].len() == 2 
               && parts[0].chars().all(|c| c.is_ascii_digit())
               && parts[1].chars().all(|c| c.is_ascii_digit())
               && parts[2].chars().all(|c| c.is_ascii_digit()) {
                Some("date")
            } else {
                None
            }
        } else if s.matches(':').count() == 2 && s.len() == 8 {
            // Simple time format like HH:MM:SS
            Some("time")
        } else {
            None
        }
    }

    /// Generate nested schema structure directly using serde_json
    fn generate_schema_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut schema = serde_json::json!({
                    "type": "object",
                    "properties": {}
                });
                
                let properties = schema["properties"].as_object_mut().unwrap();
                
                for (field, val) in map {
                    properties.insert(field.clone(), self.generate_schema_json(val));
                }
                
                schema
            },
            serde_json::Value::Array(array) => {
                if let Some(first) = array.first() {
                    serde_json::json!({
                        "type": "array",
                        "items": self.generate_schema_json(first)
                    })
                } else {
                    serde_json::json!({
                        "type": "array",
                        "items": { "type": "any" }
                    })
                }
            },
            serde_json::Value::String(s) => {
                // Enhance string schemas with format detection
                let mut schema = serde_json::json!({"type": "string"});
                
                if let Some(format) = Self::detect_string_format(s) {
                    schema["format"] = serde_json::Value::String(format.to_string());
                }
                
                schema
            },
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    serde_json::json!({"type": "integer"})
                } else {
                    serde_json::json!({"type": "number"})
                }
            },
            serde_json::Value::Bool(_) => serde_json::json!({"type": "boolean"}),
            serde_json::Value::Null => serde_json::json!({"type": "null"}),
        }
    }

    /// Add schema for a specific value with proper indentation
    fn add_schema_for_value(
        &self,
        value: &serde_json::Value,
        content: &mut String,
        indent: usize
    ) {
        // Generate the JSON Schema directly using serde_json
        let schema = self.generate_schema_json(value);
        
        // Format with proper indentation
        if let Ok(schema_str) = serde_json::to_string_pretty(&schema) {
            // Need to adjust the indentation for the pretty printed JSON
            let lines: Vec<String> = schema_str.lines()
                .map(|line| {
                    if line.trim().is_empty() {
                        line.to_string()
                    } else {
                        " ".repeat(indent) + line
                    }
                })
                .collect();
            
            // Join the lines and remove the first indent (which is already handled by the caller)
            let mut result = lines.join("\n");
            if result.starts_with(' ') {
                result = result[indent..].to_string();
            }
            
            content.push_str(&result);
        } else {
            // Fallback to the previous implementation if serialization fails
            match value {
                serde_json::Value::Object(map) => {
                    let indent_str = " ".repeat(indent);
                    content.push_str(&format!("{}\"type\": \"object\",\n", indent_str));
                    content.push_str(&format!("{}\"properties\": {{\n", indent_str));
                    
                    self.add_properties_schema(map, content, indent + 2);
                    
                    content.push_str(&format!("{}}}\n", indent_str));
                },
                _ => {
                    let type_str = Self::get_type_str(value);
                    let indent_str = " ".repeat(indent);
                    content.push_str(&format!("{}\"type\": \"{}\"\n", indent_str, type_str));
                }
            }
        }
    }

    /// Add object schema to content using direct serde_json serialization
    fn add_object_schema(&self, schema_value: &serde_json::Value, content: &mut String) {
        if let serde_json::Value::Object(map) = schema_value {
            // Generate the full schema structure
            let schema = self.generate_schema_json(schema_value);
            
            // Convert to pretty-printed JSON
            if let Ok(schema_str) = serde_json::to_string_pretty(&schema) {
                content.push_str(&schema_str);
                return;
            }
            
            // Fallback to manual construction if serialization fails
            content.push_str("{\n  \"type\": \"object\",\n  \"properties\": {\n");
            self.add_properties_schema(map, content, 4);
            content.push_str("  }\n}\n");
        }
    }

    /// Add array schema to content using direct serde_json serialization
    fn add_array_schema(&self, schema_value: &serde_json::Value, content: &mut String) {
        if let serde_json::Value::Array(array) = schema_value {
            // Generate the full schema structure
            let schema = self.generate_schema_json(schema_value);
            
            // Convert to pretty-printed JSON
            if let Ok(schema_str) = serde_json::to_string_pretty(&schema) {
                content.push_str(&schema_str);
                return;
            }
            
            // Fallback to manual construction if serialization fails
            content.push_str("{\n  \"type\": \"array\",\n");
            
            if !array.is_empty() {
                content.push_str("  \"items\": {\n");
                
                if let Some(first) = array.first() {
                    self.add_schema_for_value(first, content, 4);
                } else {
                    content.push_str("    \"type\": \"any\"\n");
                }
                
                content.push_str("  }\n");
            }
            
            content.push_str("}\n");
        }
    }

    /// Add JSON format information to content
    fn add_json_format(
        &self,
        schema_value: &serde_json::Value,
        schema: &T,
        is_array: bool,
        content: &mut String
    ) {
        content.push_str("Please return the response in JSON format.\n\n");

        if let Ok(json) = serde_json::to_string_pretty(schema) {
            content.push_str(&format!("Example format:\n```json\n{}\n```\n", json));
            
            // Add JSON Schema information
            content.push_str("\nJSON Schema information:\n```json\n");
            
            if is_array {
                self.add_array_schema(schema_value, content);
            } else {
                self.add_object_schema(schema_value, content);
            }
            
            content.push_str("```\n");
        }
    }

    /// Add JSON Array format information to content
    fn add_json_array_format(
        &self,
        schema_value: &serde_json::Value,
        schema: &T,
        is_array: bool,
        content: &mut String
    ) {
        content.push_str("Please return the response as a JSON array of items.\n\n");

        if let Ok(json) = serde_json::to_string_pretty(schema) {
            // Format the example based on whether schema is already an array
            if is_array {
                content.push_str(&format!("Example format:\n```json\n{}\n```\n", json));
            } else {
                // Wrap the object in an array
                content.push_str(&format!("Example format:\n```json\n[\n  {}\n]\n```\n", json));
            }
            
            // Create array schema directly using serde_json
            let array_schema = if is_array {
                // Already an array, just use it
                self.generate_schema_json(schema_value)
            } else {
                // Wrap the object schema in an array schema
                serde_json::json!({
                    "type": "array",
                    "items": self.generate_schema_json(schema_value)
                })
            };
            
            // Print the schema
            content.push_str("\nJSON Schema information:\n```json\n");
            if let Ok(schema_str) = serde_json::to_string_pretty(&array_schema) {
                content.push_str(&schema_str);
            } else {
                // Fallback to the old implementation
                content.push_str("{\n  \"type\": \"array\",\n  \"items\": {\n");
                
                if is_array {
                    if let serde_json::Value::Array(array) = schema_value {
                        if let Some(first) = array.first() {
                            self.add_schema_for_value(first, content, 4);
                        } else {
                            content.push_str("    \"type\": \"any\"\n");
                        }
                    }
                } else if let serde_json::Value::Object(map) = schema_value {
                    content.push_str("    \"type\": \"object\",\n");
                    content.push_str("    \"properties\": {\n");
                    
                    self.add_properties_schema(map, content, 6);
                    
                    content.push_str("    }\n");
                }
                
                content.push_str("  }\n}\n");
            }
            
            content.push_str("```\n");
        }
    }

    #[cfg(feature = "yaml")]
    /// Add YAML format information to content
    fn add_yaml_format(
        &self,
        schema_value: &serde_json::Value,
        schema: &T,
        is_array: bool,
        content: &mut String
    ) {
        content.push_str("Please return the response in YAML format.\n\n");

        if let Ok(yaml) = serde_yaml::to_string(schema) {
            content.push_str(&format!("Example format:\n```yaml\n{}\n```\n", yaml));
            
            // Add a note about the structure type for arrays
            if is_array {
                if let serde_json::Value::Array(array) = schema_value {
                    if let Some(first) = array.first() {
                        if matches!(first, serde_json::Value::Object(_)) {
                            content.push_str("\nThis is an array of objects. Each item should follow the above structure.\n");
                        } else {
                            let item_type = Self::get_type_str(first);
                            content.push_str(&format!("\nThis is an array of {} values.\n", item_type));
                        }
                    } else {
                        content.push_str("\nThis is an empty array.\n");
                    }
                }
            }
        }
    }

    #[cfg(feature = "xml")]
    /// Add XML format information to content
    fn add_xml_format(
        &self,
        schema_value: &serde_json::Value,
        is_array: bool,
        content: &mut String
    ) {
        content.push_str("Please return the response in XML format.\n\n");
        content.push_str("Example format:\n```xml\n<root>\n");

        if is_array {
            if let serde_json::Value::Array(array) = schema_value {
                // Find the first item, if any
                array.first().map_or_else(
                    // No items - empty array
                    || content.push_str("  <!-- Empty array - no items -->\n"),
                    |first| match first {
                        // Object array
                        serde_json::Value::Object(map) => {
                            content.push_str("  <item>\n");
                            
                            // Add fields from the object
                            for (field, value) in map {
                                let value_str = match value {
                                    serde_json::Value::String(s) => s.clone(),
                                    _ => value.to_string(),
                                };
                                content.push_str(&format!("    <{}>{}</{}>\n", field, value_str, field));
                            }
                            
                            content.push_str("  </item>\n");
                            content.push_str("  <!-- Additional items here -->\n");
                        },
                        // Simple value array
                        _ => {
                            let value_str = match first {
                                serde_json::Value::String(s) => s.clone(),
                                _ => first.to_string(),
                            };
                            content.push_str(&format!("  <item>{}</item>\n", value_str));
                            content.push_str("  <!-- Additional items here -->\n");
                        }
                    }
                );
            }
        } else if let serde_json::Value::Object(map) = schema_value {
            // Just add the object fields
            map.iter().for_each(|(field, value)| {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                content.push_str(&format!("  <{}>{}</{}>\n", field, value_str, field));
            });
        }
        
        content.push_str("</root>\n```\n");
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
