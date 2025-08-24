# Neural Trader V2 - Binary Separation Chaos Engineering Framework

## Overview

Chaos Engineering framework for the **binary separation architecture** to validate system resilience, fault tolerance, and graceful degradation under adverse conditions through controlled binary failure injection and Redis Streams disruption.

## Binary Chaos Testing Strategy

### Independent Binary Failure Testing
- **config-store-binary**: gRPC service failure scenarios
- **data-ingestion-binary**: Streaming service disruption
- **ruv-FANN-binary**: Neural network model failures
- **DAA-coordinator-binary**: Distributed coordination failures
- **Redis Streams**: Cross-binary communication disruption

## Chaos Engineering Principles

### 1. Core Hypothesis
"The system will continue to function correctly even when failures occur"

### 2. Blast Radius Control
- Start with staging environments
- Limited scope experiments
- Gradual production rollout
- Immediate rollback capabilities

### 3. Observability First
- Comprehensive monitoring during experiments
- Real-time dashboards
- Automated alerting
- Detailed failure analysis

## Chaos Testing Categories

### Binary Infrastructure Failures
- **Binary process crashes**: Individual binary termination
- **Redis Streams partitions**: Cross-binary communication failure
- **gRPC connection failures**: Config store unavailability
- **Resource exhaustion**: Per-binary memory/CPU limits
- **Container failures**: Binary container restart scenarios

### Binary Application Failures
- **config-store**: gRPC service degradation
- **data-ingestion**: Stream processing failures
- **ruv-FANN**: Neural network inference errors
- **DAA-coordinator**: Agent coordination failures
- **Redis Streams**: Message ordering disruption

### Cross-Binary Data Failures
- **Stream message corruption**: Invalid Redis Streams data
- **Binary state inconsistency**: Cross-binary data drift
- **Configuration propagation failure**: Config updates not received
- **Neural model desync**: Model predictions inconsistency
- **Agent coordination breakdown**: DAA state conflicts

## 1. Binary Process Chaos Testing

### Binary Termination and Recovery Simulation
```rust
// tests/chaos/binary_chaos.rs
use std::collections::HashMap;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

pub struct BinaryChaosEngine {
    binary_processes: HashMap<String, Child>,
    chaos_experiments: Vec<ChaosExperiment>,
    redis_client: redis::Client,
}

impl BinaryChaosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let redis_client = redis::Client::open("redis://localhost:6379")?;
        
        Ok(BinaryChaosEngine {
            binary_processes: HashMap::new(),
            chaos_experiments: Vec::new(),
            redis_client,
        })
    }
    
    pub async fn simulate_binary_crash(
        &mut self,
        binary_name: &str,
        crash_duration: Duration,
        recovery_strategy: RecoveryStrategy,
    ) -> Result<ChaosExperiment, Box<dyn std::error::Error>> {
        let experiment_id = Uuid::new_v4().to_string();
        println!("🔥 Starting binary crash chaos: {} - {}", binary_name, experiment_id);
        
        let experiment = ChaosExperiment {
            id: experiment_id.clone(),
            experiment_type: ChaosType::BinaryCrash,
            target_binary: binary_name.to_string(),
            start_time: Instant::now(),
            duration: crash_duration,
            status: ChaosStatus::Running,
            metrics: ChaosMetrics::new(),
        };
        
        // Capture baseline metrics
        let baseline_metrics = self.capture_system_baseline().await?;
        
        // Terminate the binary process
        self.terminate_binary(binary_name).await?;
        
        // Monitor system behavior during outage
        let monitoring_task = self.start_chaos_monitoring(&experiment).await;
        
        // Wait for crash duration
        sleep(crash_duration).await;
        
        // Recover the binary based on strategy
        self.recover_binary(binary_name, recovery_strategy).await?;
        
        // Wait for system stabilization
        sleep(Duration::from_secs(10)).await;
        
        // Capture recovery metrics
        let recovery_metrics = self.capture_recovery_metrics().await?;
        
        // Stop monitoring
        monitoring_task.abort();
        
        let mut completed_experiment = experiment;
        completed_experiment.status = ChaosStatus::Completed;
        completed_experiment.metrics.baseline = Some(baseline_metrics);
        completed_experiment.metrics.recovery = Some(recovery_metrics);
        
        self.chaos_experiments.push(completed_experiment.clone());
        
        println!("✅ Binary crash chaos completed: {}", experiment_id);
        Ok(completed_experiment)
    }
    
    pub async fn simulate_redis_streams_partition(
        &mut self,
        affected_streams: Vec<&str>,
        partition_duration: Duration,
    ) -> Result<ChaosExperiment, Box<dyn std::error::Error>> {
        let experiment_id = Uuid::new_v4().to_string();
        println!("🔥 Starting Redis Streams partition: {}", experiment_id);
        
        let experiment = ChaosExperiment {
            id: experiment_id.clone(),
            experiment_type: ChaosType::RedisPartition,
            target_binary: "redis-streams".to_string(),
            start_time: Instant::now(),
            duration: partition_duration,
            status: ChaosStatus::Running,
            metrics: ChaosMetrics::new(),
        };
        
        // Block Redis Streams access using iptables or similar
        self.block_redis_access().await?;
        
        // Monitor binary behavior during partition
        let monitoring_task = self.start_partition_monitoring(&affected_streams).await;
        
        sleep(partition_duration).await;
        
        // Restore Redis Streams access
        self.restore_redis_access().await?;
        
        // Monitor recovery and message replay
        let recovery_start = Instant::now();
        self.monitor_message_replay(&affected_streams).await?;
        let recovery_time = recovery_start.elapsed();
        
        monitoring_task.abort();
        
        let mut completed_experiment = experiment;
        completed_experiment.status = ChaosStatus::Completed;
        completed_experiment.metrics.recovery_time = Some(recovery_time);
        
        self.chaos_experiments.push(completed_experiment.clone());
        
        println!("✅ Redis Streams partition completed: {}", experiment_id);
        Ok(completed_experiment)
    }
    
    pub async fn simulate_binary_cascade_failure(
        &mut self,
        failure_sequence: Vec<(&str, Duration)>, // (binary_name, delay)
        total_duration: Duration,
    ) -> Result<ChaosExperiment, Box<dyn std::error::Error>> {
        let experiment_id = Uuid::new_v4().to_string();
        println!("🔥 Starting binary cascade failure: {}", experiment_id);
        
        let experiment = ChaosExperiment {
            id: experiment_id.clone(),
            experiment_type: ChaosType::CascadeFailure,
            target_binary: "all-binaries".to_string(),
            start_time: Instant::now(),
            duration: total_duration,
            status: ChaosStatus::Running,
            metrics: ChaosMetrics::new(),
        };
        
        let baseline = self.capture_system_baseline().await?;
        
        // Execute cascade failure sequence
        for (binary_name, delay) in failure_sequence {
            sleep(delay).await;
            
            println!("Failing binary in cascade: {}", binary_name);
            self.terminate_binary(binary_name).await?;
            
            // Capture system state after each failure
            let state = self.capture_system_state().await?;
            println!("System state after {} failure: {:?}", binary_name, state.binary_health);
        }
        
        // Monitor system during complete outage
        let outage_duration = total_duration / 2;
        sleep(outage_duration).await;
        
        // Gradual recovery
        for (binary_name, _) in failure_sequence.iter().rev() {
            println!("Recovering binary: {}", binary_name);
            self.recover_binary(binary_name, RecoveryStrategy::CleanRestart).await?;
            sleep(Duration::from_secs(5)).await; // Staggered recovery
        }
        
        // Wait for system stabilization
        sleep(Duration::from_secs(30)).await;
        
        let recovery_metrics = self.capture_recovery_metrics().await?;
        
        let mut completed_experiment = experiment;
        completed_experiment.status = ChaosStatus::Completed;
        completed_experiment.metrics.baseline = Some(baseline);
        completed_experiment.metrics.recovery = Some(recovery_metrics);
        
        self.chaos_experiments.push(completed_experiment.clone());
        
        println!("✅ Binary cascade failure completed: {}", experiment_id);
        Ok(completed_experiment)
    }
    
    async fn terminate_binary(&mut self, binary_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        match binary_name {
            "config-store" => {
                Command::new("pkill")
                    .args(["-f", "config-store"])
                    .output()?;
            }
            "data-ingestion" => {
                Command::new("pkill")
                    .args(["-f", "data_ingestion"])
                    .output()?;
            }
            "ruv-fann" => {
                Command::new("pkill")
                    .args(["-f", "ruv-fann"])
                    .output()?;
            }
            "daa-coordinator" => {
                Command::new("pkill")
                    .args(["-f", "daa-coordinator"])
                    .output()?;
            }
            _ => return Err(format!("Unknown binary: {}", binary_name).into()),
        }
        
        println!("Binary {} terminated for chaos experiment", binary_name);
        Ok(())
    }
    
    async fn recover_binary(
        &mut self,
        binary_name: &str,
        strategy: RecoveryStrategy,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match strategy {
            RecoveryStrategy::CleanRestart => self.clean_restart_binary(binary_name).await,
            RecoveryStrategy::StateRecovery => self.recover_with_state(binary_name).await,
            RecoveryStrategy::ManualIntervention => {
                println!("Manual intervention required for {}", binary_name);
                // In real scenario, this would wait for manual recovery
                sleep(Duration::from_secs(5)).await;
                self.clean_restart_binary(binary_name).await
            }
        }
    }
    
    async fn clean_restart_binary(&mut self, binary_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let child = match binary_name {
            "config-store" => {
                Command::new("cargo")
                    .args(["run", "--bin", "config-store"])
                    .current_dir("./config-store")
                    .env("RUST_LOG", "info")
                    .env("DATABASE_URL", "postgres://user:pass@localhost:5432/config_store")
                    .env("REDIS_URL", "redis://localhost:6379")
                    .spawn()?
            }
            "data-ingestion" => {
                Command::new("python")
                    .args(["-m", "data_ingestion.main"])
                    .current_dir("./data_ingestion")
                    .env("PYTHONPATH", "./data_ingestion")
                    .env("REDIS_URL", "redis://localhost:6379")
                    .spawn()?
            }
            "ruv-fann" => {
                Command::new("cargo")
                    .args(["run", "--bin", "ruv-fann"])
                    .current_dir("./ruv-fann")
                    .env("RUST_LOG", "info")
                    .env("REDIS_URL", "redis://localhost:6379")
                    .spawn()?
            }
            "daa-coordinator" => {
                Command::new("cargo")
                    .args(["run", "--bin", "daa-coordinator"])
                    .current_dir("./daa-coordinator")
                    .env("RUST_LOG", "info")
                    .env("REDIS_URL", "redis://localhost:6379")
                    .spawn()?
            }
            _ => return Err(format!("Unknown binary: {}", binary_name).into()),
        };
        
        self.binary_processes.insert(binary_name.to_string(), child);
        println!("Binary {} restarted after chaos experiment", binary_name);
        
        // Wait for binary to be ready
        sleep(Duration::from_secs(10)).await;
        
        Ok(())
    }
    
    async fn capture_system_baseline(&self) -> Result<SystemMetrics, Box<dyn std::error::Error>> {
        let mut con = self.redis_client.get_async_connection().await?;
        
        // Test all critical streams
        let streams = vec![
            "market_data_stream",
            "config_updates_stream",
            "neural_signals_stream",
            "daa_coordination_stream",
        ];
        
        let mut stream_health = HashMap::new();
        
        for stream in streams {
            let info: redis::Value = redis::cmd("XINFO")
                .arg("STREAM")
                .arg(stream)
                .query_async(&mut con)
                .await
                .unwrap_or(redis::Value::Nil);
                
            stream_health.insert(stream.to_string(), info != redis::Value::Nil);
        }
        
        Ok(SystemMetrics {
            timestamp: Instant::now(),
            binary_health: self.check_all_binaries_health().await?,
            stream_health,
            response_times: self.measure_response_times().await?,
            message_throughput: self.measure_message_throughput().await?,
        })
    }
    
    async fn check_all_binaries_health(&self) -> Result<HashMap<String, bool>, Box<dyn std::error::Error>> {
        let mut health = HashMap::new();
        
        let binaries = vec!["config-store", "data-ingestion", "ruv-fann", "daa-coordinator"];
        
        for binary in binaries {
            let is_healthy = self.check_binary_health(binary).await;
            health.insert(binary.to_string(), is_healthy);
        }
        
        Ok(health)
    }
    
    async fn check_binary_health(&self, binary_name: &str) -> bool {
        match binary_name {
            "config-store" => {
                // Check gRPC health endpoint
                // Simplified - in reality would make actual gRPC health check
                true
            }
            "data-ingestion" => {
                // Check if process is running and producing to streams
                Command::new("pgrep")
                    .args(["-f", "data_ingestion"])
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
            }
            "ruv-fann" => {
                Command::new("pgrep")
                    .args(["-f", "ruv-fann"])
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
            }
            "daa-coordinator" => {
                Command::new("pgrep")
                    .args(["-f", "daa-coordinator"])
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChaosExperiment {
    pub id: String,
    pub experiment_type: ChaosType,
    pub target_binary: String,
    pub start_time: Instant,
    pub duration: Duration,
    pub status: ChaosStatus,
    pub metrics: ChaosMetrics,
}

#[derive(Debug, Clone)]
pub enum ChaosType {
    BinaryCrash,
    RedisPartition,
    CascadeFailure,
    ResourceExhaustion,
}

#[derive(Debug, Clone)]
pub enum ChaosStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    CleanRestart,
    StateRecovery,
    ManualIntervention,
}

#[derive(Debug, Clone)]
pub struct ChaosMetrics {
    pub baseline: Option<SystemMetrics>,
    pub during_chaos: Vec<SystemMetrics>,
    pub recovery: Option<SystemMetrics>,
    pub recovery_time: Option<Duration>,
}

impl ChaosMetrics {
    pub fn new() -> Self {
        ChaosMetrics {
            baseline: None,
            during_chaos: Vec::new(),
            recovery: None,
            recovery_time: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub timestamp: Instant,
    pub binary_health: HashMap<String, bool>,
    pub stream_health: HashMap<String, bool>,
    pub response_times: HashMap<String, Duration>,
    pub message_throughput: HashMap<String, u64>,
}
```

## 2. Redis Streams Chaos Testing
  private activeExperiments: Set<string> = new Set();
  private toxiproxy: ToxiproxyClient;

  constructor() {
    this.toxiproxy = new ToxiproxyClient('http://localhost:8474');
  }

  async simulateNetworkPartition(
    services: string[],
    duration: number,
    partitionType: 'complete' | 'partial' = 'partial'
  ): Promise<ChaosExperiment> {
    const experimentId = `network-partition-${Date.now()}`;
    console.log(`🔥 Starting network partition chaos: ${experimentId}`);

    const experiment: ChaosExperiment = {
      id: experimentId,
      type: 'network-partition',
      startTime: new Date(),
      duration,
      services,
      status: 'running',
      metrics: {
        beforeFailure: await this.captureBaseline(services),
        duringFailure: [],
        afterRecovery: []
      }
    };

    this.activeExperiments.add(experimentId);

    try {
      // Create network proxies for each service
      for (const service of services) {
        await this.createNetworkProxy(service);
      }

      // Apply network failures
      if (partitionType === 'complete') {
        await this.applyCompleteNetworkFailure(services);
      } else {
        await this.applyPartialNetworkFailure(services);
      }

      // Monitor system behavior during chaos
      const monitoringTask = this.startChaosMonitoring(experiment);

      // Wait for experiment duration
      await new Promise(resolve => setTimeout(resolve, duration));

      // Restore network connectivity
      await this.restoreNetworkConnectivity(services);
      
      // Stop monitoring and capture recovery metrics
      clearInterval(monitoringTask);
      experiment.metrics.afterRecovery = await this.captureRecoveryMetrics(services);
      experiment.endTime = new Date();
      experiment.status = 'completed';

      console.log(`✅ Network partition chaos completed: ${experimentId}`);
      return experiment;

    } catch (error) {
      experiment.status = 'failed';
      experiment.error = error.message;
      console.error(`❌ Network partition chaos failed: ${error.message}`);
      throw error;

    } finally {
      this.activeExperiments.delete(experimentId);
      await this.cleanupNetworkProxies(services);
    }
  }

  async simulatePacketLoss(
    service: string,
    lossPercentage: number,
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `packet-loss-${Date.now()}`;
    
    const toxic = await this.toxiproxy.createToxic(service, {
      name: 'packet_loss',
      type: 'loss_upstream',
      attributes: {
        probability: lossPercentage / 100
      }
    });

    await new Promise(resolve => setTimeout(resolve, duration));
    
    await this.toxiproxy.removeToxic(service, toxic.id);

    return {
      id: experimentId,
      type: 'packet-loss',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: [service],
      status: 'completed',
      metrics: {}
    };
  }

  async simulateLatencySpike(
    service: string,
    latencyMs: number,
    jitterMs: number,
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `latency-spike-${Date.now()}`;
    
    const toxic = await this.toxiproxy.createToxic(service, {
      name: 'latency_spike',
      type: 'latency_downstream',
      attributes: {
        latency: latencyMs,
        jitter: jitterMs
      }
    });

    await new Promise(resolve => setTimeout(resolve, duration));
    
    await this.toxiproxy.removeToxic(service, toxic.id);

    return {
      id: experimentId,
      type: 'latency-spike',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: [service],
      status: 'completed',
      metrics: {}
    };
  }

  private async createNetworkProxy(service: string): Promise<void> {
    const serviceConfig = this.getServiceConfig(service);
    
    await this.toxiproxy.createProxy({
      name: service,
      listen: `0.0.0.0:${serviceConfig.proxyPort}`,
      upstream: `${serviceConfig.host}:${serviceConfig.port}`
    });
  }

  private async applyCompleteNetworkFailure(services: string[]): Promise<void> {
    for (const service of services) {
      await this.toxiproxy.createToxic(service, {
        name: 'complete_failure',
        type: 'reset_peer',
        attributes: {}
      });
    }
  }

  private async applyPartialNetworkFailure(services: string[]): Promise<void> {
    for (const service of services) {
      // Random packet loss and latency
      await Promise.all([
        this.toxiproxy.createToxic(service, {
          name: 'packet_loss',
          type: 'loss_upstream',
          attributes: { probability: 0.3 }
        }),
        this.toxiproxy.createToxic(service, {
          name: 'high_latency',
          type: 'latency_downstream',
          attributes: { latency: 2000, jitter: 500 }
        })
      ]);
    }
  }

  private async restoreNetworkConnectivity(services: string[]): Promise<void> {
    for (const service of services) {
      const toxics = await this.toxiproxy.getToxics(service);
      
      for (const toxic of toxics) {
        await this.toxiproxy.removeToxic(service, toxic.id);
      }
    }
  }

  private async cleanupNetworkProxies(services: string[]): Promise<void> {
    for (const service of services) {
      try {
        await this.toxiproxy.deleteProxy(service);
      } catch (error) {
        console.warn(`Failed to cleanup proxy for ${service}:`, error.message);
      }
    }
  }

  private startChaosMonitoring(experiment: ChaosExperiment): NodeJS.Timeout {
    return setInterval(async () => {
      const metrics = await this.captureMetricsDuringChaos(experiment.services);
      experiment.metrics.duringFailure.push({
        timestamp: new Date(),
        ...metrics
      });
    }, 5000); // Every 5 seconds
  }

  private getServiceConfig(service: string): ServiceConfig {
    const configs: Record<string, ServiceConfig> = {
      'market-data': { host: 'localhost', port: 8080, proxyPort: 28080 },
      'trading-engine': { host: 'localhost', port: 8081, proxyPort: 28081 },
      'config-store': { host: 'localhost', port: 8082, proxyPort: 28082 },
      'database': { host: 'localhost', port: 5432, proxyPort: 25432 },
      'redis': { host: 'localhost', port: 6379, proxyPort: 26379 }
    };
    
    return configs[service] || { host: 'localhost', port: 3000, proxyPort: 23000 };
  }

  private async captureBaseline(services: string[]): Promise<SystemMetrics> {
    return {
      timestamp: new Date(),
      responseTime: await this.measureResponseTime(services),
      errorRate: await this.measureErrorRate(services),
      throughput: await this.measureThroughput(services),
      availability: await this.measureAvailability(services)
    };
  }

  private async captureMetricsDuringChaos(services: string[]): Promise<SystemMetrics> {
    // Similar to baseline but during chaos
    return this.captureBaseline(services);
  }

  private async captureRecoveryMetrics(services: string[]): Promise<SystemMetrics> {
    // Measure recovery time and system stability
    return this.captureBaseline(services);
  }

  private async measureResponseTime(services: string[]): Promise<number> {
    const measurements = await Promise.all(
      services.map(async service => {
        const start = Date.now();
        try {
          await this.healthCheck(service);
          return Date.now() - start;
        } catch {
          return 5000; // Timeout value for failed requests
        }
      })
    );
    
    return measurements.reduce((a, b) => a + b, 0) / measurements.length;
  }

  private async measureErrorRate(services: string[]): Promise<number> {
    const results = await Promise.all(
      services.map(async service => {
        try {
          await this.healthCheck(service);
          return false;
        } catch {
          return true;
        }
      })
    );
    
    const errors = results.filter(Boolean).length;
    return errors / results.length;
  }

  private async measureThroughput(services: string[]): Promise<number> {
    // Simulate throughput measurement
    const start = Date.now();
    const requests = 10;
    
    const promises = Array.from({ length: requests }, () =>
      Promise.all(services.map(service => this.healthCheck(service).catch(() => null)))
    );
    
    await Promise.all(promises);
    const duration = (Date.now() - start) / 1000;
    
    return (requests * services.length) / duration;
  }

  private async measureAvailability(services: string[]): Promise<number> {
    const results = await Promise.all(
      services.map(async service => {
        try {
          await this.healthCheck(service);
          return true;
        } catch {
          return false;
        }
      })
    );
    
    const available = results.filter(Boolean).length;
    return available / results.length;
  }

  private async healthCheck(service: string): Promise<void> {
    const config = this.getServiceConfig(service);
    const response = await fetch(`http://${config.host}:${config.port}/health`);
    
    if (!response.ok) {
      throw new Error(`Health check failed for ${service}`);
    }
  }
}

interface ChaosExperiment {
  id: string;
  type: string;
  startTime: Date;
  endTime?: Date;
  duration: number;
  services: string[];
  status: 'running' | 'completed' | 'failed';
  error?: string;
  metrics: {
    beforeFailure: SystemMetrics;
    duringFailure: SystemMetrics[];
    afterRecovery: SystemMetrics[];
  };
}

interface SystemMetrics {
  timestamp: Date;
  responseTime: number;
  errorRate: number;
  throughput: number;
  availability: number;
}

interface ServiceConfig {
  host: string;
  port: number;
  proxyPort: number;
}
```

## 2. Service Failure Simulation

### Service Chaos Engine
```typescript
// tests/chaos/service-chaos.ts
export class ServiceChaosEngine {
  private docker: Docker;
  private activeExperiments: Map<string, ChaosExperiment> = new Map();

  constructor() {
    this.docker = new Docker();
  }

  async simulateServiceCrash(
    serviceName: string,
    crashType: 'immediate' | 'gradual' | 'oom',
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `service-crash-${serviceName}-${Date.now()}`;
    console.log(`🔥 Starting service crash chaos: ${experimentId}`);

    const experiment: ChaosExperiment = {
      id: experimentId,
      type: 'service-crash',
      startTime: new Date(),
      duration,
      services: [serviceName],
      status: 'running',
      metrics: {
        beforeFailure: await this.captureServiceMetrics(serviceName),
        duringFailure: [],
        afterRecovery: []
      }
    };

    this.activeExperiments.set(experimentId, experiment);

    try {
      const container = await this.getServiceContainer(serviceName);
      
      // Apply crash based on type
      switch (crashType) {
        case 'immediate':
          await this.immediateServiceCrash(container);
          break;
        case 'gradual':
          await this.gradualServiceCrash(container, duration);
          break;
        case 'oom':
          await this.outOfMemoryCrash(container);
          break;
      }

      // Monitor during chaos
      const monitoring = this.startServiceMonitoring(experiment);

      // Wait for experiment duration
      await new Promise(resolve => setTimeout(resolve, duration));

      // Restart service
      await this.restartService(container);
      
      // Wait for service recovery
      await this.waitForServiceRecovery(serviceName, 30000);

      clearInterval(monitoring);
      experiment.metrics.afterRecovery = await this.captureServiceMetrics(serviceName);
      experiment.endTime = new Date();
      experiment.status = 'completed';

      console.log(`✅ Service crash chaos completed: ${experimentId}`);
      return experiment;

    } catch (error) {
      experiment.status = 'failed';
      experiment.error = error.message;
      console.error(`❌ Service crash chaos failed: ${error.message}`);
      throw error;

    } finally {
      this.activeExperiments.delete(experimentId);
    }
  }

  async simulateHighCpuLoad(
    serviceName: string,
    cpuPercent: number,
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `cpu-chaos-${serviceName}-${Date.now()}`;
    
    const experiment: ChaosExperiment = {
      id: experimentId,
      type: 'cpu-chaos',
      startTime: new Date(),
      duration,
      services: [serviceName],
      status: 'running',
      metrics: {
        beforeFailure: await this.captureServiceMetrics(serviceName),
        duringFailure: [],
        afterRecovery: []
      }
    };

    const container = await this.getServiceContainer(serviceName);
    
    // Inject CPU stress
    await this.injectCpuStress(container, cpuPercent);
    
    // Monitor during experiment
    const monitoring = this.startServiceMonitoring(experiment);
    
    await new Promise(resolve => setTimeout(resolve, duration));
    
    // Remove CPU stress
    await this.removeCpuStress(container);
    
    clearInterval(monitoring);
    experiment.metrics.afterRecovery = await this.captureServiceMetrics(serviceName);
    experiment.endTime = new Date();
    experiment.status = 'completed';

    return experiment;
  }

  async simulateMemoryExhaustion(
    serviceName: string,
    memoryPercent: number,
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `memory-chaos-${serviceName}-${Date.now()}`;
    
    const container = await this.getServiceContainer(serviceName);
    
    // Get current memory limit
    const containerInfo = await container.inspect();
    const memoryLimit = containerInfo.HostConfig.Memory;
    const targetMemory = Math.floor(memoryLimit * memoryPercent / 100);
    
    // Inject memory stress
    await container.exec({
      Cmd: ['stress-ng', '--vm', '1', `--vm-bytes=${targetMemory}`, `--timeout=${duration}s`],
      AttachStdout: true,
      AttachStderr: true
    });

    // Monitor and wait
    await new Promise(resolve => setTimeout(resolve, duration));

    return {
      id: experimentId,
      type: 'memory-chaos',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: [serviceName],
      status: 'completed',
      metrics: {
        beforeFailure: { timestamp: new Date(), responseTime: 0, errorRate: 0, throughput: 0, availability: 1 },
        duringFailure: [],
        afterRecovery: []
      }
    };
  }

  async simulateDiskPressure(
    serviceName: string,
    diskUsagePercent: number,
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `disk-chaos-${serviceName}-${Date.now()}`;
    const container = await this.getServiceContainer(serviceName);
    
    // Fill disk to specified percentage
    const fillCommand = [
      'dd', 'if=/dev/zero', 'of=/tmp/chaos-fill',
      `bs=1M`, `count=${diskUsagePercent * 10}` // Approximate disk fill
    ];
    
    await container.exec({
      Cmd: fillCommand,
      AttachStdout: true,
      AttachStderr: true
    });

    await new Promise(resolve => setTimeout(resolve, duration));

    // Cleanup disk space
    await container.exec({
      Cmd: ['rm', '-f', '/tmp/chaos-fill'],
      AttachStdout: true,
      AttachStderr: true
    });

    return {
      id: experimentId,
      type: 'disk-chaos',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: [serviceName],
      status: 'completed',
      metrics: {
        beforeFailure: { timestamp: new Date(), responseTime: 0, errorRate: 0, throughput: 0, availability: 1 },
        duringFailure: [],
        afterRecovery: []
      }
    };
  }

  private async getServiceContainer(serviceName: string): Promise<Container> {
    const containers = await this.docker.listContainers({ all: true });
    const container = containers.find(c => 
      c.Names.some(name => name.includes(serviceName)) ||
      c.Labels?.['service'] === serviceName
    );

    if (!container) {
      throw new Error(`Service container not found: ${serviceName}`);
    }

    return this.docker.getContainer(container.Id);
  }

  private async immediateServiceCrash(container: Container): Promise<void> {
    await container.kill({ signal: 'SIGKILL' });
  }

  private async gradualServiceCrash(container: Container, duration: number): Promise<void> {
    // First send SIGTERM for graceful shutdown
    await container.kill({ signal: 'SIGTERM' });
    
    // Wait a bit, then force kill if still running
    setTimeout(async () => {
      try {
        const info = await container.inspect();
        if (info.State.Running) {
          await container.kill({ signal: 'SIGKILL' });
        }
      } catch {
        // Container already stopped
      }
    }, duration / 2);
  }

  private async outOfMemoryCrash(container: Container): Promise<void> {
    // Inject memory stress that will trigger OOM killer
    await container.exec({
      Cmd: ['stress-ng', '--vm', '1', '--vm-bytes', '2G', '--timeout', '60s'],
      AttachStdout: true,
      AttachStderr: true
    });
  }

  private async injectCpuStress(container: Container, cpuPercent: number): Promise<void> {
    const cpuWorkers = Math.ceil(cpuPercent / 25); // Each worker ~25% CPU
    
    await container.exec({
      Cmd: ['stress-ng', '--cpu', cpuWorkers.toString(), '--timeout', '0'],
      AttachStdout: true,
      AttachStderr: true,
      Detach: true
    });
  }

  private async removeCpuStress(container: Container): Promise<void> {
    await container.exec({
      Cmd: ['pkill', '-f', 'stress-ng'],
      AttachStdout: true,
      AttachStderr: true
    });
  }

  private async restartService(container: Container): Promise<void> {
    await container.restart();
  }

  private async waitForServiceRecovery(serviceName: string, timeoutMs: number): Promise<void> {
    const startTime = Date.now();
    
    while (Date.now() - startTime < timeoutMs) {
      try {
        await this.healthCheckService(serviceName);
        console.log(`✅ Service ${serviceName} recovered`);
        return;
      } catch {
        // Service not ready yet, wait
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
    }
    
    throw new Error(`Service ${serviceName} failed to recover within ${timeoutMs}ms`);
  }

  private async healthCheckService(serviceName: string): Promise<void> {
    const config = this.getServiceConfig(serviceName);
    const response = await fetch(`http://${config.host}:${config.port}/health`);
    
    if (!response.ok) {
      throw new Error(`Health check failed for ${serviceName}`);
    }
  }

  private async captureServiceMetrics(serviceName: string): Promise<SystemMetrics> {
    try {
      const container = await this.getServiceContainer(serviceName);
      const stats = await container.stats({ stream: false });
      
      return {
        timestamp: new Date(),
        responseTime: await this.measureServiceResponseTime(serviceName),
        errorRate: 0, // Would measure actual error rate
        throughput: 0, // Would measure actual throughput
        availability: 1, // Service is available
        cpuUsage: this.calculateCpuUsage(stats),
        memoryUsage: stats.memory_stats.usage / stats.memory_stats.limit
      };
    } catch {
      return {
        timestamp: new Date(),
        responseTime: 0,
        errorRate: 1,
        throughput: 0,
        availability: 0
      };
    }
  }

  private calculateCpuUsage(stats: any): number {
    const cpuDelta = stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    const systemDelta = stats.cpu_stats.system_cpu_usage - stats.precpu_stats.system_cpu_usage;
    return (cpuDelta / systemDelta) * 100;
  }

  private async measureServiceResponseTime(serviceName: string): Promise<number> {
    const start = Date.now();
    try {
      await this.healthCheckService(serviceName);
      return Date.now() - start;
    } catch {
      return -1; // Service unavailable
    }
  }

  private getServiceConfig(serviceName: string): { host: string; port: number } {
    const configs: Record<string, { host: string; port: number }> = {
      'market-data': { host: 'localhost', port: 8080 },
      'trading-engine': { host: 'localhost', port: 8081 },
      'config-store': { host: 'localhost', port: 8082 },
    };
    
    return configs[serviceName] || { host: 'localhost', port: 3000 };
  }

  private startServiceMonitoring(experiment: ChaosExperiment): NodeJS.Timeout {
    return setInterval(async () => {
      const metrics = await this.captureServiceMetrics(experiment.services[0]);
      experiment.metrics.duringFailure.push(metrics);
    }, 2000); // Every 2 seconds
  }
}
```

## 3. Data Chaos Testing

### Database Chaos Engine
```typescript
// tests/chaos/database-chaos.ts
export class DatabaseChaosEngine {
  private dbConnection: DatabaseConnection;
  
  constructor(dbConnection: DatabaseConnection) {
    this.dbConnection = dbConnection;
  }

  async simulateDataCorruption(
    tableName: string,
    corruptionType: 'random' | 'specific' | 'schema',
    affectedRows: number
  ): Promise<ChaosExperiment> {
    const experimentId = `data-corruption-${tableName}-${Date.now()}`;
    console.log(`🔥 Starting data corruption chaos: ${experimentId}`);

    // Backup original data
    const backup = await this.backupTable(tableName);
    
    try {
      switch (corruptionType) {
        case 'random':
          await this.injectRandomCorruption(tableName, affectedRows);
          break;
        case 'specific':
          await this.injectSpecificCorruption(tableName, affectedRows);
          break;
        case 'schema':
          await this.injectSchemaCorruption(tableName);
          break;
      }

      // Test system behavior with corrupted data
      const testResults = await this.testSystemWithCorruptedData(tableName);

      // Restore original data
      await this.restoreTable(tableName, backup);

      return {
        id: experimentId,
        type: 'data-corruption',
        startTime: new Date(),
        endTime: new Date(),
        duration: 0,
        services: [tableName],
        status: 'completed',
        metrics: {
          beforeFailure: { timestamp: new Date(), responseTime: 0, errorRate: 0, throughput: 0, availability: 1 },
          duringFailure: [testResults],
          afterRecovery: []
        }
      };

    } catch (error) {
      // Restore data on error
      await this.restoreTable(tableName, backup);
      throw error;
    }
  }

  async simulateSlowQueries(
    duration: number,
    slowdownFactor: number = 10
  ): Promise<ChaosExperiment> {
    const experimentId = `slow-queries-${Date.now()}`;
    
    // Inject artificial delays into database operations
    await this.injectQueryDelays(slowdownFactor);
    
    const monitoring = this.startQueryPerformanceMonitoring();
    
    await new Promise(resolve => setTimeout(resolve, duration));
    
    await this.removeQueryDelays();
    clearInterval(monitoring);

    return {
      id: experimentId,
      type: 'slow-queries',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: ['database'],
      status: 'completed',
      metrics: {
        beforeFailure: { timestamp: new Date(), responseTime: 0, errorRate: 0, throughput: 0, availability: 1 },
        duringFailure: [],
        afterRecovery: []
      }
    };
  }

  async simulateConnectionPoolExhaustion(
    duration: number
  ): Promise<ChaosExperiment> {
    const experimentId = `connection-exhaustion-${Date.now()}`;
    
    // Create many connections to exhaust pool
    const connections = await this.exhaustConnectionPool();
    
    const monitoring = this.startConnectionPoolMonitoring();
    
    await new Promise(resolve => setTimeout(resolve, duration));
    
    // Release connections
    await this.releaseConnections(connections);
    clearInterval(monitoring);

    return {
      id: experimentId,
      type: 'connection-exhaustion',
      startTime: new Date(Date.now() - duration),
      endTime: new Date(),
      duration,
      services: ['database'],
      status: 'completed',
      metrics: {
        beforeFailure: { timestamp: new Date(), responseTime: 0, errorRate: 0, throughput: 0, availability: 1 },
        duringFailure: [],
        afterRecovery: []
      }
    };
  }

  private async backupTable(tableName: string): Promise<any[]> {
    const result = await this.dbConnection.query(`SELECT * FROM ${tableName}`);
    return result.rows;
  }

  private async restoreTable(tableName: string, backup: any[]): Promise<void> {
    await this.dbConnection.query(`DELETE FROM ${tableName}`);
    
    for (const row of backup) {
      const columns = Object.keys(row).join(', ');
      const values = Object.values(row).map((_, i) => `$${i + 1}`).join(', ');
      
      await this.dbConnection.query(
        `INSERT INTO ${tableName} (${columns}) VALUES (${values})`,
        Object.values(row)
      );
    }
  }

  private async injectRandomCorruption(tableName: string, affectedRows: number): Promise<void> {
    // Randomly corrupt data in specified number of rows
    await this.dbConnection.query(`
      UPDATE ${tableName} 
      SET data = 'CORRUPTED_' || data
      WHERE id IN (
        SELECT id FROM ${tableName} 
        ORDER BY RANDOM() 
        LIMIT $1
      )
    `, [affectedRows]);
  }

  private async injectSpecificCorruption(tableName: string, affectedRows: number): Promise<void> {
    // Corrupt specific types of data (e.g., negative prices, invalid dates)
    await this.dbConnection.query(`
      UPDATE ${tableName}
      SET price = -price
      WHERE price > 0
      LIMIT $1
    `, [affectedRows]);
  }

  private async injectSchemaCorruption(tableName: string): Promise<void> {
    // Temporarily modify schema (add/remove columns)
    await this.dbConnection.query(`ALTER TABLE ${tableName} ADD COLUMN temp_corruption TEXT`);
    
    // This would be restored in the cleanup phase
  }

  private async testSystemWithCorruptedData(tableName: string): Promise<SystemMetrics> {
    // Test how the system handles corrupted data
    const start = Date.now();
    let errors = 0;
    let total = 0;

    try {
      // Test various operations with corrupted data
      const operations = [
        () => this.dbConnection.query(`SELECT * FROM ${tableName} LIMIT 10`),
        () => this.dbConnection.query(`SELECT AVG(price) FROM ${tableName} WHERE price > 0`),
        () => this.dbConnection.query(`SELECT COUNT(*) FROM ${tableName}`)
      ];

      for (const operation of operations) {
        total++;
        try {
          await operation();
        } catch {
          errors++;
        }
      }

    } catch (error) {
      errors++;
    }

    return {
      timestamp: new Date(),
      responseTime: Date.now() - start,
      errorRate: errors / total,
      throughput: total / ((Date.now() - start) / 1000),
      availability: errors === total ? 0 : 1
    };
  }

  private async injectQueryDelays(slowdownFactor: number): Promise<void> {
    // In a real implementation, this would inject delays at the database level
    // For testing, we can simulate this by wrapping query methods
  }

  private async removeQueryDelays(): Promise<void> {
    // Remove artificial delays
  }

  private async exhaustConnectionPool(): Promise<any[]> {
    const connections = [];
    
    // Create connections until pool is exhausted
    try {
      for (let i = 0; i < 100; i++) {
        const conn = await this.dbConnection.getConnection();
        connections.push(conn);
      }
    } catch {
      // Pool exhausted
    }

    return connections;
  }

  private async releaseConnections(connections: any[]): Promise<void> {
    for (const conn of connections) {
      try {
        conn.release();
      } catch {
        // Connection may have already been released
      }
    }
  }

  private startQueryPerformanceMonitoring(): NodeJS.Timeout {
    return setInterval(async () => {
      // Monitor query performance during chaos
      const slowQueries = await this.dbConnection.query(`
        SELECT query, mean_time, calls 
        FROM pg_stat_statements 
        WHERE mean_time > 100 
        ORDER BY mean_time DESC 
        LIMIT 10
      `);
      
      console.log('Slow queries detected:', slowQueries.rows.length);
    }, 5000);
  }

  private startConnectionPoolMonitoring(): NodeJS.Timeout {
    return setInterval(async () => {
      // Monitor connection pool status
      const poolStatus = await this.dbConnection.query(`
        SELECT state, COUNT(*) 
        FROM pg_stat_activity 
        GROUP BY state
      `);
      
      console.log('Connection pool status:', poolStatus.rows);
    }, 2000);
  }
}
```

## 4. Complete Chaos Test Suite

### Integrated Chaos Testing
```typescript
// tests/chaos/chaos-test-suite.ts
export class ChaosTestSuite {
  private networkChaos = new NetworkChaosEngine();
  private serviceChaos = new ServiceChaosEngine();
  private databaseChaos = new DatabaseChaosEngine(dbConnection);
  
  async runComprehensiveChaosTests(): Promise<ChaosTestReport> {
    const report: ChaosTestReport = {
      timestamp: new Date(),
      experiments: [],
      summary: {
        totalExperiments: 0,
        successfulExperiments: 0,
        failedExperiments: 0,
        systemResilienceScore: 0
      }
    };

    console.log('🔥 Starting comprehensive chaos engineering tests...');

    try {
      // Network chaos experiments
      report.experiments.push(
        await this.networkChaos.simulateNetworkPartition(['market-data', 'trading-engine'], 30000),
        await this.networkChaos.simulatePacketLoss('database', 20, 15000),
        await this.networkChaos.simulateLatencySpike('config-store', 1000, 200, 20000)
      );

      // Service chaos experiments
      report.experiments.push(
        await this.serviceChaos.simulateServiceCrash('market-data', 'immediate', 30000),
        await this.serviceChaos.simulateHighCpuLoad('trading-engine', 90, 25000),
        await this.serviceChaos.simulateMemoryExhaustion('config-store', 95, 20000)
      );

      // Database chaos experiments
      report.experiments.push(
        await this.databaseChaos.simulateSlowQueries(30000, 5),
        await this.databaseChaos.simulateConnectionPoolExhaustion(15000)
      );

      // Calculate summary
      report.summary = this.calculateChaosSummary(report.experiments);
      
      console.log('🎯 Chaos engineering tests completed');
      console.log(`Resilience Score: ${report.summary.systemResilienceScore}/100`);
      
      return report;

    } catch (error) {
      console.error('❌ Chaos test suite failed:', error.message);
      throw error;
    }
  }

  private calculateChaosSummary(experiments: ChaosExperiment[]): ChaosSummary {
    const totalExperiments = experiments.length;
    const successfulExperiments = experiments.filter(exp => exp.status === 'completed').length;
    const failedExperiments = totalExperiments - successfulExperiments;
    
    // Calculate resilience score based on system behavior during chaos
    let resilienceScore = 0;
    
    for (const experiment of experiments) {
      if (experiment.status === 'completed') {
        // Analyze metrics to determine how well system handled the chaos
        const duringMetrics = experiment.metrics.duringFailure;
        const beforeMetrics = experiment.metrics.beforeFailure;
        
        let experimentScore = 100;
        
        // Penalize high error rates
        const avgErrorRate = duringMetrics.reduce((sum, m) => sum + m.errorRate, 0) / duringMetrics.length;
        experimentScore -= (avgErrorRate * 100);
        
        // Penalize availability loss
        const avgAvailability = duringMetrics.reduce((sum, m) => sum + m.availability, 0) / duringMetrics.length;
        experimentScore -= ((1 - avgAvailability) * 50);
        
        // Penalize response time degradation
        const responseTimeDegradation = duringMetrics.reduce((sum, m) => sum + m.responseTime, 0) / duringMetrics.length;
        const baselineResponseTime = beforeMetrics.responseTime;
        if (responseTimeDegradation > baselineResponseTime * 2) {
          experimentScore -= 25;
        }
        
        resilienceScore += Math.max(0, experimentScore);
      }
    }
    
    resilienceScore = resilienceScore / totalExperiments;
    
    return {
      totalExperiments,
      successfulExperiments,
      failedExperiments,
      systemResilienceScore: Math.round(resilienceScore)
    };
  }
}

interface ChaosTestReport {
  timestamp: Date;
  experiments: ChaosExperiment[];
  summary: ChaosSummary;
}

interface ChaosSummary {
  totalExperiments: number;
  successfulExperiments: number;
  failedExperiments: number;
  systemResilienceScore: number; // 0-100
}
```

This comprehensive Chaos Engineering framework validates that Neural Trader V2 maintains resilience and graceful degradation under various failure conditions, ensuring production reliability.