// Phase 3 Production Validator Binary
// ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS
//
// This binary orchestrates all production validation checks and provides
// command-line interface for the validation framework

use clap::{Arg, Command, ValueEnum};
use std::path::PathBuf;
use std::time::Instant;
use autonomous_platform::orchestrator::validators::{
    ValidationOrchestrator, 
    ValidationMode,
    report_generator::{ReportGenerator, ReportFormat},
};

/// Command line arguments for the production validator
#[derive(Debug, Clone)]
struct ValidatorArgs {
    pub validator: String,
    pub mode: ValidationMode,
    pub report_format: ReportFormat,
    pub output_dir: PathBuf,
    pub fail_fast: bool,
    pub verbose: bool,
    pub dry_run: bool,
}

/// Available validator types
#[derive(Debug, Clone, ValueEnum)]
enum ValidatorType {
    #[value(name = "code-completeness")]
    CodeCompleteness,
    #[value(name = "interface-contract")]
    InterfaceContract,
    #[value(name = "test-coverage")]
    TestCoverage,
    #[value(name = "performance-benchmark")]
    PerformanceBenchmark,
    #[value(name = "security-standards")]
    SecurityStandards,
    #[value(name = "all")]
    All,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("production_validator=info")
        .init();

    let app = Command::new("production-validator")
        .version("1.0.0")
        .author("Neural Trader Team")
        .about("Phase 3 Production Validation Framework - ZERO TOLERANCE for incomplete implementations")
        .long_about("
🚀 Phase 3 Production Validation Framework

This tool enforces ZERO TOLERANCE for incomplete implementations by running
comprehensive validation checks across all Phase 3 components:

• Code Completeness - Detects TODOs, stubs, and incomplete functions
• Interface Contracts - Validates gRPC services and Redis Streams
• Test Coverage - Enforces 95% minimum coverage requirement
• Performance Benchmarks - Validates SLA compliance
• Security Standards - OWASP and NIST compliance checks

USAGE EXAMPLES:
  production-validator --validator=all --mode=production
  production-validator --validator=code-completeness --verbose
  production-validator --validator=test-coverage --report=html --output=./reports
        ")
        .arg(
            Arg::new("validator")
                .short('v')
                .long("validator")
                .help("Validator to run")
                .value_parser(clap::value_parser!(ValidatorType))
                .required(true)
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .help("Validation mode")
                .value_parser(["development", "staging", "production"])
                .default_value("development")
        )
        .arg(
            Arg::new("report")
                .short('r')
                .long("report")
                .help("Report format")
                .value_parser(["console", "json", "html", "markdown", "csv", "xml"])
                .default_value("console")
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("Output directory for reports")
                .value_parser(clap::value_parser!(PathBuf))
                .default_value("./target/validation-results")
        )
        .arg(
            Arg::new("fail-fast")
                .long("fail-fast")
                .help("Stop on first validation failure")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Show what would be executed without running")
                .action(clap::ArgAction::SetTrue)
        );

    let matches = app.get_matches();

    // Parse arguments
    let args = ValidatorArgs {
        validator: match matches.get_one::<ValidatorType>("validator").unwrap() {
            ValidatorType::CodeCompleteness => "code-completeness".to_string(),
            ValidatorType::InterfaceContract => "interface-contract".to_string(),
            ValidatorType::TestCoverage => "test-coverage".to_string(),
            ValidatorType::PerformanceBenchmark => "performance-benchmark".to_string(),
            ValidatorType::SecurityStandards => "security-standards".to_string(),
            ValidatorType::All => "all".to_string(),
        },
        mode: match matches.get_one::<String>("mode").unwrap().as_str() {
            "development" => ValidationMode::Development,
            "staging" => ValidationMode::Staging,
            "production" => ValidationMode::Production,
            _ => ValidationMode::Development,
        },
        report_format: match matches.get_one::<String>("report").unwrap().as_str() {
            "json" => ReportFormat::Json,
            "html" => ReportFormat::Html,
            "markdown" => ReportFormat::Markdown,
            "csv" => ReportFormat::Csv,
            "xml" => ReportFormat::Xml,
            _ => ReportFormat::Console,
        },
        output_dir: matches.get_one::<PathBuf>("output").unwrap().clone(),
        fail_fast: matches.get_flag("fail-fast"),
        verbose: matches.get_flag("verbose"),
        dry_run: matches.get_flag("dry-run"),
    };

    if args.verbose {
        println!("🔍 Phase 3 Production Validator Starting...");
        println!("   Validator: {}", args.validator);
        println!("   Mode: {:?}", args.mode);
        println!("   Report Format: {:?}", args.report_format);
        println!("   Output Directory: {}", args.output_dir.display());
        println!("   Fail Fast: {}", args.fail_fast);
        println!("   Dry Run: {}", args.dry_run);
        println!();
    }

    if args.dry_run {
        println!("🔍 DRY RUN - Would execute the following validation:");
        println!("   Validator: {}", args.validator);
        println!("   Mode: {:?}", args.mode);
        println!("   Output: {}", args.output_dir.display());
        println!("✅ Dry run completed successfully");
        return Ok(());
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output_dir)?;

    // Initialize orchestrator
    let mut orchestrator = ValidationOrchestrator::new();
    orchestrator.set_verbose(args.verbose);

    // Print banner
    print_banner();

    // Record start time
    let start_time = Instant::now();

    // Execute validation
    let results = if args.validator == "all" {
        println!("🔍 Running ALL production validators (ZERO TOLERANCE mode)...\n");
        
        if args.fail_fast {
            orchestrator.validate_all_fail_fast(&args.mode).await?
        } else {
            orchestrator.validate_all(&args.mode).await?
        }
    } else {
        println!("🔍 Running {} validator...\n", args.validator);
        orchestrator.validate_single(&args.validator, &args.mode).await?
    };

    let execution_time = start_time.elapsed();
    
    // Generate report
    let generator = ReportGenerator::new();
    let report = generator.generate_report(
        results,
        &format!("{:?}", args.mode).to_lowercase(),
        execution_time.as_millis() as u64,
    )?;

    // Print console output
    if matches!(args.report_format, ReportFormat::Console) {
        println!("{}", generator.format_console(&report));
    }

    // Write report to file if not console format
    if !matches!(args.report_format, ReportFormat::Console) {
        let file_extension = match args.report_format {
            ReportFormat::Json => "json",
            ReportFormat::Html => "html",
            ReportFormat::Markdown => "md",
            ReportFormat::Csv => "csv",
            ReportFormat::Xml => "xml",
            _ => "txt",
        };
        
        let report_file = args.output_dir.join(format!("validation-report.{}", file_extension));
        generator.write_report(&report, args.report_format.clone(), &report_file)?;
        
        println!("📊 Report saved to: {}", report_file.display());
    }

    // Also save JSON version for programmatic access
    if !matches!(args.report_format, ReportFormat::Json) {
        let json_file = args.output_dir.join("validation-results.json");
        generator.write_report(&report, ReportFormat::Json, &json_file)?;
        
        if args.verbose {
            println!("📋 JSON report saved to: {}", json_file.display());
        }
    }

    // Print final status
    println!("\n{}", "=".repeat(80));
    if report.deployment_decision.approved {
        println!("🚀 VALIDATION PASSED - DEPLOYMENT APPROVED");
        if matches!(args.mode, ValidationMode::Production) {
            println!("✅ Code is production-ready and meets all quality gates");
        }
    } else {
        println!("🚨 VALIDATION FAILED - DEPLOYMENT BLOCKED");
        println!("❌ Reason: {}", report.deployment_decision.reason);
        
        if !report.deployment_decision.required_actions.is_empty() {
            println!("\n🔧 Required Actions:");
            for action in &report.deployment_decision.required_actions {
                println!("   • {}", action);
            }
        }
    }
    
    println!("⏱️  Total execution time: {}ms", execution_time.as_millis());
    println!("📊 Overall score: {:.1}%", report.summary.overall_score);
    println!("{}", "=".repeat(80));

    // Exit with appropriate code
    if report.deployment_decision.approved {
        Ok(())
    } else {
        std::process::exit(1)
    }
}

fn print_banner() {
    println!("{}", "=".repeat(80));
    println!("  🚀 PHASE 3 PRODUCTION VALIDATION FRAMEWORK");
    println!("  🚨 ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS");
    println!("{}", "=".repeat(80));
    println!();
}

// Async main wrapper for tokio
#[tokio::main]
async fn main_async() -> Result<(), Box<dyn std::error::Error>> {
    main().await
}

// We need to have a sync main that calls the async main
fn actual_main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Runtime::new()?.block_on(main())
}

// Override main to be sync
#[allow(dead_code)]
fn sync_main() -> Result<(), Box<dyn std::error::Error>> {
    actual_main()
}