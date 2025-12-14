# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-12-14

### Added
- Initial release of config-client
- Core `ConfigClient` with etcd connectivity
- Type-safe configuration with serde support
- Key prefix support for multi-tenant deployments
- Environment variable override mechanism
- Configuration watching with callbacks
- Comprehensive error handling with `ConfigError`
- `WatchHandle` for cancellable configuration watches
- Support for JSON and YAML serialization
- Examples and integration tests
- Full API documentation

### Features
- `ConfigClient::new()` - Connect to etcd
- `ConfigClient::with_prefix()` - Connect with key prefix
- `ConfigClient::get()` - Get typed configuration
- `ConfigClient::set()` - Set configuration
- `ConfigClient::delete()` - Delete configuration
- `ConfigClient::list()` - List keys under prefix
- `ConfigClient::watch()` - Watch for configuration changes
- `ConfigClient::get_with_env()` - Environment variable overrides
- `ConfigClient::get_raw()` / `set_raw()` - Raw JSON operations

### Documentation
- README with usage examples
- QUICK_START guide
- API documentation with examples
- Integration test suite
- Basic example demonstrating all features

[0.1.0]: https://github.com/yourorg/config-client/releases/tag/v0.1.0
