# air-012: Home Assistant Integration

## Home Assistant API Access

**Verified working API access to query device state:**

```bash
curl -X GET -H "Authorization: Bearer $HASS_TOKEN" http://192.168.52.221:8123/api/states/<entity_id>
```

**Example - Query RPi Power Status:**
```bash
curl -X GET -H "Authorization: Bearer $HASS_TOKEN" http://192.168.52.221:8123/api/states/binary_sensor.rpi_power_status
```

**Response format:**
```json
{
  "entity_id": "binary_sensor.rpi_power_status",
  "state": "off",
  "attributes": {
    "device_class": "problem",
    "icon": "mdi:raspberry-pi",
    "friendly_name": "RPi Power status"
  },
  "last_changed": "2025-12-31T12:31:02.467501+00:00",
  "last_reported": "2025-12-31T12:31:32.486649+00:00",
  "last_updated": "2025-12-31T12:31:02.467501+00:00",
  "context": {
    "id": "01KDT67SA30AQYHP9CY2BYPVZV",
    "parent_id": null,
    "user_id": null
  }
}
```

**Configuration:**
- Home Assistant IP: `192.168.52.221`
- Port: `8123` (HTTP, not HTTPS)
- Authentication: Long-lived access token via `$HASS_TOKEN` environment variable
- API Endpoint: `/api/states/<entity_id>`

---

## Scope

_TODO: Define feature scope_

## Acceptance Criteria

_TODO: Define acceptance criteria_
