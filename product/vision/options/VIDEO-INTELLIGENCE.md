# Option: Video Intelligence Integration

> **Status:** Future consideration
> **Created:** 2026-02-03
> **Priority:** Not active

---

## Summary

Extend the platform to ingest video streams and extract events/metrics for correlation discovery. Video-derived data (person detected, motion events, counts) becomes another stream type feeding the same intelligence engine.

---

## Feasibility

| Capability | Pi 5 Native | With Accelerator ($60-70) |
|------------|-------------|---------------------------|
| Object detection | 2-5 fps | 15-30 fps |
| Concurrent streams | 1-2 cameras | 4-8 cameras |
| Model size | ~6MB (YOLO-tiny) | ~6MB |

**Hardware options:**
- Google Coral USB ($60) - 4 TOPS
- Hailo-8L M.2 ($70) - 13 TOPS

---

## Architecture Fit

```
Camera → Object Detection → Events/Metrics → Bronze → Correlation Discovery
                │
                ├── person_detected (state stream)
                ├── motion_zone_X (state stream)
                ├── person_count_hourly (continuous)
                └── dwell_time_avg (continuous)
```

Video events correlate with other sensors:
- "person in kitchen" → "CO2 spike 5 min later"
- "entrance motion + time" → "sales next hour"
- "no bedroom motion >12hrs" → "fall risk"

---

## Use Cases

| Industry | Application |
|----------|-------------|
| Retail | Traffic → sales correlation, dwell → conversion |
| Healthcare | Fall detection, wandering, activity patterns |
| Manufacturing | Visual QA, safety compliance |
| Agriculture | Pest detection, livestock behavior |
| Building | Occupancy for HVAC, security anomalies |

---

## Two Approaches

**A) Video as sensor (events only)**
- Extract metrics, discard frames
- ~10-50GB/year storage
- Privacy-preserving
- Fits existing architecture

**B) Video with retention**
- 24-48 hour buffer
- Event-triggered clip extraction
- More storage, more privacy considerations
- Enables incident review

---

## Cost Impact

| Config | Hardware | Storage/Year |
|--------|----------|--------------|
| Sensor-only | $75 | ~1GB |
| +Video (events) | $135-145 | ~10-50GB |
| +Video (retention) | $135-145 | ~500GB+ |

---

## Decision Criteria

Revisit when:
- Core platform (v1.3) is stable
- Customer demand validated
- Specific vertical requires it (retail, healthcare)

---

*Preserved for future consideration*
