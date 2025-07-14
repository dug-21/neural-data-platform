// config-engine.rs - YAML-Driven Configuration Engine Implementation

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_yaml;
use serde_json;
use tokio::sync::{broadcast, Mutex};
use async_trait::async_trait;
use thiserror::Error;
use jsonschema::{Draft, JSONSchema};
use regex::Regex;
use chrono::{DateTime, Utc};

// Error types for configuration engine
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("YAML parsing error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonParse(#[from] serde_json::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Schema error: {0}")]
    Schema(String),
    
    #[error("Secret resolution error: {0}")]
    SecretResolution(String),
    
    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),
    
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),
    
    #[error("Component not found: {0}")]
    ComponentNotFound(String),
}

// Configuration value that supports different types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
    Object(HashMap<String, ConfigValue>),
}

// Configuration source types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigSource {
    File {
        paths: Vec<String>,
    },
    Environment {
        prefix: String,
    },
    Vault {
        endpoint: String,
        auth: VaultAuth,
    },
    Consul {
        endpoint: String,
        prefix: String,
    },
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultAuth {
    pub method: String,
    pub role: Option<String>,
    pub token: Option<String>,
}

// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    #[serde(flatten)]
    pub data: HashMap<String, ConfigValue>,
    
    #[serde(skip)]
    pub metadata: ConfigMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigMetadata {
    pub version: String,
    pub loaded_at: Option<DateTime<Utc>>,
    pub sources: Vec<String>,
    pub environment: String,
}

// Component definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfig {
    pub name: String,
    pub component_type: String,
    pub config: HashMap<String, ConfigValue>,
    pub dependencies: Vec<String>,
    pub lifecycle: Option<ComponentLifecycle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentLifecycle {
    pub startup_order: i32,
    pub health_check: Option<HealthCheckConfig>,
    pub shutdown_timeout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub endpoint: String,
    pub interval: String,
    pub timeout: String,
    pub retries: i32,
}

// Schema validator
pub struct SchemaValidator {
    schemas: HashMap<String, JSONSchema>,
}

impl SchemaValidator {
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }
    
    pub fn load_schema(&mut self, name: &str, schema_path: &Path) -> Result<(), ConfigError> {
        let schema_content = std::fs::read_to_string(schema_path)?;
        let schema_json: serde_json::Value = serde_json::from_str(&schema_content)?;
        
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_json)
            .map_err(|e| ConfigError::Schema(e.to_string()))?;
            
        self.schemas.insert(name.to_string(), compiled);
        Ok(())
    }
    
    pub fn validate(&self, schema_name: &str, value: &serde_json::Value) -> Result<(), ConfigError> {
        let schema = self.schemas.get(schema_name)
            .ok_or_else(|| ConfigError::Schema(format!("Schema '{}' not found", schema_name)))?;
            
        let result = schema.validate(value);
        if let Err(errors) = result {
            let error_messages: Vec<String> = errors
                .map(|e| format!("{}: {}", e.instance_path, e))
                .collect();
            return Err(ConfigError::Validation(error_messages.join("; ")));
        }
        
        Ok(())
    }
}

// Secret resolver trait
#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, secret_ref: &str) -> Result<String, ConfigError>;
}

// Vault secret resolver implementation
pub struct VaultSecretResolver {
    client: Arc<Mutex<reqwest::Client>>,
    config: VaultAuth,
    endpoint: String,
}

#[async_trait]
impl SecretResolver for VaultSecretResolver {
    async fn resolve(&self, secret_ref: &str) -> Result<String, ConfigError> {
        // Parse secret reference: vault/path/to/secret
        let parts: Vec<&str> = secret_ref.split('/').collect();
        if parts.len() < 2 {
            return Err(ConfigError::SecretResolution(
                format!("Invalid secret reference: {}", secret_ref)
            ));
        }
        
        let path = parts[1..].join("/");
        let url = format!("{}/v1/{}", self.endpoint, path);
        
        let client = self.client.lock().await;
        let response = client.get(&url)
            .header("X-Vault-Token", self.config.token.as_ref().unwrap_or(&String::new()))
            .send()
            .await
            .map_err(|e| ConfigError::SecretResolution(e.to_string()))?;
            
        if !response.status().is_success() {
            return Err(ConfigError::SecretResolution(
                format!("Failed to fetch secret: {}", response.status())
            ));
        }
        
        let body: serde_json::Value = response.json().await
            .map_err(|e| ConfigError::SecretResolution(e.to_string()))?;
            
        // Extract secret value from Vault response
        body.get("data")
            .and_then(|d| d.get("data"))
            .and_then(|d| d.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ConfigError::SecretResolution(
                "Secret value not found in Vault response".to_string()
            ))
    }
}

// Environment variable resolver
pub struct EnvVarResolver {
    env_vars: HashMap<String, String>,
    secret_pattern: Regex,
}

impl EnvVarResolver {
    pub fn new() -> Self {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        let secret_pattern = Regex::new(r"\$\{([^}]+)\}").unwrap();
        
        Self {
            env_vars,
            secret_pattern,
        }
    }
    
    pub fn resolve_value(&self, value: &str) -> Result<String, ConfigError> {
        let mut result = value.to_string();
        
        for cap in self.secret_pattern.captures_iter(value) {
            let full_match = &cap[0];
            let var_spec = &cap[1];
            
            let resolved = self.resolve_var_spec(var_spec)?;
            result = result.replace(full_match, &resolved);
        }
        
        Ok(result)
    }
    
    fn resolve_var_spec(&self, spec: &str) -> Result<String, ConfigError> {
        // Handle different variable specifications:
        // VAR_NAME - simple variable
        // VAR_NAME:-default - with default value
        // VAR_NAME:int - with type hint
        // VAR_NAME:-default:int - with default and type
        
        let parts: Vec<&str> = spec.splitn(2, ":-").collect();
        let var_parts: Vec<&str> = parts[0].split(':').collect();
        let var_name = var_parts[0];
        let type_hint = var_parts.get(1);
        
        let value = self.env_vars.get(var_name)
            .or_else(|| parts.get(1).map(|s| s.split(':').next().unwrap_or(s)))
            .ok_or_else(|| ConfigError::EnvVarNotFound(var_name.to_string()))?;
            
        // Apply type conversion if specified
        match type_hint {
            Some(&"int") => value.parse::<i64>()
                .map(|v| v.to_string())
                .map_err(|_| ConfigError::Validation(
                    format!("Cannot parse '{}' as integer", value)
                )),
            Some(&"bool") => Ok(match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => "true",
                _ => "false"
            }.to_string()),
            Some(&"float") => value.parse::<f64>()
                .map(|v| v.to_string())
                .map_err(|_| ConfigError::Validation(
                    format!("Cannot parse '{}' as float", value)
                )),
            _ => Ok(value.to_string())
        }
    }
}

// Configuration loader
pub struct ConfigLoader {
    sources: Vec<ConfigSource>,
    validator: SchemaValidator,
    secret_resolvers: HashMap<String, Arc<dyn SecretResolver>>,
    env_resolver: EnvVarResolver,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            validator: SchemaValidator::new(),
            secret_resolvers: HashMap::new(),
            env_resolver: EnvVarResolver::new(),
        }
    }
    
    pub fn add_source(&mut self, source: ConfigSource) {
        self.sources.push(source);
    }
    
    pub fn add_secret_resolver(&mut self, name: String, resolver: Arc<dyn SecretResolver>) {
        self.secret_resolvers.insert(name, resolver);
    }
    
    pub async fn load(&self) -> Result<Configuration, ConfigError> {
        let mut merged_config = HashMap::new();
        let mut sources_loaded = Vec::new();
        
        // Load from each source
        for source in &self.sources {
            let source_data = self.load_source(source).await?;
            sources_loaded.push(format!("{:?}", source));
            
            // Deep merge configurations
            self.deep_merge(&mut merged_config, source_data);
        }
        
        // Resolve environment variables and secrets
        let resolved_config = self.resolve_all_values(merged_config).await?;
        
        // Create configuration with metadata
        let mut config = Configuration {
            data: resolved_config,
            metadata: ConfigMetadata {
                version: "1.0.0".to_string(),
                loaded_at: Some(Utc::now()),
                sources: sources_loaded,
                environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            },
        };
        
        Ok(config)
    }
    
    async fn load_source(&self, source: &ConfigSource) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        match source {
            ConfigSource::File { paths } => self.load_from_files(paths),
            ConfigSource::Environment { prefix } => self.load_from_env(prefix),
            ConfigSource::Vault { endpoint, auth } => self.load_from_vault(endpoint, auth).await,
            ConfigSource::Consul { endpoint, prefix } => self.load_from_consul(endpoint, prefix).await,
            ConfigSource::Http { url, headers } => self.load_from_http(url, headers).await,
        }
    }
    
    fn load_from_files(&self, paths: &[String]) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let mut result = HashMap::new();
        
        for path in paths {
            let resolved_path = self.env_resolver.resolve_value(path)?;
            if Path::new(&resolved_path).exists() {
                let content = std::fs::read_to_string(&resolved_path)?;
                let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                let config_value = self.yaml_to_config_value(yaml_value);
                
                if let ConfigValue::Object(map) = config_value {
                    self.deep_merge(&mut result, map);
                }
            }
        }
        
        Ok(result)
    }
    
    fn load_from_env(&self, prefix: &str) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let mut result = HashMap::new();
        
        for (key, value) in &self.env_resolver.env_vars {
            if key.starts_with(prefix) {
                let config_key = key[prefix.len()..].to_lowercase().replace('_', ".");
                let config_value = ConfigValue::String(value.clone());
                self.set_nested_value(&mut result, &config_key, config_value);
            }
        }
        
        Ok(result)
    }
    
    async fn load_from_vault(&self, endpoint: &str, auth: &VaultAuth) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        // Implementation would connect to Vault and load configuration
        // This is a placeholder
        Ok(HashMap::new())
    }
    
    async fn load_from_consul(&self, endpoint: &str, prefix: &str) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        // Implementation would connect to Consul and load configuration
        // This is a placeholder
        Ok(HashMap::new())
    }
    
    async fn load_from_http(&self, url: &str, headers: &HashMap<String, String>) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let client = reqwest::Client::new();
        let mut request = client.get(url);
        
        for (key, value) in headers {
            request = request.header(key, value);
        }
        
        let response = request.send().await
            .map_err(|e| ConfigError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            
        let content = response.text().await
            .map_err(|e| ConfigError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;
        let config_value = self.yaml_to_config_value(yaml_value);
        
        if let ConfigValue::Object(map) = config_value {
            Ok(map)
        } else {
            Ok(HashMap::new())
        }
    }
    
    fn yaml_to_config_value(&self, yaml: serde_yaml::Value) -> ConfigValue {
        match yaml {
            serde_yaml::Value::Null => ConfigValue::String(String::new()),
            serde_yaml::Value::Bool(b) => ConfigValue::Boolean(b),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ConfigValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    ConfigValue::Float(f)
                } else {
                    ConfigValue::String(n.to_string())
                }
            },
            serde_yaml::Value::String(s) => ConfigValue::String(s),
            serde_yaml::Value::Sequence(seq) => {
                ConfigValue::Array(seq.into_iter().map(|v| self.yaml_to_config_value(v)).collect())
            },
            serde_yaml::Value::Mapping(map) => {
                let mut result = HashMap::new();
                for (k, v) in map {
                    if let Some(key) = k.as_str() {
                        result.insert(key.to_string(), self.yaml_to_config_value(v));
                    }
                }
                ConfigValue::Object(result)
            },
            _ => ConfigValue::String(String::new()),
        }
    }
    
    fn deep_merge(&self, base: &mut HashMap<String, ConfigValue>, overlay: HashMap<String, ConfigValue>) {
        for (key, value) in overlay {
            match (base.get_mut(&key), value) {
                (Some(ConfigValue::Object(base_map)), ConfigValue::Object(overlay_map)) => {
                    // Recursive merge for nested objects
                    for (k, v) in overlay_map {
                        match base_map.get_mut(&k) {
                            Some(ConfigValue::Object(_)) if matches!(v, ConfigValue::Object(_)) => {
                                // Would need to make this properly recursive
                                base_map.insert(k, v);
                            },
                            _ => {
                                base_map.insert(k, v);
                            }
                        }
                    }
                },
                _ => {
                    base.insert(key, value);
                }
            }
        }
    }
    
    fn set_nested_value(&self, map: &mut HashMap<String, ConfigValue>, path: &str, value: ConfigValue) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = map;
        
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.insert(part.to_string(), value.clone());
            } else {
                let entry = current.entry(part.to_string())
                    .or_insert(ConfigValue::Object(HashMap::new()));
                if let ConfigValue::Object(inner_map) = entry {
                    current = inner_map;
                }
            }
        }
    }
    
    async fn resolve_all_values(&self, config: HashMap<String, ConfigValue>) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let mut resolved = HashMap::new();
        
        for (key, value) in config {
            let resolved_value = self.resolve_config_value(value).await?;
            resolved.insert(key, resolved_value);
        }
        
        Ok(resolved)
    }
    
    async fn resolve_config_value(&self, value: ConfigValue) -> Result<ConfigValue, ConfigError> {
        match value {
            ConfigValue::String(s) => {
                let resolved = self.resolve_string_value(&s).await?;
                Ok(ConfigValue::String(resolved))
            },
            ConfigValue::Array(arr) => {
                let mut resolved_arr = Vec::new();
                for item in arr {
                    resolved_arr.push(self.resolve_config_value(item).await?);
                }
                Ok(ConfigValue::Array(resolved_arr))
            },
            ConfigValue::Object(map) => {
                let mut resolved_map = HashMap::new();
                for (k, v) in map {
                    resolved_map.insert(k, self.resolve_config_value(v).await?);
                }
                Ok(ConfigValue::Object(resolved_map))
            },
            _ => Ok(value),
        }
    }
    
    async fn resolve_string_value(&self, value: &str) -> Result<String, ConfigError> {
        // First resolve environment variables
        let env_resolved = self.env_resolver.resolve_value(value)?;
        
        // Then resolve secrets
        let secret_pattern = Regex::new(r"\$\{secret:([^/]+)/([^}]+)\}").unwrap();
        let mut result = env_resolved.clone();
        
        for cap in secret_pattern.captures_iter(&env_resolved) {
            let full_match = &cap[0];
            let provider = &cap[1];
            let secret_path = &cap[2];
            
            if let Some(resolver) = self.secret_resolvers.get(provider) {
                let secret_value = resolver.resolve(secret_path).await?;
                result = result.replace(full_match, &secret_value);
            } else {
                return Err(ConfigError::SecretResolution(
                    format!("Unknown secret provider: {}", provider)
                ));
            }
        }
        
        Ok(result)
    }
}

// Component factory for dynamic initialization
pub struct ComponentFactory {
    registry: HashMap<String, Box<dyn ComponentBuilder>>,
    instances: Arc<RwLock<HashMap<String, Arc<dyn Component>>>>,
}

// Trait for components that can be dynamically created
#[async_trait]
pub trait Component: Send + Sync {
    fn name(&self) -> &str;
    async fn initialize(&mut self, config: HashMap<String, ConfigValue>) -> Result<(), ConfigError>;
    async fn start(&mut self) -> Result<(), ConfigError>;
    async fn stop(&mut self) -> Result<(), ConfigError>;
    async fn health_check(&self) -> Result<bool, ConfigError>;
}

// Trait for component builders
pub trait ComponentBuilder: Send + Sync {
    fn build(&self) -> Box<dyn Component>;
}

impl ComponentFactory {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            instances: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn register<T: ComponentBuilder + 'static>(&mut self, type_name: &str, builder: T) {
        self.registry.insert(type_name.to_string(), Box::new(builder));
    }
    
    pub async fn create_component(&self, config: &ComponentConfig) -> Result<Arc<dyn Component>, ConfigError> {
        let builder = self.registry.get(&config.component_type)
            .ok_or_else(|| ConfigError::ComponentNotFound(config.component_type.clone()))?;
            
        let mut component = builder.build();
        component.initialize(config.config.clone()).await?;
        
        let arc_component = Arc::from(component);
        
        {
            let mut instances = self.instances.write().unwrap();
            instances.insert(config.name.clone(), arc_component.clone());
        }
        
        Ok(arc_component)
    }
    
    pub async fn create_all(&self, configs: Vec<ComponentConfig>) -> Result<(), ConfigError> {
        // Sort by startup order
        let mut sorted_configs = configs;
        sorted_configs.sort_by_key(|c| c.lifecycle.as_ref().map(|l| l.startup_order).unwrap_or(999));
        
        // Check for circular dependencies
        self.check_circular_dependencies(&sorted_configs)?;
        
        // Create components in order
        for config in sorted_configs {
            // Wait for dependencies
            for dep in &config.dependencies {
                self.wait_for_component(dep).await?;
            }
            
            // Create component
            let component = self.create_component(&config).await?;
            component.start().await?;
        }
        
        Ok(())
    }
    
    fn check_circular_dependencies(&self, configs: &[ComponentConfig]) -> Result<(), ConfigError> {
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        
        for config in configs {
            graph.insert(config.name.clone(), config.dependencies.iter().cloned().collect());
        }
        
        // DFS to detect cycles
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for node in graph.keys() {
            if !visited.contains(node) {
                if self.has_cycle(&graph, node, &mut visited, &mut rec_stack)? {
                    return Err(ConfigError::CircularDependency(
                        format!("Circular dependency detected involving component: {}", node)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    fn has_cycle(
        &self,
        graph: &HashMap<String, HashSet<String>>,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> Result<bool, ConfigError> {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        
        if let Some(dependencies) = graph.get(node) {
            for dep in dependencies {
                if !visited.contains(dep) {
                    if self.has_cycle(graph, dep, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(dep) {
                    return Ok(true);
                }
            }
        }
        
        rec_stack.remove(node);
        Ok(false)
    }
    
    async fn wait_for_component(&self, name: &str) -> Result<(), ConfigError> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(30);
        
        loop {
            {
                let instances = self.instances.read().unwrap();
                if instances.contains_key(name) {
                    return Ok(());
                }
            }
            
            if start.elapsed() > timeout {
                return Err(ConfigError::ComponentNotFound(
                    format!("Timeout waiting for component: {}", name)
                ));
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

// Configuration watcher for hot reloading
pub struct ConfigWatcher {
    loader: Arc<ConfigLoader>,
    config: Arc<RwLock<Configuration>>,
    update_channel: broadcast::Sender<Configuration>,
    watch_interval: Duration,
}

impl ConfigWatcher {
    pub fn new(loader: Arc<ConfigLoader>, watch_interval: Duration) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            loader,
            config: Arc::new(RwLock::new(Configuration {
                data: HashMap::new(),
                metadata: ConfigMetadata::default(),
            })),
            update_channel: tx,
            watch_interval,
        }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<Configuration> {
        self.update_channel.subscribe()
    }
    
    pub async fn start(&self) {
        let loader = self.loader.clone();
        let config = self.config.clone();
        let tx = self.update_channel.clone();
        let interval = self.watch_interval;
        
        tokio::spawn(async move {
            let mut last_version = String::new();
            
            loop {
                match loader.load().await {
                    Ok(new_config) => {
                        if new_config.metadata.version != last_version {
                            last_version = new_config.metadata.version.clone();
                            
                            {
                                let mut current = config.write().unwrap();
                                *current = new_config.clone();
                            }
                            
                            let _ = tx.send(new_config);
                        }
                    },
                    Err(e) => {
                        eprintln!("Error reloading configuration: {}", e);
                    }
                }
                
                tokio::time::sleep(interval).await;
            }
        });
    }
    
    pub fn get_current(&self) -> Configuration {
        self.config.read().unwrap().clone()
    }
}

// Example usage
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_config_loading() {
        let mut loader = ConfigLoader::new();
        
        // Add file source
        loader.add_source(ConfigSource::File {
            paths: vec!["config/base.yaml".to_string()],
        });
        
        // Add environment source
        loader.add_source(ConfigSource::Environment {
            prefix: "APP_".to_string(),
        });
        
        // Load configuration
        let config = loader.load().await.unwrap();
        
        println!("Loaded configuration: {:?}", config);
    }
    
    #[tokio::test]
    async fn test_component_factory() {
        // Example component implementation
        struct ExampleComponent {
            name: String,
            config: HashMap<String, ConfigValue>,
        }
        
        #[async_trait]
        impl Component for ExampleComponent {
            fn name(&self) -> &str {
                &self.name
            }
            
            async fn initialize(&mut self, config: HashMap<String, ConfigValue>) -> Result<(), ConfigError> {
                self.config = config;
                Ok(())
            }
            
            async fn start(&mut self) -> Result<(), ConfigError> {
                println!("Starting component: {}", self.name);
                Ok(())
            }
            
            async fn stop(&mut self) -> Result<(), ConfigError> {
                println!("Stopping component: {}", self.name);
                Ok(())
            }
            
            async fn health_check(&self) -> Result<bool, ConfigError> {
                Ok(true)
            }
        }
        
        struct ExampleBuilder;
        
        impl ComponentBuilder for ExampleBuilder {
            fn build(&self) -> Box<dyn Component> {
                Box::new(ExampleComponent {
                    name: "example".to_string(),
                    config: HashMap::new(),
                })
            }
        }
        
        let mut factory = ComponentFactory::new();
        factory.register("example", ExampleBuilder);
        
        let component_config = ComponentConfig {
            name: "test-component".to_string(),
            component_type: "example".to_string(),
            config: HashMap::new(),
            dependencies: vec![],
            lifecycle: None,
        };
        
        let component = factory.create_component(&component_config).await.unwrap();
        assert_eq!(component.name(), "example");
    }
}