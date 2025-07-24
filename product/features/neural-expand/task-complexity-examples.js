/**
 * Task Complexity Analyzer Examples
 * 
 * Demonstrates how to use the task complexity analyzer for various
 * trading system tasks in the neural-trader project.
 */

const { TaskComplexityAnalyzer, analyzeTask } = require('./task-complexity-analyzer');

/**
 * Example task analyses for common neural-trader scenarios
 */
async function runExamples() {
    console.log('Task Complexity Analysis Examples for Neural-Trader\n');
    console.log('=' .repeat(60));
    
    const analyzer = new TaskComplexityAnalyzer();
    
    // Example 1: Simple configuration task
    console.log('\n1. Simple Task: Configuration Update');
    console.log('-'.repeat(40));
    const configTask = await analyzer.analyzeTaskComplexity({
        description: "Update trading strategy parameters in config file",
        components: ['config.rs'],
        requirements: ['parameter validation']
    });
    displayResult(configTask);
    
    // Example 2: Medium complexity API integration
    console.log('\n2. Medium Task: API Integration');
    console.log('-'.repeat(40));
    const apiTask = await analyzer.analyzeTaskComplexity({
        description: "Integrate new market data provider API with error handling and rate limiting",
        components: ['providers', 'rate_limiter', 'error_handler'],
        requirements: ['authentication', 'retry logic', 'data normalization']
    });
    displayResult(apiTask);
    
    // Example 3: Complex neural network task
    console.log('\n3. Complex Task: Neural Network Enhancement');
    console.log('-'.repeat(40));
    const neuralTask = await analyzer.analyzeTaskComplexity({
        description: "Implement ensemble neural network prediction with real-time streaming data integration and distributed processing",
        components: ['neural', 'streaming', 'distributed', 'prediction_cache'],
        existingCode: true,
        requirements: ['low latency', 'high throughput', 'fault tolerance', 'scalability']
    });
    displayResult(neuralTask);
    
    // Example 4: Trading strategy implementation
    console.log('\n4. Trading Strategy Implementation');
    console.log('-'.repeat(40));
    const strategyTask = await analyzer.analyzeTaskComplexity({
        description: "Implement momentum-based trading strategy with risk management and portfolio optimization",
        components: ['strategies', 'risk_manager', 'portfolio'],
        requirements: ['backtesting', 'performance metrics', 'position sizing']
    });
    displayResult(strategyTask);
    
    // Example 5: Real-time data pipeline
    console.log('\n5. Real-time Data Pipeline');
    console.log('-'.repeat(40));
    const pipelineTask = await analyzer.analyzeTaskComplexity({
        description: "Build real-time websocket data pipeline with transformation, validation, and storage to TimescaleDB",
        components: ['websocket', 'transformer', 'validator', 'timescale_adapter'],
        requirements: ['streaming', 'data quality', 'persistence', 'monitoring']
    });
    displayResult(pipelineTask);
    
    // Batch analysis example
    console.log('\n\nBatch Analysis Example');
    console.log('='.repeat(60));
    
    const batchTasks = [
        {
            description: "Add logging to trading execution module",
            components: ['trading_executor']
        },
        {
            description: "Implement distributed agent coordination with fault tolerance",
            components: ['agents', 'coordinator', 'messaging']
        },
        {
            description: "Create performance monitoring dashboard",
            components: ['monitoring', 'metrics', 'visualization']
        }
    ];
    
    console.log('\nAnalyzing batch of', batchTasks.length, 'tasks...\n');
    
    for (let i = 0; i < batchTasks.length; i++) {
        const result = await analyzer.analyzeTaskComplexity(batchTasks[i]);
        console.log(`Task ${i + 1}: ${batchTasks[i].description}`);
        console.log(`Complexity: ${result.complexity.category} (${result.complexity.score})`);
        console.log(`Recommended agents: ${result.agentAllocation.totalAgents}`);
        console.log(`Estimated duration: ${result.estimatedDuration.hours} hours\n`);
    }
    
    // Trading-specific pattern analysis
    console.log('\nTrading Pattern Recognition Examples');
    console.log('='.repeat(60));
    
    const tradingPatterns = [
        "Implement high-frequency trading algorithm with sub-millisecond latency",
        "Add simple stop-loss feature to existing strategy",
        "Create comprehensive risk management system with VaR calculations",
        "Update documentation for trading API endpoints",
        "Integrate machine learning model for price prediction",
        "Build scalable microservice architecture for order execution"
    ];
    
    for (const pattern of tradingPatterns) {
        const result = await analyzeTask(pattern);
        const patternInfo = analyzer.identifyTaskPattern(pattern);
        console.log(`\nPattern: "${pattern}"`);
        console.log(`Identified as: ${patternInfo ? patternInfo.name : 'custom task'}`);
        console.log(`Complexity: ${result.complexity.category} (${result.complexity.score})`);
    }
}

/**
 * Display analysis result in formatted output
 */
function displayResult(result) {
    console.log(`\nComplexity Score: ${result.complexity.score}/10 (${result.complexity.category})`);
    console.log(`Confidence: ${(result.complexity.confidence * 100).toFixed(1)}%`);
    
    console.log('\nComplexity Factors:');
    Object.entries(result.factors).forEach(([factor, data]) => {
        console.log(`  - ${factor}: ${data.score}/${getMaxScore(factor)}`);
        if (data.details.length > 0) {
            data.details.forEach(detail => console.log(`    • ${detail}`));
        }
    });
    
    console.log('\nAgent Allocation:');
    console.log(`  Total agents needed: ${result.agentAllocation.totalAgents}`);
    console.log(`  Topology: ${result.agentAllocation.topology}`);
    result.agentAllocation.distribution.forEach(agent => {
        const spec = agent.specialization ? ` (${agent.specialization})` : '';
        console.log(`  - ${agent.count} ${agent.type}${spec}`);
    });
    
    console.log(`\nEstimated Duration: ${result.estimatedDuration.hours} hours (${result.estimatedDuration.range})`);
    
    if (result.recommendations.length > 0) {
        console.log('\nRecommendations:');
        result.recommendations.forEach(rec => console.log(`  • ${rec}`));
    }
    
    if (result.riskAssessment.length > 0) {
        console.log('\nRisk Assessment:');
        result.riskAssessment.forEach(risk => {
            console.log(`  • ${risk.type} (${risk.level}): ${risk.description}`);
        });
    }
}

/**
 * Get maximum score for a factor
 */
function getMaxScore(factor) {
    const maxScores = {
        dependencies: 10,
        integrationPoints: 8,
        dataFlows: 6,
        errorHandling: 5,
        performanceRequirements: 5,
        technicalDebt: 4
    };
    return maxScores[factor] || 5;
}

/**
 * Advanced usage: Custom complexity analysis with neural feedback
 */
async function advancedExample() {
    console.log('\n\nAdvanced Example: Custom Analysis with Pattern Learning');
    console.log('='.repeat(60));
    
    const analyzer = new TaskComplexityAnalyzer();
    
    // Simulate multiple analyses to show learning
    const historicalTasks = [
        {
            description: "Implement basic websocket connection",
            actualComplexity: 4, // Actual complexity from experience
            actualDuration: 6    // Actual hours taken
        },
        {
            description: "Add websocket reconnection logic with exponential backoff",
            actualComplexity: 6,
            actualDuration: 12
        },
        {
            description: "Build complete websocket streaming system with failover",
            actualComplexity: 8,
            actualDuration: 32
        }
    ];
    
    console.log('\nAnalyzing historical tasks to improve accuracy...\n');
    
    for (const task of historicalTasks) {
        const result = await analyzer.analyzeTaskComplexity({
            description: task.description
        });
        
        console.log(`Task: ${task.description}`);
        console.log(`Predicted complexity: ${result.complexity.score}, Actual: ${task.actualComplexity}`);
        console.log(`Predicted duration: ${result.estimatedDuration.hours}h, Actual: ${task.actualDuration}h`);
        console.log(`Accuracy: ${(100 - Math.abs(result.complexity.score - task.actualComplexity) * 10).toFixed(1)}%\n`);
    }
    
    // Now analyze a new similar task
    console.log('Analyzing new task with learned patterns...');
    const newTask = await analyzer.analyzeTaskComplexity({
        description: "Implement websocket connection pooling with load balancing and health checks",
        components: ['websocket', 'connection_pool', 'load_balancer', 'health_monitor']
    });
    
    console.log('\nPredicted complexity:', newTask.complexity.score);
    console.log('Confidence:', (newTask.complexity.confidence * 100).toFixed(1) + '%');
    console.log('This prediction incorporates patterns from', analyzer.analysisHistory.length, 'previous analyses');
}

// Run examples if executed directly
if (require.main === module) {
    runExamples()
        .then(() => advancedExample())
        .catch(console.error);
}

module.exports = {
    runExamples,
    advancedExample
};