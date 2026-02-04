# SPEC-D05: Correlation-Ready Dashboard (v11-011)

**Feature ID**: v11-011
**Feature Name**: Correlation-Ready Dashboard
**Priority**: High
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Overview

### 1.1 User Story

> As a **home owner**, I want to view aligned Gold layer data across all streams in a single dashboard, with objective thresholds visible, so that I can visually identify potential correlations between environmental conditions and air quality.

### 1.2 Goal

Create a Grafana dashboard that visualizes the aligned Gold layer data, demonstrating V1.1 capabilities and enabling visual exploration before V1.2 automated pattern detection.

### 1.3 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Aligned view time series visualization | Automated correlation detection (V1.2) |
| Objective thresholds as reference lines | Alert configuration |
| Multi-stream overlay charts | Statistical analysis panels |
| Gold layer query patterns | Bronze/Silver layer queries |
| Basic lag feature visualization | Complex feature engineering |

---

## 2. Functional Requirements

### 2.1 Dashboard Structure (FR-D05-DASH)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D05-DASH-001 | Dashboard displays aligned Gold layer data | P0 | All 4 streams visible |
| FR-D05-DASH-002 | Dashboard loads in < 2 seconds | P0 | Performance verified |
| FR-D05-DASH-003 | Time range selector works correctly | P0 | 1h, 6h, 24h, 7d, 30d options |
| FR-D05-DASH-004 | Auto-refresh every 5 minutes | P1 | Dashboard stays current |
| FR-D05-DASH-005 | Dashboard provisioned via config | P0 | JSON in deploy/grafana/ |

### 2.2 Panel Requirements (FR-D05-PNL)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D05-PNL-001 | Indoor air quality panel (PM2.5, CO2) | P0 | Time series with dual Y-axis |
| FR-D05-PNL-002 | Outdoor conditions panel (temp, humidity) | P0 | Time series visualization |
| FR-D05-PNL-003 | Outdoor air quality panel (AQI, PM2.5) | P0 | Time series visualization |
| FR-D05-PNL-004 | State events overlay | P1 | Window state as annotations |
| FR-D05-PNL-005 | Objective thresholds as reference lines | P0 | Threshold lines visible |
| FR-D05-PNL-006 | Lag comparison panel | P1 | Current vs 24h ago overlay |

### 2.3 Query Requirements (FR-D05-QRY)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D05-QRY-001 | Queries use Gold layer aligned view | P0 | All queries from gold.* |
| FR-D05-QRY-002 | Queries filter by time range variable | P0 | $__timeFilter() works |
| FR-D05-QRY-003 | Queries efficient on Pi resources | P0 | < 50ms per panel query |
| FR-D05-QRY-004 | Queries use continuous aggregates | P0 | Not raw Silver tables |

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-D05-PERF)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D05-PERF-001 | Initial dashboard load | < 2 seconds | Browser timing |
| NFR-D05-PERF-002 | Panel refresh | < 500ms per panel | Grafana metrics |
| NFR-D05-PERF-003 | 30-day time range query | < 100ms | Query timing |
| NFR-D05-PERF-004 | Memory during render | < 50MB | Browser memory |

### 3.2 Usability (NFR-D05-USE)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D05-USE-001 | Readable on 1080p display | Yes | Visual inspection |
| NFR-D05-USE-002 | Clear axis labels and legends | Yes | Visual inspection |
| NFR-D05-USE-003 | Consistent color scheme across panels | Yes | Visual inspection |
| NFR-D05-USE-004 | Threshold lines clearly visible | Yes | Visual inspection |

---

## 4. Acceptance Criteria (Gherkin)

### 4.1 Dashboard Loading

```gherkin
Feature: Correlation-Ready Dashboard

  Scenario: Dashboard loads successfully
    Given I am authenticated to Grafana
    When I navigate to the Gold Layer Correlation dashboard
    Then the dashboard should load within 2 seconds
    And all panels should display data

  Scenario: Time range selection works
    Given the dashboard is loaded
    When I select the "Last 7 days" time range
    Then all panels should update to show 7 days of data
    And the query should complete within 500ms

  Scenario: Auto-refresh updates data
    Given the dashboard is loaded with 5-minute auto-refresh
    When 5 minutes passes
    Then all panels should refresh with new data
    And no manual action should be required
```

### 4.2 Panel Visualization

```gherkin
Feature: Dashboard Panels

  Scenario: Indoor air quality panel shows PM2.5 and CO2
    Given the dashboard is loaded
    When I view the Indoor Air Quality panel
    Then I should see PM2.5 on the left Y-axis
    And I should see CO2 on the right Y-axis
    And the healthy_pm25 threshold (12 ug/m3) should be visible as a line
    And the healthy_co2 threshold (800 ppm) should be visible as a line

  Scenario: Outdoor air quality panel shows AQI and PM2.5
    Given the dashboard is loaded
    When I view the Outdoor Air Quality panel
    Then I should see outdoor PM2.5 values
    And I should see outdoor AQI (EPA scale)
    And the outdoor_air_safe constraint threshold (35 ug/m3) should be visible

  Scenario: State events shown as annotations
    Given the dashboard is loaded
    And window state changes occurred in the time range
    When I view the Indoor Air Quality panel
    Then window open events should appear as vertical annotations
    And window close events should be distinguishable
```

### 4.3 Query Efficiency

```gherkin
Feature: Dashboard Query Performance

  Scenario: Queries use Gold layer
    Given I inspect the dashboard panel queries
    Then all queries should reference gold.* tables
    And no queries should reference silver.* tables directly

  Scenario: Queries are efficient
    Given I run EXPLAIN ANALYZE on dashboard queries
    Then all queries should complete in less than 100ms
    And query plans should show index usage
```

---

## 5. Dashboard Layout

### 5.1 Overall Structure

```
+------------------------------------------------------------------+
| Gold Layer Correlation Dashboard                    [Time Range v]|
+------------------------------------------------------------------+
| Row 1: Overview Stats                                             |
| +------------+ +------------+ +------------+ +------------+       |
| | Indoor     | | Outdoor    | | Outdoor    | | Window     |       |
| | PM2.5      | | Temp       | | AQI        | | State      |       |
| | (current)  | | (current)  | | (current)  | | (current)  |       |
| +------------+ +------------+ +------------+ +------------+       |
+------------------------------------------------------------------+
| Row 2: Indoor Air Quality (Height: 250px)                         |
| +--------------------------------------------------------------+ |
| |                    Indoor Air Quality                         | |
| |  PM2.5 [left axis]           CO2 [right axis]                | |
| |  -------- pm25_mean          -------- co2_mean               | |
| |  ........ pm25 threshold     ........ co2 threshold          | |
| +--------------------------------------------------------------+ |
+------------------------------------------------------------------+
| Row 3: Outdoor Conditions (Height: 200px)                         |
| +-----------------------------+ +-----------------------------+   |
| | Outdoor Weather             | | Outdoor Air Quality         |   |
| | Temperature, Humidity       | | PM2.5, AQI                  |   |
| +-----------------------------+ +-----------------------------+   |
+------------------------------------------------------------------+
| Row 4: Correlation Exploration (Height: 250px)                    |
| +--------------------------------------------------------------+ |
| |              Indoor vs Outdoor PM2.5 Overlay                  | |
| |  Indoor PM2.5 (solid)        Outdoor PM2.5 (dashed)          | |
| +--------------------------------------------------------------+ |
+------------------------------------------------------------------+
| Row 5: Lag Features (Height: 200px)                               |
| +-----------------------------+ +-----------------------------+   |
| | PM2.5 Current vs 24h Ago    | | CO2 with 6h Lag             |   |
| +-----------------------------+ +-----------------------------+   |
+------------------------------------------------------------------+
```

### 5.2 Panel Specifications

#### Panel 1: Indoor Air Quality

| Attribute | Value |
|-----------|-------|
| **Type** | Time Series |
| **Height** | 250px |
| **Data Source** | TimescaleDB |
| **Query** | See Section 6.1 |
| **Left Y-Axis** | PM2.5 (ug/m3) |
| **Right Y-Axis** | CO2 (ppm) |
| **Thresholds** | PM2.5 = 12, CO2 = 800 |
| **Colors** | PM2.5 = Blue, CO2 = Green |

#### Panel 2: Outdoor Weather

| Attribute | Value |
|-----------|-------|
| **Type** | Time Series |
| **Height** | 200px |
| **Data Source** | TimescaleDB |
| **Query** | See Section 6.2 |
| **Left Y-Axis** | Temperature (C) |
| **Right Y-Axis** | Humidity (%) |
| **Colors** | Temp = Orange, Humidity = Cyan |

#### Panel 3: Outdoor Air Quality

| Attribute | Value |
|-----------|-------|
| **Type** | Time Series |
| **Height** | 200px |
| **Data Source** | TimescaleDB |
| **Query** | See Section 6.3 |
| **Left Y-Axis** | PM2.5 (ug/m3) |
| **Right Y-Axis** | AQI (EPA scale) |
| **Thresholds** | PM2.5 = 35 (constraint) |
| **Colors** | PM2.5 = Purple, AQI = Red |

#### Panel 4: Indoor vs Outdoor PM2.5 Overlay

| Attribute | Value |
|-----------|-------|
| **Type** | Time Series |
| **Height** | 250px |
| **Data Source** | TimescaleDB |
| **Query** | See Section 6.4 |
| **Y-Axis** | PM2.5 (ug/m3) |
| **Series** | Indoor (solid), Outdoor (dashed) |
| **Purpose** | Visual correlation exploration |

#### Panel 5: Lag Feature Comparison

| Attribute | Value |
|-----------|-------|
| **Type** | Time Series |
| **Height** | 200px |
| **Data Source** | TimescaleDB |
| **Query** | See Section 6.5 |
| **Series** | Current PM2.5, PM2.5 24h ago |
| **Purpose** | Visualize temporal patterns |

---

## 6. Query Specifications

### 6.1 Indoor Air Quality Query

```sql
SELECT
    bucket AS time,
    indoor_pm25 AS "PM2.5",
    indoor_co2 AS "CO2"
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
ORDER BY bucket;
```

**Grafana Configuration**:
- Format: Time series
- Time column: time
- Metric columns: PM2.5, CO2

### 6.2 Outdoor Weather Query

```sql
SELECT
    bucket AS time,
    outdoor_temp AS "Temperature (C)",
    outdoor_humidity AS "Humidity (%)"
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
ORDER BY bucket;
```

### 6.3 Outdoor Air Quality Query

```sql
SELECT
    bucket AS time,
    outdoor_aqi_pm25 AS "Outdoor PM2.5",
    outdoor_aqi_epa AS "EPA AQI"
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
ORDER BY bucket;
```

### 6.4 Indoor vs Outdoor PM2.5 Overlay Query

```sql
SELECT
    bucket AS time,
    indoor_pm25 AS "Indoor PM2.5",
    outdoor_aqi_pm25 AS "Outdoor PM2.5"
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
ORDER BY bucket;
```

### 6.5 Lag Feature Comparison Query

```sql
SELECT
    bucket AS time,
    indoor_pm25 AS "Current PM2.5",
    indoor_pm25_lag_24h AS "PM2.5 (24h ago)"
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
ORDER BY bucket;
```

### 6.6 State Events Annotation Query

```sql
SELECT
    bucket AS time,
    'Window ' || window_state AS text
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom()
  AND bucket <= $__timeTo()
  AND window_state IS NOT NULL
  AND LAG(window_state) OVER (ORDER BY bucket) IS DISTINCT FROM window_state
ORDER BY bucket;
```

---

## 7. Threshold Configuration

### 7.1 Objective Thresholds

Thresholds from domain objectives should appear as reference lines:

| Objective | Metric | Threshold | Color | Line Style |
|-----------|--------|-----------|-------|------------|
| healthy_pm25 | Indoor PM2.5 | 12 ug/m3 | Green | Dashed |
| healthy_co2 | Indoor CO2 | 800 ppm | Green | Dashed |
| outdoor_air_safe | Outdoor PM2.5 | 35 ug/m3 | Yellow | Dotted |

### 7.2 Grafana Threshold Configuration

```json
{
  "fieldConfig": {
    "defaults": {
      "thresholds": {
        "mode": "absolute",
        "steps": [
          { "color": "green", "value": null },
          { "color": "yellow", "value": 12 },
          { "color": "red", "value": 35 }
        ]
      },
      "custom": {
        "thresholdsStyle": {
          "mode": "line"
        }
      }
    }
  }
}
```

---

## 8. Color Scheme

### 8.1 Stream Colors

| Stream | Primary Color | Hex Code |
|--------|---------------|----------|
| Indoor Air (PM2.5) | Blue | #5794F2 |
| Indoor Air (CO2) | Green | #73BF69 |
| Outdoor Weather (Temp) | Orange | #FF9830 |
| Outdoor Weather (Humidity) | Cyan | #5C95BF |
| Outdoor AQI (PM2.5) | Purple | #B877D9 |
| Outdoor AQI (AQI) | Red | #F2495C |
| State Events | Yellow | #FADE2A |

### 8.2 Threshold Colors

| Level | Meaning | Hex Code |
|-------|---------|----------|
| Good | Below objective | #73BF69 (Green) |
| Warning | Approaching threshold | #FF9830 (Orange) |
| Critical | Above threshold | #F2495C (Red) |

---

## 9. Dashboard Provisioning

### 9.1 File Location

```
deploy/grafana/dashboards/gold-layer-correlation.json
```

### 9.2 Provisioning Config

```yaml
# deploy/grafana/provisioning/dashboards/gold-layer.yaml
apiVersion: 1
providers:
  - name: 'Gold Layer Dashboards'
    orgId: 1
    folder: 'Gold Layer'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 30
    options:
      path: /var/lib/grafana/dashboards/gold-layer
```

### 9.3 Dashboard JSON Structure

```json
{
  "dashboard": {
    "id": null,
    "uid": "gold-correlation-v1",
    "title": "Gold Layer Correlation",
    "tags": ["gold", "correlation", "v1.1"],
    "timezone": "browser",
    "refresh": "5m",
    "schemaVersion": 38,
    "version": 1,
    "panels": [
      // Panel definitions...
    ],
    "templating": {
      "list": [
        {
          "name": "datasource",
          "type": "datasource",
          "query": "postgres"
        }
      ]
    },
    "time": {
      "from": "now-24h",
      "to": "now"
    }
  }
}
```

---

## 10. London TDD Interfaces

### 10.1 Interface: Dashboard Query Validation

```rust
/// Validate dashboard queries use Gold layer
pub struct DashboardQueryValidator;

impl DashboardQueryValidator {
    /// Check that query only references gold.* tables
    pub fn validate_uses_gold_layer(&self, query: &str) -> Result<(), ValidationError>;

    /// Check that query uses time filter variable
    pub fn validate_has_time_filter(&self, query: &str) -> Result<(), ValidationError>;

    /// Estimate query performance
    pub fn estimate_query_time(&self, query: &str) -> Duration;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_query_uses_gold_layer() {
        let validator = DashboardQueryValidator::new();

        let valid_query = "SELECT * FROM gold.indoor_air_quality_aligned WHERE bucket >= $__timeFrom()";
        assert!(validator.validate_uses_gold_layer(valid_query).is_ok());

        let invalid_query = "SELECT * FROM silver.air_quality_observations";
        assert!(validator.validate_uses_gold_layer(invalid_query).is_err());
    }

    #[test]
    fn test_query_has_time_filter() {
        let validator = DashboardQueryValidator::new();

        let valid_query = "SELECT * FROM gold.aligned WHERE bucket >= $__timeFrom()";
        assert!(validator.validate_has_time_filter(valid_query).is_ok());

        let missing_filter = "SELECT * FROM gold.aligned";
        assert!(validator.validate_has_time_filter(missing_filter).is_err());
    }
}
```

### 10.2 Interface: Dashboard JSON Validation

```rust
/// Validate provisioned dashboard JSON
pub struct DashboardJsonValidator;

impl DashboardJsonValidator {
    /// Validate dashboard structure
    pub fn validate(&self, json: &Value) -> Result<(), Vec<ValidationError>>;

    /// Check all panels have valid queries
    pub fn validate_panel_queries(&self, json: &Value) -> Result<(), Vec<ValidationError>>;

    /// Check threshold configuration
    pub fn validate_thresholds(&self, json: &Value) -> Result<(), Vec<ValidationError>>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_dashboard_json_valid() {
        let validator = DashboardJsonValidator::new();
        let dashboard_json = load_dashboard_json("gold-layer-correlation.json");

        let result = validator.validate(&dashboard_json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_all_panels_have_gold_queries() {
        let validator = DashboardJsonValidator::new();
        let dashboard_json = load_dashboard_json("gold-layer-correlation.json");

        let result = validator.validate_panel_queries(&dashboard_json);
        assert!(result.is_ok(), "All panels should query gold.* tables");
    }
}
```

---

## 11. Performance Optimization

### 11.1 Query Optimization Guidelines

1. **Always use continuous aggregate**: Query `gold.indoor_air_quality_aligned`, not underlying tables
2. **Include time filter**: Always use `WHERE bucket >= $__timeFrom()`
3. **Limit columns**: Only SELECT needed columns
4. **Avoid subqueries**: Use JOINs in aligned view, not dashboard query

### 11.2 Grafana Optimization

1. **Query caching**: Enable query caching in data source
2. **Min interval**: Set to 1 hour (matches bucket granularity)
3. **Max data points**: Limit to 1000 for performance

### 11.3 Expected Query Plans

```sql
EXPLAIN ANALYZE
SELECT bucket, indoor_pm25, indoor_co2
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '7 days';

-- Expected plan:
-- Index Scan on air_quality_hourly_bucket_idx
-- Filter: (bucket >= (now() - '7 days'::interval))
-- Rows Removed by Filter: 0
-- Execution Time: < 50ms
```

---

## 12. References

- [SCOPE.md](../../SCOPE.md) - Feature v11-011 definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [Grafana Dashboard Best Practices](https://grafana.com/docs/grafana/latest/best-practices/best-practices-for-creating-dashboards/)
- [TimescaleDB Grafana Integration](https://docs.timescale.com/use-timescale/latest/integrations/grafana/)

---

*Specification created: 2026-02-04*
