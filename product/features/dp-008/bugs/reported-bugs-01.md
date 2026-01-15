These are reported bugs/or enhancements to a number of the panels on these existing dashboards

## Pipeline Health

- Panel(s)
    - Air Quality Freshness - Incorrect thresholds.  The threshold for data freshness is set to the stream expectations.. which is correct.  HOWEVER.. our ETL process is currently running every 5 minutes.  So, the guage (and the Air Quality (Indoor) status in the Stream Status Overview) both show red based on the stream.  A better guage should show red on anything greater than 5 minutes.  On this guage, there's not likely much yellow.
    - Recent Temperature Readings shows no data.


## Forecast Accuracy

### Key Issues
the largest issue is that there is a need to add a lead_time type field to have available to assist with forecast accuracy.  We actuall added that to the database as a materialized field on the same record.  This caused the ETL duckdb loading process to fail.  Based on this issue, there are a number of panels on this dashboard reporting no data.  1 additional aspect of this is that we even modified the init script if this needs to be recreated to add that field.(This needs to be removed).

So this needs to be solved somehow, and the panels on this view that require that field need to be updated, and the init script (if making database changes, like a view need to be updated).  