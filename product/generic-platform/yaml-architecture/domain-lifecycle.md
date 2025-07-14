# Domain Lifecycle Management

## Overview

Domain lifecycle management is a critical component of the Domain Registry system that ensures domains transition through well-defined states with proper validation, resource allocation, and cleanup. This document details the lifecycle states, transitions, and management strategies.

## Lifecycle States

### 1. Discovered
- **Description**: Domain configuration file has been detected by a discovery provider
- **Entry Conditions**: File matching domain pattern found
- **Exit Conditions**: Validation initiated or discovery expired
- **Timeout**: 5 minutes (configurable)
- **Actions**:
  - File parsing
  - Initial metadata extraction
  - Duplicate detection
  - Event emission: `domain.discovered`

### 2. Validating
- **Description**: Domain configuration is being validated
- **Entry Conditions**: Manual trigger or automatic after discovery
- **Exit Conditions**: Validation complete (success or failure)
- **Timeout**: 60 seconds
- **Actions**:
  - Schema validation
  - Dependency verification
  - Resource requirement checks
  - Security policy validation
  - Network policy validation

### 3. Validated
- **Description**: Domain configuration has passed all validation checks
- **Entry Conditions**: All validation checks passed
- **Exit Conditions**: Activation initiated or validation expired
- **Timeout**: None (stable state)
- **Actions**:
  - Store validated configuration
  - Calculate resource allocations
  - Prepare activation plan
  - Event emission: `domain.validated`

### 4. Failed
- **Description**: Domain configuration failed validation
- **Entry Conditions**: Any validation check failed
- **Exit Conditions**: New configuration submitted
- **Timeout**: None (terminal state)
- **Actions**:
  - Store validation errors
  - Notify domain owner
  - Event emission: `domain.failed`

### 5. Initializing
- **Description**: Domain is being prepared for activation
- **Entry Conditions**: Activation requested on validated domain
- **Exit Conditions**: Initialization complete or failed
- **Timeout**: Configurable (default: 5 minutes)
- **Actions**:
  - Resource allocation
  - Network setup
  - Secret injection
  - Storage provisioning
  - Dependency resolution

### 6. Active
- **Description**: Domain is fully operational
- **Entry Conditions**: Successful initialization
- **Exit Conditions**: Deactivation requested
- **Timeout**: None (stable state)
- **Actions**:
  - Health monitoring
  - Metric collection
  - Traffic routing
  - Auto-scaling
  - Event emission: `domain.active`

### 7. Deactivating
- **Description**: Domain is being shut down gracefully
- **Entry Conditions**: Deactivation requested
- **Exit Conditions**: All resources released
- **Timeout**: Configurable (default: 10 minutes)
- **Actions**:
  - Connection draining
  - Data persistence
  - Resource cleanup
  - Traffic rerouting
  - Dependent notification

### 8. Deactivated
- **Description**: Domain is completely shut down
- **Entry Conditions**: Successful deactivation
- **Exit Conditions**: Reactivation or deletion
- **Timeout**: None (stable state)
- **Actions**:
  - Configuration archived
  - Metrics retained
  - Event emission: `domain.deactivated`

## State Transition Matrix

| From State    | To State      | Trigger                  | Guards                           |
|---------------|---------------|--------------------------|----------------------------------|
| Discovered    | Validating    | Auto/Manual              | File parseable                   |
| Discovered    | Failed        | Parse error              | -                                |
| Validating    | Validated     | Validation success       | All checks pass                  |
| Validating    | Failed        | Validation failure       | Any check fails                  |
| Validated     | Initializing  | Activation request       | Resources available              |
| Failed        | Validating    | Retry/Update             | New configuration                |
| Initializing  | Active        | Init success             | Health checks pass               |
| Initializing  | Failed        | Init failure             | Timeout or error                 |
| Active        | Deactivating  | Deactivation request     | No critical dependents           |
| Deactivating  | Deactivated   | Cleanup complete         | All resources released           |
| Deactivated   | Validating    | Reactivation request     | Configuration exists             |

## Lifecycle Hooks

### Pre-Validation Hooks
```yaml
hooks:
  pre_validation:
    - name: syntax_check
      script: /hooks/syntax_check.sh
      timeout: 30s
      required: true
    - name: security_scan
      script: /hooks/security_scan.sh
      timeout: 60s
      required: false
```

### Post-Validation Hooks
```yaml
hooks:
  post_validation:
    - name: dependency_resolver
      script: /hooks/resolve_deps.sh
      timeout: 120s
    - name: capacity_planner
      script: /hooks/plan_capacity.sh
      timeout: 60s
```

### Pre-Activation Hooks
```yaml
hooks:
  pre_activation:
    - name: resource_provisioner
      script: /hooks/provision.sh
      timeout: 300s
      retry: 3
    - name: network_configurator
      script: /hooks/setup_network.sh
      timeout: 120s
```

### Post-Activation Hooks
```yaml
hooks:
  post_activation:
    - name: health_verifier
      script: /hooks/verify_health.sh
      timeout: 60s
      retry: 5
    - name: traffic_enabler
      script: /hooks/enable_traffic.sh
      timeout: 30s
```

### Pre-Deactivation Hooks
```yaml
hooks:
  pre_deactivation:
    - name: backup_data
      script: /hooks/backup.sh
      timeout: 600s
    - name: notify_dependents
      script: /hooks/notify.sh
      timeout: 60s
```

### Post-Deactivation Hooks
```yaml
hooks:
  post_deactivation:
    - name: cleanup_resources
      script: /hooks/cleanup.sh
      timeout: 300s
    - name: archive_logs
      script: /hooks/archive.sh
      timeout: 120s
```

## Activation Strategies

### 1. Immediate Activation
```yaml
activation:
  strategy: immediate
  preWarm: false
  healthCheck:
    enabled: true
    path: /health
    interval: 5s
    timeout: 2s
    successThreshold: 3
```

**Process**:
1. Allocate all resources
2. Start domain instances
3. Wait for health checks
4. Enable traffic immediately

**Use Case**: Development environments, non-critical services

### 2. Rolling Activation
```yaml
activation:
  strategy: rolling
  maxUnavailable: 1
  maxSurge: 1
  preWarm: true
  healthCheck:
    enabled: true
    successThreshold: 5
```

**Process**:
1. Start new instances gradually
2. Verify health before proceeding
3. Shift traffic incrementally
4. Decommission old instances

**Use Case**: Production services with zero-downtime requirements

### 3. Canary Activation
```yaml
activation:
  strategy: canary
  steps:
    - percentage: 10
      duration: 5m
      metrics:
        - errorRate < 1%
        - latency.p99 < 100ms
    - percentage: 50
      duration: 10m
      metrics:
        - errorRate < 1%
        - latency.p99 < 100ms
    - percentage: 100
  rollbackOnFailure: true
```

**Process**:
1. Deploy to small percentage
2. Monitor metrics
3. Gradually increase traffic
4. Automatic rollback on failures

**Use Case**: High-risk changes, A/B testing

### 4. Blue-Green Activation
```yaml
activation:
  strategy: blue_green
  environments:
    - blue
    - green
  switchover:
    type: instant  # or gradual
    validation:
      duration: 10m
      metrics:
        - errorRate < 0.1%
```

**Process**:
1. Deploy to inactive environment
2. Run validation tests
3. Switch traffic at once
4. Keep old environment as backup

**Use Case**: Critical services requiring instant rollback

## Deactivation Strategies

### 1. Graceful Shutdown
```yaml
deactivation:
  strategy: graceful
  drainTimeout: 300s
  steps:
    - action: stop_accepting_new
    - action: wait_for_active_complete
      timeout: 300s
    - action: terminate_instances
```

### 2. Immediate Shutdown
```yaml
deactivation:
  strategy: immediate
  force: true
  preserveData: false
```

### 3. Scheduled Deactivation
```yaml
deactivation:
  strategy: scheduled
  schedule: "0 2 * * *"  # 2 AM daily
  notification:
    advance: 24h
    channels:
      - email
      - slack
```

## Lifecycle Events

### Event Schema
```yaml
event:
  id: uuid
  timestamp: ISO8601
  domain: string
  version: string
  type: string
  severity: info|warning|error|critical
  payload: object
  metadata:
    user: string
    source: string
    correlationId: string
```

### Event Types
```yaml
events:
  - domain.discovered
  - domain.validation.started
  - domain.validation.completed
  - domain.validation.failed
  - domain.activation.started
  - domain.activation.progress
  - domain.activation.completed
  - domain.activation.failed
  - domain.health.changed
  - domain.deactivation.started
  - domain.deactivation.completed
  - domain.deleted
  - domain.rollback.initiated
  - domain.rollback.completed
```

## Resource Management

### Resource Allocation
```yaml
resources:
  compute:
    strategy: guaranteed  # or best-effort
    cpu:
      request: 1000m
      limit: 2000m
    memory:
      request: 2Gi
      limit: 4Gi
  storage:
    persistent:
      size: 10Gi
      class: fast-ssd
    ephemeral:
      size: 5Gi
  network:
    bandwidth:
      ingress: 1Gbps
      egress: 1Gbps
    connections:
      max: 10000
      perIP: 100
```

### Resource Cleanup
```yaml
cleanup:
  strategy: cascade  # or orphan
  retain:
    logs: 30d
    metrics: 90d
    backups: 365d
  delete:
    - temporary_files
    - cache_data
    - session_data
```

## Health Management

### Health Check Configuration
```yaml
health:
  liveness:
    httpGet:
      path: /health/live
      port: 8080
    initialDelaySeconds: 30
    periodSeconds: 10
    timeoutSeconds: 5
    successThreshold: 1
    failureThreshold: 3
  readiness:
    httpGet:
      path: /health/ready
      port: 8080
    initialDelaySeconds: 10
    periodSeconds: 5
    successThreshold: 1
    failureThreshold: 3
  startup:
    httpGet:
      path: /health/startup
      port: 8080
    initialDelaySeconds: 0
    periodSeconds: 10
    successThreshold: 1
    failureThreshold: 30
```

### Health States
```yaml
health_states:
  healthy:
    conditions:
      - all_checks_passing
      - error_rate < 1%
      - latency_p99 < 100ms
  degraded:
    conditions:
      - some_checks_failing
      - error_rate < 5%
      - latency_p99 < 500ms
    actions:
      - alert_on_call
      - reduce_traffic
  critical:
    conditions:
      - critical_checks_failing
      - error_rate > 5%
    actions:
      - page_on_call
      - failover_traffic
```

## Rollback Procedures

### Automatic Rollback
```yaml
rollback:
  triggers:
    - metric: error_rate
      threshold: 5%
      duration: 5m
    - metric: latency_p99
      threshold: 1000ms
      duration: 5m
  strategy: immediate
  target: last_known_good
  preserve_data: true
```

### Manual Rollback
```yaml
rollback:
  approval:
    required: true
    approvers:
      - platform-team
      - domain-owner
  verification:
    - dependency_check
    - data_migration_check
  notification:
    channels:
      - slack
      - email
      - pagerduty
```

## Monitoring & Alerting

### Lifecycle Metrics
```yaml
metrics:
  - name: domain_state_transitions
    type: counter
    labels: [domain, from_state, to_state]
  - name: domain_activation_duration
    type: histogram
    labels: [domain, strategy]
  - name: domain_health_score
    type: gauge
    labels: [domain, component]
  - name: lifecycle_hook_duration
    type: histogram
    labels: [domain, hook, phase]
```

### Alerts
```yaml
alerts:
  - name: DomainActivationFailed
    condition: increase(domain_activation_failed[5m]) > 0
    severity: critical
    annotations:
      summary: "Domain activation failed"
      runbook: "https://wiki/runbooks/domain-activation"
  
  - name: DomainHealthDegraded
    condition: domain_health_score < 0.8
    for: 10m
    severity: warning
    
  - name: LifecycleTransitionStuck
    condition: domain_state_duration > 3600
    severity: warning
```

## Best Practices

### 1. State Management
- Always use idempotent operations
- Implement proper timeout handling
- Log all state transitions
- Maintain audit trail

### 2. Error Handling
- Define clear rollback procedures
- Implement circuit breakers
- Use exponential backoff for retries
- Preserve error context

### 3. Resource Management
- Pre-allocate resources when possible
- Implement proper cleanup
- Monitor resource usage
- Set appropriate limits

### 4. Dependency Management
- Verify dependencies before activation
- Handle circular dependencies
- Implement dependency timeouts
- Support optional dependencies

### 5. Monitoring
- Track all lifecycle transitions
- Monitor hook execution times
- Alert on stuck states
- Maintain historical data

## Implementation Example

### State Machine Implementation
```go
type DomainStateMachine struct {
    domain      *Domain
    currentState State
    transitions  map[StateTransition]TransitionFunc
    hooks       map[HookPoint][]Hook
}

func (sm *DomainStateMachine) Transition(to State) error {
    from := sm.currentState
    transition := StateTransition{From: from, To: to}
    
    // Check if transition is valid
    if !sm.isValidTransition(transition) {
        return ErrInvalidTransition
    }
    
    // Execute pre-transition hooks
    if err := sm.executeHooks(PreTransition, transition); err != nil {
        return err
    }
    
    // Execute transition
    transitionFunc := sm.transitions[transition]
    if err := transitionFunc(sm.domain); err != nil {
        sm.executeHooks(TransitionFailed, transition)
        return err
    }
    
    // Update state
    sm.currentState = to
    sm.domain.Status = to
    
    // Execute post-transition hooks
    sm.executeHooks(PostTransition, transition)
    
    return nil
}
```

### Hook Execution
```go
func (sm *DomainStateMachine) executeHooks(point HookPoint, ctx interface{}) error {
    hooks := sm.hooks[point]
    
    for _, hook := range hooks {
        // Create timeout context
        hookCtx, cancel := context.WithTimeout(context.Background(), hook.Timeout)
        defer cancel()
        
        // Execute hook with retry
        err := retry.Do(func() error {
            return hook.Execute(hookCtx, sm.domain, ctx)
        }, retry.Attempts(hook.Retries))
        
        if err != nil && hook.Required {
            return fmt.Errorf("required hook %s failed: %w", hook.Name, err)
        }
    }
    
    return nil
}
```

## Conclusion

Domain lifecycle management is crucial for maintaining system stability and reliability. By following these patterns and best practices, you can ensure smooth domain operations with minimal downtime and maximum observability.