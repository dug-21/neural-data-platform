//! SQL constants for domain sync operations.
//!
//! All SQL uses parameterized queries ($1, $2, ...) -- never string concatenation.

/// UPSERT domain. $1=domain_id, $2=description, $3=stream_count, $4=config_path
pub const UPSERT_DOMAIN: &str = "\
INSERT INTO data_dictionary.domains (domain_id, description, stream_count, config_path) \
VALUES ($1, $2, $3, $4) \
ON CONFLICT (domain_id) DO UPDATE SET \
description = EXCLUDED.description, \
stream_count = EXCLUDED.stream_count, \
config_path = EXCLUDED.config_path, \
updated_at = NOW()";

/// DELETE domain_streams for a domain. $1=domain_id
pub const DELETE_DOMAIN_STREAMS: &str = "\
DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1";

/// INSERT domain_stream. $1=domain_id, $2=stream_id, $3=alias, $4=role
pub const INSERT_DOMAIN_STREAM: &str = "\
INSERT INTO data_dictionary.domain_streams (domain_id, stream_id, alias, role) \
VALUES ($1, $2, $3, $4)";

/// DELETE objectives for a domain. $1=domain_id
pub const DELETE_OBJECTIVES: &str = "\
DELETE FROM data_dictionary.objectives WHERE domain_id = $1";

/// INSERT objective. $1=objective_id, $2=domain_id, $3=description, $4=target_stream,
/// $5=target_metric, $6=condition, $7=threshold (float8→numeric), $8=threshold_upper (float8→numeric), $9=unit, $10=priority
pub const INSERT_OBJECTIVE: &str = "\
INSERT INTO data_dictionary.objectives \
(objective_id, domain_id, description, target_stream, target_metric, \
condition, threshold, threshold_upper, unit, priority) \
VALUES ($1, $2, $3, $4, $5, $6, CAST($7 AS float8), CAST($8 AS float8), $9, $10)";

/// DELETE constraints for a domain. $1=domain_id
pub const DELETE_CONSTRAINTS: &str = "\
DELETE FROM data_dictionary.constraints WHERE domain_id = $1";

/// INSERT constraint. $1=constraint_id, $2=domain_id, $3=description,
/// $4=constraint_stream, $5=constraint_metric, $6=condition, $7=threshold (float8→numeric), $8=unit
pub const INSERT_CONSTRAINT: &str = "\
INSERT INTO data_dictionary.constraints \
(constraint_id, domain_id, description, constraint_stream, \
constraint_metric, condition, threshold, unit) \
VALUES ($1, $2, $3, $4, $5, $6, CAST($7 AS float8), $8)";
