//! Architecture Tests for Neural Trader Clean Architecture
//!
//! These tests validate architectural constraints and design principles:
//! - Module size limits (<500 lines per module)
//! - Code complexity metrics
//! - Dependency structure validation
//! - API contract compliance
//! - Documentation completeness

use std::path::Path;
use anyhow::Result;

mod helpers;
use helpers::{discover_rust_modules, count_lines_in_file};

/// Test that all modules respect the 500-line limit
#[test]
fn test_module_size_constraints() -> Result<()> {
    let src_modules = discover_rust_modules("src/")?;
    let test_modules = discover_rust_modules("tests/")?;
    
    let mut all_modules = src_modules;
    all_modules.extend(test_modules);
    
    println!("🏗️  Validating module size constraints...");
    
    let mut violations = Vec::new();
    let max_lines = 500;
    
    for module_path in &all_modules {
        let line_count = count_lines_in_file(module_path)?;
        
        if line_count > max_lines {
            violations.push((module_path.clone(), line_count));
        }
        
        // Report all modules for visibility
        println!("   {}: {} lines", module_path, line_count);
    }
    
    // Report violations
    if !violations.is_empty() {
        println!("\n❌ Module size violations found:");
        for (module, lines) in &violations {
            println!("   {} has {} lines (max {})", module, lines, max_lines);
        }
        
        return Err(anyhow::anyhow!(
            "{} modules exceed {} line limit",
            violations.len(),
            max_lines
        ));
    }
    
    println!("   ✅ All {} modules respect the 500-line limit", all_modules.len());
    println!("✅ Module size constraints test passed");
    Ok(())
}

/// Test that critical modules are appropriately sized
#[test]
fn test_critical_module_sizing() -> Result<()> {
    let critical_modules = vec![
        ("src/neural/predictor.rs", 300), // Main predictor should be concise
        ("src/adapters/enhanced_neural_adapter.rs", 500), // Complex but bounded
        ("src/neural/fann_predictor.rs", 400), // Core functionality
        ("src/main.rs", 200), // Entry point should be simple
    ];
    
    println!("🎯 Validating critical module sizing...");
    
    for (module_path, max_lines) in critical_modules {
        if Path::new(module_path).exists() {
            let line_count = count_lines_in_file(module_path)?;
            
            assert!(
                line_count <= max_lines,
                "Critical module {} has {} lines (max {} expected)",
                module_path,
                line_count,
                max_lines
            );
            
            println!("   ✅ {}: {} lines (max {})", module_path, line_count, max_lines);
        } else {
            println!("   ⚠️  {} not found (may have been refactored)", module_path);
        }
    }
    
    println!("✅ Critical module sizing test passed");
    Ok(())
}

/// Test module complexity metrics
#[test]
fn test_module_complexity() -> Result<()> {
    let modules = discover_rust_modules("src/")?;
    
    println!("🧮 Analyzing module complexity...");
    
    for module_path in &modules {
        let content = std::fs::read_to_string(module_path)?;
        
        // Count functions
        let function_count = content.matches("fn ").count();
        
        // Count impl blocks
        let impl_count = content.matches("impl ").count();
        
        // Count struct/enum definitions
        let struct_count = content.matches("struct ").count();
        let enum_count = content.matches("enum ").count();
        
        // Count dependencies (use statements)
        let use_count = content.lines()
            .filter(|line| line.trim_start().starts_with("use "))
            .count();
        
        // Calculate complexity score
        let complexity_score = function_count + (impl_count * 2) + struct_count + enum_count + (use_count / 5);
        
        println!("   {}: complexity score {}", module_path, complexity_score);
        println!("     Functions: {}, Impls: {}, Structs: {}, Enums: {}, Uses: {}", 
                function_count, impl_count, struct_count, enum_count, use_count);
        
        // Warn if complexity is very high
        if complexity_score > 100 {
            println!("     ⚠️  High complexity score: {}", complexity_score);
        }
        
        // Check for very long functions (rough estimate)
        let long_functions = content.split("fn ")
            .skip(1) // Skip content before first function
            .filter(|func_content| {
                // Count lines in function (rough estimate)
                let brace_depth = func_content.chars()
                    .scan(0, |depth, c| {
                        match c {
                            '{' => *depth += 1,
                            '}' => *depth -= 1,
                            _ => {}
                        }
                        Some(*depth)
                    })
                    .take_while(|&depth| depth > 0)
                    .count();
                
                brace_depth > 50 // Rough estimate for long functions
            })
            .count();
        
        if long_functions > 0 {
            println!("     ⚠️  {} potentially long functions detected", long_functions);
        }
    }
    
    println!("✅ Module complexity analysis completed");
    Ok(())
}

/// Test dependency structure and layering
#[test]
fn test_dependency_structure() -> Result<()> {
    println!("📦 Validating dependency structure...");
    
    let module_categories = vec![
        ("src/main.rs", "entry"),
        ("src/lib.rs", "library"),
        ("src/config/", "config"),
        ("src/data/", "data"),
        ("src/neural/", "neural"),
        ("src/adapters/", "adapters"),
        ("src/integration/", "integration"),
    ];
    
    // Validate that modules follow proper layering
    for (path_pattern, category) in module_categories {
        if Path::new(path_pattern).exists() {
            println!("   ✅ {} layer exists", category);
            
            // Additional checks for specific layers
            match category {
                "neural" => {
                    // Neural layer should not depend on integration layer
                    let neural_modules = discover_rust_modules("src/neural/")?;
                    for module in neural_modules {
                        let content = std::fs::read_to_string(&module)?;
                        
                        // Check for inappropriate dependencies
                        if content.contains("use crate::integration::") {
                            println!("     ⚠️  {} depends on integration layer", module);
                        }
                    }
                }
                "adapters" => {
                    // Adapters should be independent of each other
                    let adapter_modules = discover_rust_modules("src/adapters/")?;
                    for module in adapter_modules {
                        let content = std::fs::read_to_string(&module)?;
                        
                        // Check for cross-adapter dependencies
                        if content.contains("use crate::adapters::") && 
                           !content.contains("use crate::adapters::errors") &&
                           !content.contains("use super::") {
                            println!("     ⚠️  {} may have cross-adapter dependency", module);
                        }
                    }
                }
                _ => {}
            }
        } else {
            println!("   ⚠️  {} layer not found at {}", category, path_pattern);
        }
    }
    
    println!("✅ Dependency structure validation completed");
    Ok(())
}

/// Test API contract consistency
#[test]
fn test_api_contract_consistency() -> Result<()> {
    println!("🔗 Validating API contract consistency...");
    
    // Check that key traits are consistently implemented
    let key_files = vec![
        "src/neural/predictor.rs",
        "src/adapters/enhanced_neural_adapter.rs",
        "src/neural/fann_predictor.rs",
    ];
    
    for file_path in key_files {
        if Path::new(file_path).exists() {
            let content = std::fs::read_to_string(file_path)?;
            
            // Check for NeuralPredictorTrait implementation
            let has_trait_impl = content.contains("impl NeuralPredictorTrait") ||
                                content.contains("NeuralPredictorTrait for");
            
            // Check for async trait usage
            let has_async_trait = content.contains("#[async_trait]");
            
            // Check for proper error handling
            let has_error_handling = content.contains("Result<") && content.contains("anyhow::");
            
            println!("   {} API consistency:", file_path);
            println!("     NeuralPredictorTrait: {}", if has_trait_impl { "✅" } else { "❌" });
            println!("     Async trait: {}", if has_async_trait { "✅" } else { "⚠️" });
            println!("     Error handling: {}", if has_error_handling { "✅" } else { "❌" });
            
            // Specific validations for key methods
            let required_methods = vec!["predict", "predict_ensemble", "get_feature_importance"];
            for method in required_methods {
                if content.contains(&format!("async fn {}", method)) {
                    println!("     {} method: ✅", method);
                } else {
                    println!("     {} method: ❌", method);
                }
            }
        }
    }
    
    println!("✅ API contract consistency validation completed");
    Ok(())
}

/// Test documentation completeness
#[test]
fn test_documentation_completeness() -> Result<()> {
    println!("📚 Validating documentation completeness...");
    
    let src_modules = discover_rust_modules("src/")?;
    
    let mut doc_stats = Vec::new();
    
    for module_path in &src_modules {
        let content = std::fs::read_to_string(module_path)?;
        
        // Count module-level docs
        let has_module_doc = content.starts_with("//!") || content.contains("\n//!");
        
        // Count function docs
        let function_lines: Vec<&str> = content.lines().collect();
        let mut documented_functions = 0;
        let mut total_functions = 0;
        
        for (i, line) in function_lines.iter().enumerate() {
            if line.trim_start().starts_with("pub fn ") || 
               line.trim_start().starts_with("async fn ") ||
               (line.trim_start().starts_with("fn ") && !line.contains("test")) {
                total_functions += 1;
                
                // Check if previous lines contain documentation
                if i > 0 {
                    let prev_line = function_lines[i - 1];
                    if prev_line.trim_start().starts_with("///") {
                        documented_functions += 1;
                    }
                }
            }
        }
        
        // Count struct/enum docs
        let struct_enum_count = content.matches("pub struct ").count() + content.matches("pub enum ").count();
        let documented_types = content.matches("/// ").count();
        
        let doc_coverage = if total_functions > 0 {
            (documented_functions as f64 / total_functions as f64) * 100.0
        } else {
            100.0
        };
        
        doc_stats.push((
            module_path.clone(),
            has_module_doc,
            total_functions,
            documented_functions,
            struct_enum_count,
            documented_types,
            doc_coverage,
        ));
    }
    
    // Report documentation statistics
    let mut total_functions = 0;
    let mut total_documented = 0;
    let mut modules_with_doc = 0;
    
    for (module, has_module_doc, functions, documented, types, doc_comments, coverage) in &doc_stats {
        println!("   {}: {:.1}% function coverage", module, coverage);
        
        if *has_module_doc {
            modules_with_doc += 1;
            println!("     ✅ Module documentation");
        } else {
            println!("     ❌ Missing module documentation");
        }
        
        if *functions > 0 {
            println!("     Functions: {}/{} documented", documented, functions);
        }
        
        if *types > 0 {
            println!("     Types: {}, Doc comments: {}", types, doc_comments);
        }
        
        total_functions += functions;
        total_documented += documented;
    }
    
    let overall_coverage = if total_functions > 0 {
        (total_documented as f64 / total_functions as f64) * 100.0
    } else {
        100.0
    };
    
    println!("\n📊 Documentation Summary:");
    println!("   Overall function coverage: {:.1}% ({}/{})", overall_coverage, total_documented, total_functions);
    println!("   Modules with documentation: {}/{}", modules_with_doc, doc_stats.len());
    
    // Validate minimum documentation standards
    assert!(
        overall_coverage >= 60.0,
        "Documentation coverage {:.1}% below minimum 60%",
        overall_coverage
    );
    
    let module_doc_percentage = (modules_with_doc as f64 / doc_stats.len() as f64) * 100.0;
    assert!(
        module_doc_percentage >= 80.0,
        "Module documentation coverage {:.1}% below minimum 80%",
        module_doc_percentage
    );
    
    println!("✅ Documentation completeness validation passed");
    Ok(())
}

/// Test code style consistency
#[test]
fn test_code_style_consistency() -> Result<()> {
    println!("🎨 Validating code style consistency...");
    
    let modules = discover_rust_modules("src/")?;
    let mut style_issues = Vec::new();
    
    for module_path in &modules {
        let content = std::fs::read_to_string(module_path)?;
        let lines: Vec<&str> = content.lines().collect();
        
        // Check for consistent error handling patterns
        let has_anyhow = content.contains("anyhow::");
        let has_std_error = content.contains("std::error::");
        
        if has_anyhow && has_std_error {
            style_issues.push(format!("{}: Mixed error handling patterns", module_path));
        }
        
        // Check for consistent async patterns
        let async_functions = content.matches("async fn ").count();
        let sync_returns = content.matches("-> Result<").count();
        
        if async_functions > 0 {
            // Should use async Result patterns
            let async_results = content.matches("-> anyhow::Result<").count() + 
                              content.matches("-> Result<").count();
            
            println!("   {}: {} async functions, {} result types", 
                    module_path, async_functions, async_results);
        }
        
        // Check for consistent naming patterns
        let mut long_function_names = 0;
        for line in &lines {
            if line.trim_start().starts_with("pub fn ") || line.trim_start().starts_with("fn ") {
                if let Some(fn_name_part) = line.split('(').next() {
                    if let Some(fn_name) = fn_name_part.split_whitespace().last() {
                        if fn_name.len() > 40 {
                            long_function_names += 1;
                        }
                    }
                }
            }
        }
        
        if long_function_names > 0 {
            style_issues.push(format!("{}: {} functions with very long names", module_path, long_function_names));
        }
        
        // Check for consistent import organization
        let use_statements: Vec<&str> = lines.iter()
            .filter(|line| line.trim_start().starts_with("use "))
            .cloned()
            .collect();
        
        if use_statements.len() > 1 {
            // Check if use statements are grouped (std, external, crate)
            let mut std_uses = false;
            let mut external_uses = false;
            let mut crate_uses = false;
            
            for use_stmt in &use_statements {
                if use_stmt.contains("use std::") {
                    std_uses = true;
                } else if use_stmt.contains("use crate::") || use_stmt.contains("use super::") {
                    crate_uses = true;
                } else {
                    external_uses = true;
                }
            }
            
            println!("   {}: std: {}, external: {}, crate: {} imports", 
                    module_path, std_uses, external_uses, crate_uses);
        }
    }
    
    // Report style issues
    if !style_issues.is_empty() {
        println!("\n⚠️  Style consistency issues:");
        for issue in &style_issues {
            println!("   {}", issue);
        }
    }
    
    println!("   Analyzed {} modules for style consistency", modules.len());
    println!("   Found {} potential style issues", style_issues.len());
    
    println!("✅ Code style consistency validation completed");
    Ok(())
}

/// Test architectural principles compliance
#[test]
fn test_architectural_principles() -> Result<()> {
    println!("🏛️  Validating architectural principles...");
    
    // Principle 1: Single Responsibility - modules should have focused purpose
    let neural_modules = discover_rust_modules("src/neural/")?;
    for module in &neural_modules {
        let content = std::fs::read_to_string(module)?;
        
        // Count different types of functionality
        let prediction_functions = content.matches("predict").count();
        let training_functions = content.matches("train").count();
        let config_functions = content.matches("config").count();
        let health_functions = content.matches("health").count();
        
        let functionality_types = [prediction_functions, training_functions, config_functions, health_functions]
            .iter()
            .filter(|&&count| count > 0)
            .count();
        
        if functionality_types > 3 {
            println!("   ⚠️  {} may violate single responsibility (has {} types of functionality)", 
                    module, functionality_types);
        }
    }
    
    // Principle 2: Dependency Inversion - high-level modules shouldn't depend on low-level modules
    let high_level_modules = vec!["src/neural/predictor.rs", "src/main.rs"];
    for module_path in high_level_modules {
        if Path::new(module_path).exists() {
            let content = std::fs::read_to_string(module_path)?;
            
            // Check for direct dependencies on implementation details
            if content.contains("use ruv_fann::") {
                println!("   ⚠️  {} directly depends on low-level implementation", module_path);
            }
            
            // Should depend on abstractions (traits)
            if content.contains("NeuralPredictorTrait") {
                println!("   ✅ {} depends on abstractions", module_path);
            }
        }
    }
    
    // Principle 3: Interface Segregation - clients shouldn't depend on interfaces they don't use
    let adapter_modules = discover_rust_modules("src/adapters/")?;
    for module in &adapter_modules {
        let content = std::fs::read_to_string(module)?;
        
        // Check for large interfaces
        if content.contains("trait ") {
            let trait_methods = content.matches("fn ").count();
            if trait_methods > 10 {
                println!("   ⚠️  {} may have interface with too many methods ({})", module, trait_methods);
            }
        }
    }
    
    // Principle 4: Open/Closed - modules should be open for extension, closed for modification
    let core_modules = vec!["src/neural/predictor.rs", "src/adapters/enhanced_neural_adapter.rs"];
    for module_path in core_modules {
        if Path::new(module_path).exists() {
            let content = std::fs::read_to_string(module_path)?;
            
            // Check for extensibility patterns
            let has_traits = content.contains("trait ") || content.contains("impl ");
            let has_generics = content.contains("<T") || content.contains("<'");
            let has_composition = content.contains("Arc<") || content.contains("Box<");
            
            if has_traits || has_generics || has_composition {
                println!("   ✅ {} designed for extensibility", module_path);
            } else {
                println!("   ⚠️  {} may be difficult to extend", module_path);
            }
        }
    }
    
    println!("✅ Architectural principles validation completed");
    Ok(())
}