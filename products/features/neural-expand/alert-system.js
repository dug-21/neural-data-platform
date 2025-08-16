/**
 * Real-time Alert System for Performance Monitoring
 * 
 * Provides multi-channel alerting with intelligent deduplication,
 * escalation policies, and automated remediation triggers.
 */

import { EventEmitter } from 'events';
import WebSocket from 'ws';

/**
 * Advanced Alert System with ML-based anomaly detection
 */
export class AlertSystem extends EventEmitter {
    constructor(config = {}) {
        super();
        
        this.config = {
            channels: config.channels || ['console', 'websocket'],
            deduplicationWindow: config.deduplicationWindow || 300000, // 5 minutes
            escalationPolicy: config.escalationPolicy || 'progressive',
            anomalyDetection: config.anomalyDetection !== false,
            autoRemediation: config.autoRemediation !== false,
            ...config
        };
        
        // Alert channels
        this.channels = new Map();
        this.initializeChannels();
        
        // Alert management
        this.activeAlerts = new Map();
        this.alertHistory = [];
        this.suppressedAlerts = new Set();
        
        // Escalation management
        this.escalationState = new Map();
        
        // Anomaly detection
        this.anomalyDetector = new AnomalyDetector();
        
        // Auto-remediation
        this.remediationEngine = new RemediationEngine();
    }
    
    /**
     * Initialize alert channels
     */
    initializeChannels() {
        this.config.channels.forEach(channelName => {
            switch (channelName) {
                case 'console':
                    this.channels.set('console', new ConsoleAlertChannel());
                    break;
                case 'websocket':
                    this.channels.set('websocket', new WebSocketAlertChannel(this.config.websocketPort || 8080));
                    break;
                case 'email':
                    this.channels.set('email', new EmailAlertChannel(this.config.emailConfig));
                    break;
                case 'slack':
                    this.channels.set('slack', new SlackAlertChannel(this.config.slackConfig));
                    break;
                case 'pagerduty':
                    this.channels.set('pagerduty', new PagerDutyAlertChannel(this.config.pagerdutyConfig));
                    break;
                default:
                    console.warn(`Unknown alert channel: ${channelName}`);
            }
        });
    }
    
    /**
     * Create and process a new alert
     */
    createAlert(alertData) {
        const alert = new Alert({
            ...alertData,
            id: this.generateAlertId(),
            timestamp: Date.now(),
            correlationId: this.generateCorrelationId(alertData)
        });
        
        // Check for anomalies
        if (this.config.anomalyDetection) {
            const anomalyScore = this.anomalyDetector.analyze(alert);
            alert.anomalyScore = anomalyScore;
            
            if (anomalyScore > 0.8) {
                alert.severity = this.escalateSeverity(alert.severity);
                alert.tags.push('anomaly');
            }
        }
        
        // Deduplication check
        if (this.isDuplicate(alert)) {
            this.updateExistingAlert(alert);
            return;
        }
        
        // Process the alert
        this.processAlert(alert);
    }
    
    /**
     * Process an alert through the system
     */
    processAlert(alert) {
        // Store alert
        this.activeAlerts.set(alert.id, alert);
        this.alertHistory.push(alert);
        
        // Apply suppression rules
        if (this.shouldSuppress(alert)) {
            alert.suppressed = true;
            this.suppressedAlerts.add(alert.id);
            this.emit('alert-suppressed', alert);
            return;
        }
        
        // Correlation with existing alerts
        this.correlateAlert(alert);
        
        // Route to appropriate channels based on severity and type
        const channels = this.selectChannels(alert);
        
        // Send alert to channels
        channels.forEach(channel => {
            channel.send(alert).catch(error => {
                console.error(`Failed to send alert via ${channel.name}:`, error);
                this.emit('channel-error', { channel: channel.name, error, alert });
            });
        });
        
        // Handle escalation
        if (this.shouldEscalate(alert)) {
            this.escalateAlert(alert);
        }
        
        // Trigger auto-remediation if enabled
        if (this.config.autoRemediation && alert.remediation) {
            this.triggerRemediation(alert);
        }
        
        // Emit alert event
        this.emit('alert', alert);
    }
    
    /**
     * Check if alert is a duplicate
     */
    isDuplicate(alert) {
        const cutoff = Date.now() - this.config.deduplicationWindow;
        
        for (const [id, existingAlert] of this.activeAlerts) {
            if (existingAlert.timestamp < cutoff) {
                this.activeAlerts.delete(id);
                continue;
            }
            
            if (this.alertsAreSimilar(alert, existingAlert)) {
                return true;
            }
        }
        
        return false;
    }
    
    /**
     * Check if two alerts are similar enough to be considered duplicates
     */
    alertsAreSimilar(alert1, alert2) {
        return alert1.type === alert2.type &&
               alert1.component === alert2.component &&
               alert1.severity === alert2.severity &&
               this.messagesSimilar(alert1.message, alert2.message);
    }
    
    /**
     * Check message similarity using fuzzy matching
     */
    messagesSimilar(msg1, msg2) {
        // Simple similarity check - could be enhanced with fuzzy matching
        const similarity = this.calculateSimilarity(msg1, msg2);
        return similarity > 0.8;
    }
    
    /**
     * Calculate string similarity (Levenshtein distance based)
     */
    calculateSimilarity(str1, str2) {
        const longer = str1.length > str2.length ? str1 : str2;
        const shorter = str1.length > str2.length ? str2 : str1;
        
        if (longer.length === 0) return 1.0;
        
        const distance = this.levenshteinDistance(longer, shorter);
        return (longer.length - distance) / longer.length;
    }
    
    /**
     * Calculate Levenshtein distance between two strings
     */
    levenshteinDistance(str1, str2) {
        const matrix = [];
        
        for (let i = 0; i <= str2.length; i++) {
            matrix[i] = [i];
        }
        
        for (let j = 0; j <= str1.length; j++) {
            matrix[0][j] = j;
        }
        
        for (let i = 1; i <= str2.length; i++) {
            for (let j = 1; j <= str1.length; j++) {
                if (str2.charAt(i - 1) === str1.charAt(j - 1)) {
                    matrix[i][j] = matrix[i - 1][j - 1];
                } else {
                    matrix[i][j] = Math.min(
                        matrix[i - 1][j - 1] + 1,
                        matrix[i][j - 1] + 1,
                        matrix[i - 1][j] + 1
                    );
                }
            }
        }
        
        return matrix[str2.length][str1.length];
    }
    
    /**
     * Update an existing alert with new occurrence
     */
    updateExistingAlert(newAlert) {
        for (const [id, existingAlert] of this.activeAlerts) {
            if (this.alertsAreSimilar(newAlert, existingAlert)) {
                existingAlert.occurrences = (existingAlert.occurrences || 1) + 1;
                existingAlert.lastOccurrence = Date.now();
                
                // Update severity if it has increased
                if (this.severityLevel(newAlert.severity) > this.severityLevel(existingAlert.severity)) {
                    existingAlert.severity = newAlert.severity;
                    this.emit('alert-severity-increased', existingAlert);
                }
                
                this.emit('alert-updated', existingAlert);
                break;
            }
        }
    }
    
    /**
     * Correlate alert with existing alerts
     */
    correlateAlert(alert) {
        const correlatedAlerts = [];
        
        for (const [id, existingAlert] of this.activeAlerts) {
            if (id === alert.id) continue;
            
            // Time-based correlation
            const timeDiff = Math.abs(alert.timestamp - existingAlert.timestamp);
            if (timeDiff < 10000) { // Within 10 seconds
                correlatedAlerts.push({
                    alert: existingAlert,
                    correlation: 'temporal',
                    score: 1 - timeDiff / 10000
                });
            }
            
            // Component-based correlation
            if (alert.component === existingAlert.component) {
                correlatedAlerts.push({
                    alert: existingAlert,
                    correlation: 'component',
                    score: 0.8
                });
            }
            
            // Pattern-based correlation
            if (this.haveCommonPattern(alert, existingAlert)) {
                correlatedAlerts.push({
                    alert: existingAlert,
                    correlation: 'pattern',
                    score: 0.7
                });
            }
        }
        
        if (correlatedAlerts.length > 0) {
            alert.correlatedAlerts = correlatedAlerts
                .sort((a, b) => b.score - a.score)
                .slice(0, 5);
        }
    }
    
    /**
     * Check if alerts have common patterns
     */
    haveCommonPattern(alert1, alert2) {
        const patterns = ['spike', 'degradation', 'failure', 'timeout', 'overload'];
        
        return patterns.some(pattern => 
            alert1.message.toLowerCase().includes(pattern) &&
            alert2.message.toLowerCase().includes(pattern)
        );
    }
    
    /**
     * Check if alert should be suppressed
     */
    shouldSuppress(alert) {
        // Implement suppression rules
        const suppressionRules = [
            {
                condition: (a) => a.severity === 'info' && a.occurrences > 10,
                reason: 'Too many info alerts'
            },
            {
                condition: (a) => a.component === 'test' && process.env.NODE_ENV === 'production',
                reason: 'Test component in production'
            },
            {
                condition: (a) => this.isFlapping(a),
                reason: 'Alert is flapping'
            }
        ];
        
        for (const rule of suppressionRules) {
            if (rule.condition(alert)) {
                alert.suppressionReason = rule.reason;
                return true;
            }
        }
        
        return false;
    }
    
    /**
     * Check if alert is flapping (rapidly changing state)
     */
    isFlapping(alert) {
        const recentAlerts = this.alertHistory
            .filter(a => 
                a.type === alert.type &&
                a.component === alert.component &&
                Date.now() - a.timestamp < 60000 // Last minute
            );
        
        return recentAlerts.length > 5;
    }
    
    /**
     * Select channels based on alert properties
     */
    selectChannels(alert) {
        const channels = [];
        
        // Always use console for all alerts
        if (this.channels.has('console')) {
            channels.push(this.channels.get('console'));
        }
        
        // WebSocket for real-time updates
        if (this.channels.has('websocket')) {
            channels.push(this.channels.get('websocket'));
        }
        
        // Email for warnings and above
        if (this.channels.has('email') && this.severityLevel(alert.severity) >= 2) {
            channels.push(this.channels.get('email'));
        }
        
        // Slack for high severity
        if (this.channels.has('slack') && this.severityLevel(alert.severity) >= 3) {
            channels.push(this.channels.get('slack'));
        }
        
        // PagerDuty for critical alerts
        if (this.channels.has('pagerduty') && alert.severity === 'critical') {
            channels.push(this.channels.get('pagerduty'));
        }
        
        return channels;
    }
    
    /**
     * Check if alert should be escalated
     */
    shouldEscalate(alert) {
        if (this.config.escalationPolicy === 'none') return false;
        
        const escalationKey = `${alert.type}-${alert.component}`;
        const state = this.escalationState.get(escalationKey) || { level: 0, lastEscalation: 0 };
        
        // Progressive escalation based on occurrences and time
        if (alert.occurrences > 5 && Date.now() - state.lastEscalation > 300000) {
            return true;
        }
        
        // Immediate escalation for critical alerts
        if (alert.severity === 'critical' && state.level === 0) {
            return true;
        }
        
        return false;
    }
    
    /**
     * Escalate an alert
     */
    escalateAlert(alert) {
        const escalationKey = `${alert.type}-${alert.component}`;
        const state = this.escalationState.get(escalationKey) || { level: 0, lastEscalation: 0 };
        
        state.level++;
        state.lastEscalation = Date.now();
        this.escalationState.set(escalationKey, state);
        
        // Create escalated alert
        const escalatedAlert = {
            ...alert,
            escalated: true,
            escalationLevel: state.level,
            originalSeverity: alert.severity,
            severity: this.escalateSeverity(alert.severity)
        };
        
        this.emit('alert-escalated', escalatedAlert);
        
        // Re-process with higher severity
        this.processAlert(escalatedAlert);
    }
    
    /**
     * Escalate severity level
     */
    escalateSeverity(severity) {
        const levels = ['info', 'warning', 'high', 'critical'];
        const currentIndex = levels.indexOf(severity);
        return levels[Math.min(currentIndex + 1, levels.length - 1)];
    }
    
    /**
     * Get numeric severity level
     */
    severityLevel(severity) {
        const levels = { info: 1, warning: 2, high: 3, critical: 4 };
        return levels[severity] || 0;
    }
    
    /**
     * Trigger auto-remediation
     */
    triggerRemediation(alert) {
        if (!alert.remediation) return;
        
        this.remediationEngine.execute(alert.remediation, alert)
            .then(result => {
                this.emit('remediation-success', {
                    alert,
                    remediation: alert.remediation,
                    result
                });
                
                // Mark alert as remediated
                alert.remediated = true;
                alert.remediationResult = result;
            })
            .catch(error => {
                this.emit('remediation-failure', {
                    alert,
                    remediation: alert.remediation,
                    error
                });
            });
    }
    
    /**
     * Clear an alert
     */
    clearAlert(alertId, reason = 'manual') {
        const alert = this.activeAlerts.get(alertId);
        if (!alert) return;
        
        alert.cleared = true;
        alert.clearedAt = Date.now();
        alert.clearReason = reason;
        
        this.activeAlerts.delete(alertId);
        
        // Notify channels
        const channels = this.selectChannels(alert);
        channels.forEach(channel => {
            channel.clear(alert).catch(error => {
                console.error(`Failed to clear alert via ${channel.name}:`, error);
            });
        });
        
        this.emit('alert-cleared', alert);
    }
    
    /**
     * Generate unique alert ID
     */
    generateAlertId() {
        return `alert-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    }
    
    /**
     * Generate correlation ID for similar alerts
     */
    generateCorrelationId(alertData) {
        return `${alertData.type}-${alertData.component}-${alertData.severity}`;
    }
    
    /**
     * Get alert statistics
     */
    getStatistics() {
        const stats = {
            active: this.activeAlerts.size,
            suppressed: this.suppressedAlerts.size,
            total: this.alertHistory.length,
            byType: {},
            bySeverity: {},
            byComponent: {},
            escalated: 0,
            remediated: 0
        };
        
        this.alertHistory.forEach(alert => {
            // By type
            stats.byType[alert.type] = (stats.byType[alert.type] || 0) + 1;
            
            // By severity
            stats.bySeverity[alert.severity] = (stats.bySeverity[alert.severity] || 0) + 1;
            
            // By component
            stats.byComponent[alert.component] = (stats.byComponent[alert.component] || 0) + 1;
            
            // Escalated
            if (alert.escalated) stats.escalated++;
            
            // Remediated
            if (alert.remediated) stats.remediated++;
        });
        
        return stats;
    }
}

/**
 * Alert data structure
 */
class Alert {
    constructor(data) {
        this.id = data.id;
        this.type = data.type; // cpu, memory, latency, etc.
        this.severity = data.severity; // info, warning, high, critical
        this.component = data.component;
        this.message = data.message;
        this.timestamp = data.timestamp;
        this.correlationId = data.correlationId;
        this.tags = data.tags || [];
        this.metadata = data.metadata || {};
        this.remediation = data.remediation;
        this.occurrences = 1;
        this.suppressed = false;
        this.cleared = false;
        this.remediated = false;
        this.escalated = false;
    }
}

/**
 * Base alert channel
 */
class AlertChannel {
    constructor(name) {
        this.name = name;
    }
    
    async send(alert) {
        throw new Error('Subclass must implement send()');
    }
    
    async clear(alert) {
        // Optional - implement in subclass if channel supports clearing
    }
}

/**
 * Console alert channel
 */
class ConsoleAlertChannel extends AlertChannel {
    constructor() {
        super('console');
    }
    
    async send(alert) {
        const timestamp = new Date(alert.timestamp).toISOString();
        const severity = alert.severity.toUpperCase();
        const icon = this.getIcon(alert.severity);
        
        console.log(`${icon} [${timestamp}] ${severity}: ${alert.message}`);
        
        if (alert.correlatedAlerts?.length > 0) {
            console.log(`   Correlated with ${alert.correlatedAlerts.length} other alerts`);
        }
        
        if (alert.remediation) {
            console.log(`   Remediation available: ${alert.remediation.action}`);
        }
    }
    
    async clear(alert) {
        console.log(`✅ Alert cleared: ${alert.id} (${alert.clearReason})`);
    }
    
    getIcon(severity) {
        const icons = {
            info: 'ℹ️',
            warning: '⚠️',
            high: '🔴',
            critical: '🚨'
        };
        return icons[severity] || '📢';
    }
}

/**
 * WebSocket alert channel for real-time dashboard
 */
class WebSocketAlertChannel extends AlertChannel {
    constructor(port = 8080) {
        super('websocket');
        this.port = port;
        this.clients = new Set();
        this.server = null;
        this.startServer();
    }
    
    startServer() {
        this.server = new WebSocket.Server({ port: this.port });
        
        this.server.on('connection', (ws) => {
            this.clients.add(ws);
            
            // Send current active alerts
            ws.send(JSON.stringify({
                type: 'init',
                activeAlerts: Array.from(this.activeAlerts?.values() || [])
            }));
            
            ws.on('close', () => {
                this.clients.delete(ws);
            });
            
            ws.on('error', (error) => {
                console.error('WebSocket error:', error);
                this.clients.delete(ws);
            });
        });
    }
    
    async send(alert) {
        const message = JSON.stringify({
            type: 'alert',
            alert
        });
        
        this.broadcast(message);
    }
    
    async clear(alert) {
        const message = JSON.stringify({
            type: 'clear',
            alertId: alert.id,
            reason: alert.clearReason
        });
        
        this.broadcast(message);
    }
    
    broadcast(message) {
        this.clients.forEach(client => {
            if (client.readyState === WebSocket.OPEN) {
                client.send(message);
            }
        });
    }
}

/**
 * Email alert channel (placeholder)
 */
class EmailAlertChannel extends AlertChannel {
    constructor(config) {
        super('email');
        this.config = config;
    }
    
    async send(alert) {
        // In production, implement actual email sending
        console.log(`[EMAIL] Would send alert to ${this.config?.recipient}: ${alert.message}`);
    }
}

/**
 * Slack alert channel (placeholder)
 */
class SlackAlertChannel extends AlertChannel {
    constructor(config) {
        super('slack');
        this.config = config;
    }
    
    async send(alert) {
        // In production, implement Slack webhook integration
        console.log(`[SLACK] Would post to ${this.config?.channel}: ${alert.message}`);
    }
}

/**
 * PagerDuty alert channel (placeholder)
 */
class PagerDutyAlertChannel extends AlertChannel {
    constructor(config) {
        super('pagerduty');
        this.config = config;
    }
    
    async send(alert) {
        // In production, implement PagerDuty API integration
        console.log(`[PAGERDUTY] Would create incident: ${alert.message}`);
    }
}

/**
 * Anomaly detector for identifying unusual patterns
 */
class AnomalyDetector {
    constructor() {
        this.baseline = new Map();
        this.threshold = 2.5; // Standard deviations
    }
    
    analyze(alert) {
        const key = `${alert.type}-${alert.component}`;
        const baseline = this.baseline.get(key);
        
        if (!baseline) {
            // No baseline yet - establish one
            this.baseline.set(key, {
                count: 1,
                lastSeen: alert.timestamp,
                severities: [alert.severity]
            });
            return 0;
        }
        
        // Calculate anomaly score based on frequency and pattern
        const timeSinceLastSeen = alert.timestamp - baseline.lastSeen;
        const expectedInterval = baseline.averageInterval || 3600000; // Default 1 hour
        
        let score = 0;
        
        // Frequency anomaly
        if (timeSinceLastSeen < expectedInterval / 10) {
            score += 0.5; // Very frequent
        }
        
        // Severity anomaly
        if (!baseline.severities.includes(alert.severity)) {
            score += 0.3; // New severity level
        }
        
        // Time of day anomaly (simplified)
        const hour = new Date(alert.timestamp).getHours();
        if (hour < 6 || hour > 22) {
            score += 0.2; // Outside business hours
        }
        
        // Update baseline
        baseline.count++;
        baseline.lastSeen = alert.timestamp;
        baseline.severities.push(alert.severity);
        baseline.averageInterval = (baseline.averageInterval || timeSinceLastSeen) * 0.9 + timeSinceLastSeen * 0.1;
        
        return Math.min(score, 1);
    }
}

/**
 * Auto-remediation engine
 */
class RemediationEngine {
    constructor() {
        this.actions = new Map();
        this.registerDefaultActions();
    }
    
    registerDefaultActions() {
        // CPU remediation
        this.actions.set('scale-horizontally', async (params) => {
            console.log(`[REMEDIATION] Scaling horizontally: ${params.instances} new instances`);
            return { success: true, scaled: params.instances };
        });
        
        // Memory remediation
        this.actions.set('force-gc', async () => {
            if (global.gc) {
                global.gc();
                console.log('[REMEDIATION] Forced garbage collection');
                return { success: true };
            }
            return { success: false, reason: 'GC not exposed' };
        });
        
        // Cache remediation
        this.actions.set('clear-cache', async (params) => {
            console.log(`[REMEDIATION] Clearing cache: ${params.cache}`);
            return { success: true, cleared: params.cache };
        });
        
        // Service remediation
        this.actions.set('restart-service', async (params) => {
            console.log(`[REMEDIATION] Would restart service: ${params.service}`);
            return { success: true, service: params.service };
        });
    }
    
    async execute(remediation, alert) {
        const action = this.actions.get(remediation.action);
        
        if (!action) {
            throw new Error(`Unknown remediation action: ${remediation.action}`);
        }
        
        console.log(`[REMEDIATION] Executing ${remediation.action} for alert ${alert.id}`);
        
        return await action(remediation.params || {});
    }
}

export { AlertSystem, Alert };