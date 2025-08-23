import datetime

from google.protobuf import any_pb2 as _any_pb2
from google.protobuf import empty_pb2 as _empty_pb2
from google.protobuf import struct_pb2 as _struct_pb2
from google.protobuf import timestamp_pb2 as _timestamp_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ValueType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VALUE_TYPE_UNSPECIFIED: _ClassVar[ValueType]
    VALUE_TYPE_STRING: _ClassVar[ValueType]
    VALUE_TYPE_INT: _ClassVar[ValueType]
    VALUE_TYPE_FLOAT: _ClassVar[ValueType]
    VALUE_TYPE_BOOL: _ClassVar[ValueType]
    VALUE_TYPE_JSON: _ClassVar[ValueType]
    VALUE_TYPE_BINARY: _ClassVar[ValueType]

class ChangeType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CHANGE_TYPE_UNSPECIFIED: _ClassVar[ChangeType]
    CHANGE_TYPE_CREATED: _ClassVar[ChangeType]
    CHANGE_TYPE_UPDATED: _ClassVar[ChangeType]
    CHANGE_TYPE_DELETED: _ClassVar[ChangeType]
VALUE_TYPE_UNSPECIFIED: ValueType
VALUE_TYPE_STRING: ValueType
VALUE_TYPE_INT: ValueType
VALUE_TYPE_FLOAT: ValueType
VALUE_TYPE_BOOL: ValueType
VALUE_TYPE_JSON: ValueType
VALUE_TYPE_BINARY: ValueType
CHANGE_TYPE_UNSPECIFIED: ChangeType
CHANGE_TYPE_CREATED: ChangeType
CHANGE_TYPE_UPDATED: ChangeType
CHANGE_TYPE_DELETED: ChangeType

class GetConfigRequest(_message.Message):
    __slots__ = ("namespace_path", "key", "version", "context", "include_metadata")
    class ContextEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_METADATA_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    key: str
    version: str
    context: _containers.ScalarMap[str, str]
    include_metadata: bool
    def __init__(self, namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., version: _Optional[str] = ..., context: _Optional[_Mapping[str, str]] = ..., include_metadata: bool = ...) -> None: ...

class GetConfigResponse(_message.Message):
    __slots__ = ("success", "namespace_path", "key", "version", "value", "metadata", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    namespace_path: str
    key: str
    version: str
    value: ConfigValue
    metadata: ConfigMetadata
    error_message: str
    def __init__(self, success: bool = ..., namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., version: _Optional[str] = ..., value: _Optional[_Union[ConfigValue, _Mapping]] = ..., metadata: _Optional[_Union[ConfigMetadata, _Mapping]] = ..., error_message: _Optional[str] = ...) -> None: ...

class GetBulkConfigRequest(_message.Message):
    __slots__ = ("namespace_path", "keys", "version", "context", "include_metadata")
    class ContextEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEYS_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    CONTEXT_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_METADATA_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    keys: _containers.RepeatedScalarFieldContainer[str]
    version: str
    context: _containers.ScalarMap[str, str]
    include_metadata: bool
    def __init__(self, namespace_path: _Optional[str] = ..., keys: _Optional[_Iterable[str]] = ..., version: _Optional[str] = ..., context: _Optional[_Mapping[str, str]] = ..., include_metadata: bool = ...) -> None: ...

class GetBulkConfigResponse(_message.Message):
    __slots__ = ("success", "namespace_path", "version", "values", "metadata", "missing_keys", "error_message")
    class ValuesEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: ConfigValue
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[ConfigValue, _Mapping]] = ...) -> None: ...
    class MetadataEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: ConfigMetadata
        def __init__(self, key: _Optional[str] = ..., value: _Optional[_Union[ConfigMetadata, _Mapping]] = ...) -> None: ...
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    VALUES_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    MISSING_KEYS_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    namespace_path: str
    version: str
    values: _containers.MessageMap[str, ConfigValue]
    metadata: _containers.MessageMap[str, ConfigMetadata]
    missing_keys: _containers.RepeatedScalarFieldContainer[str]
    error_message: str
    def __init__(self, success: bool = ..., namespace_path: _Optional[str] = ..., version: _Optional[str] = ..., values: _Optional[_Mapping[str, ConfigValue]] = ..., metadata: _Optional[_Mapping[str, ConfigMetadata]] = ..., missing_keys: _Optional[_Iterable[str]] = ..., error_message: _Optional[str] = ...) -> None: ...

class SetConfigRequest(_message.Message):
    __slots__ = ("namespace_path", "key", "value", "change_reason", "validate_only", "expected_version")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    CHANGE_REASON_FIELD_NUMBER: _ClassVar[int]
    VALIDATE_ONLY_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_VERSION_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    key: str
    value: ConfigValue
    change_reason: str
    validate_only: bool
    expected_version: str
    def __init__(self, namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., value: _Optional[_Union[ConfigValue, _Mapping]] = ..., change_reason: _Optional[str] = ..., validate_only: bool = ..., expected_version: _Optional[str] = ...) -> None: ...

class SetConfigResponse(_message.Message):
    __slots__ = ("success", "namespace_path", "key", "new_version", "validation_errors", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    NEW_VERSION_FIELD_NUMBER: _ClassVar[int]
    VALIDATION_ERRORS_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    namespace_path: str
    key: str
    new_version: str
    validation_errors: _containers.RepeatedScalarFieldContainer[str]
    error_message: str
    def __init__(self, success: bool = ..., namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., new_version: _Optional[str] = ..., validation_errors: _Optional[_Iterable[str]] = ..., error_message: _Optional[str] = ...) -> None: ...

class WatchConfigRequest(_message.Message):
    __slots__ = ("namespace_path", "keys", "include_initial_values")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEYS_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_INITIAL_VALUES_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    keys: _containers.RepeatedScalarFieldContainer[str]
    include_initial_values: bool
    def __init__(self, namespace_path: _Optional[str] = ..., keys: _Optional[_Iterable[str]] = ..., include_initial_values: bool = ...) -> None: ...

class ConfigChangeEvent(_message.Message):
    __slots__ = ("namespace_path", "key", "change_type", "old_value", "new_value", "timestamp", "change_reason", "changed_by", "version")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    CHANGE_TYPE_FIELD_NUMBER: _ClassVar[int]
    OLD_VALUE_FIELD_NUMBER: _ClassVar[int]
    NEW_VALUE_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    CHANGE_REASON_FIELD_NUMBER: _ClassVar[int]
    CHANGED_BY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    key: str
    change_type: ChangeType
    old_value: ConfigValue
    new_value: ConfigValue
    timestamp: _timestamp_pb2.Timestamp
    change_reason: str
    changed_by: str
    version: str
    def __init__(self, namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., change_type: _Optional[_Union[ChangeType, str]] = ..., old_value: _Optional[_Union[ConfigValue, _Mapping]] = ..., new_value: _Optional[_Union[ConfigValue, _Mapping]] = ..., timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., change_reason: _Optional[str] = ..., changed_by: _Optional[str] = ..., version: _Optional[str] = ...) -> None: ...

class GetSchemaRequest(_message.Message):
    __slots__ = ("namespace_path", "schema_version")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    schema_version: str
    def __init__(self, namespace_path: _Optional[str] = ..., schema_version: _Optional[str] = ...) -> None: ...

class GetSchemaResponse(_message.Message):
    __slots__ = ("success", "namespace_path", "schema_version", "json_schema", "supported_versions", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    JSON_SCHEMA_FIELD_NUMBER: _ClassVar[int]
    SUPPORTED_VERSIONS_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    namespace_path: str
    schema_version: str
    json_schema: str
    supported_versions: _containers.RepeatedScalarFieldContainer[str]
    error_message: str
    def __init__(self, success: bool = ..., namespace_path: _Optional[str] = ..., schema_version: _Optional[str] = ..., json_schema: _Optional[str] = ..., supported_versions: _Optional[_Iterable[str]] = ..., error_message: _Optional[str] = ...) -> None: ...

class ConfigValue(_message.Message):
    __slots__ = ("type", "string_value", "int_value", "float_value", "bool_value", "json_value", "binary_value")
    TYPE_FIELD_NUMBER: _ClassVar[int]
    STRING_VALUE_FIELD_NUMBER: _ClassVar[int]
    INT_VALUE_FIELD_NUMBER: _ClassVar[int]
    FLOAT_VALUE_FIELD_NUMBER: _ClassVar[int]
    BOOL_VALUE_FIELD_NUMBER: _ClassVar[int]
    JSON_VALUE_FIELD_NUMBER: _ClassVar[int]
    BINARY_VALUE_FIELD_NUMBER: _ClassVar[int]
    type: ValueType
    string_value: str
    int_value: int
    float_value: float
    bool_value: bool
    json_value: _struct_pb2.Struct
    binary_value: bytes
    def __init__(self, type: _Optional[_Union[ValueType, str]] = ..., string_value: _Optional[str] = ..., int_value: _Optional[int] = ..., float_value: _Optional[float] = ..., bool_value: bool = ..., json_value: _Optional[_Union[_struct_pb2.Struct, _Mapping]] = ..., binary_value: _Optional[bytes] = ...) -> None: ...

class ConfigMetadata(_message.Message):
    __slots__ = ("created_at", "modified_at", "created_by", "modified_by", "version", "tags", "annotations")
    class AnnotationsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_AT_FIELD_NUMBER: _ClassVar[int]
    CREATED_BY_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_BY_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    TAGS_FIELD_NUMBER: _ClassVar[int]
    ANNOTATIONS_FIELD_NUMBER: _ClassVar[int]
    created_at: _timestamp_pb2.Timestamp
    modified_at: _timestamp_pb2.Timestamp
    created_by: str
    modified_by: str
    version: str
    tags: _containers.RepeatedScalarFieldContainer[str]
    annotations: _containers.ScalarMap[str, str]
    def __init__(self, created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., modified_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., created_by: _Optional[str] = ..., modified_by: _Optional[str] = ..., version: _Optional[str] = ..., tags: _Optional[_Iterable[str]] = ..., annotations: _Optional[_Mapping[str, str]] = ...) -> None: ...

class HealthStatus(_message.Message):
    __slots__ = ("healthy", "status", "details", "timestamp", "version")
    class DetailsEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    HEALTHY_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    DETAILS_FIELD_NUMBER: _ClassVar[int]
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    healthy: bool
    status: str
    details: _containers.ScalarMap[str, str]
    timestamp: _timestamp_pb2.Timestamp
    version: str
    def __init__(self, healthy: bool = ..., status: _Optional[str] = ..., details: _Optional[_Mapping[str, str]] = ..., timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., version: _Optional[str] = ...) -> None: ...

class ValidationError(_message.Message):
    __slots__ = ("field_path", "error_code", "message", "expected_value", "actual_value")
    FIELD_PATH_FIELD_NUMBER: _ClassVar[int]
    ERROR_CODE_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    EXPECTED_VALUE_FIELD_NUMBER: _ClassVar[int]
    ACTUAL_VALUE_FIELD_NUMBER: _ClassVar[int]
    field_path: str
    error_code: str
    message: str
    expected_value: _any_pb2.Any
    actual_value: _any_pb2.Any
    def __init__(self, field_path: _Optional[str] = ..., error_code: _Optional[str] = ..., message: _Optional[str] = ..., expected_value: _Optional[_Union[_any_pb2.Any, _Mapping]] = ..., actual_value: _Optional[_Union[_any_pb2.Any, _Mapping]] = ...) -> None: ...

class ListNamespacesRequest(_message.Message):
    __slots__ = ("prefix", "include_stats")
    PREFIX_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_STATS_FIELD_NUMBER: _ClassVar[int]
    prefix: str
    include_stats: bool
    def __init__(self, prefix: _Optional[str] = ..., include_stats: bool = ...) -> None: ...

class ListNamespacesResponse(_message.Message):
    __slots__ = ("success", "namespaces", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    NAMESPACES_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    namespaces: _containers.RepeatedCompositeFieldContainer[NamespaceInfo]
    error_message: str
    def __init__(self, success: bool = ..., namespaces: _Optional[_Iterable[_Union[NamespaceInfo, _Mapping]]] = ..., error_message: _Optional[str] = ...) -> None: ...

class NamespaceInfo(_message.Message):
    __slots__ = ("path", "description", "key_count", "last_modified", "tags", "schema_version")
    PATH_FIELD_NUMBER: _ClassVar[int]
    DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    KEY_COUNT_FIELD_NUMBER: _ClassVar[int]
    LAST_MODIFIED_FIELD_NUMBER: _ClassVar[int]
    TAGS_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    path: str
    description: str
    key_count: int
    last_modified: _timestamp_pb2.Timestamp
    tags: _containers.RepeatedScalarFieldContainer[str]
    schema_version: str
    def __init__(self, path: _Optional[str] = ..., description: _Optional[str] = ..., key_count: _Optional[int] = ..., last_modified: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., tags: _Optional[_Iterable[str]] = ..., schema_version: _Optional[str] = ...) -> None: ...

class GetNamespaceInfoRequest(_message.Message):
    __slots__ = ("namespace_path", "include_keys")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_KEYS_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    include_keys: bool
    def __init__(self, namespace_path: _Optional[str] = ..., include_keys: bool = ...) -> None: ...

class GetNamespaceInfoResponse(_message.Message):
    __slots__ = ("success", "info", "keys", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    INFO_FIELD_NUMBER: _ClassVar[int]
    KEYS_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    info: NamespaceInfo
    keys: _containers.RepeatedScalarFieldContainer[str]
    error_message: str
    def __init__(self, success: bool = ..., info: _Optional[_Union[NamespaceInfo, _Mapping]] = ..., keys: _Optional[_Iterable[str]] = ..., error_message: _Optional[str] = ...) -> None: ...

class ValidateConfigRequest(_message.Message):
    __slots__ = ("namespace_path", "key", "value", "schema_version")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    key: str
    value: ConfigValue
    schema_version: str
    def __init__(self, namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., value: _Optional[_Union[ConfigValue, _Mapping]] = ..., schema_version: _Optional[str] = ...) -> None: ...

class ValidateConfigResponse(_message.Message):
    __slots__ = ("valid", "errors", "schema_version")
    VALID_FIELD_NUMBER: _ClassVar[int]
    ERRORS_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_VERSION_FIELD_NUMBER: _ClassVar[int]
    valid: bool
    errors: _containers.RepeatedCompositeFieldContainer[ValidationError]
    schema_version: str
    def __init__(self, valid: bool = ..., errors: _Optional[_Iterable[_Union[ValidationError, _Mapping]]] = ..., schema_version: _Optional[str] = ...) -> None: ...

class GetAuditTrailRequest(_message.Message):
    __slots__ = ("namespace_path", "key", "start_time", "end_time", "limit", "next_token")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    START_TIME_FIELD_NUMBER: _ClassVar[int]
    END_TIME_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    NEXT_TOKEN_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    key: str
    start_time: _timestamp_pb2.Timestamp
    end_time: _timestamp_pb2.Timestamp
    limit: int
    next_token: str
    def __init__(self, namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., start_time: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., end_time: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., limit: _Optional[int] = ..., next_token: _Optional[str] = ...) -> None: ...

class GetAuditTrailResponse(_message.Message):
    __slots__ = ("success", "entries", "next_token", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    NEXT_TOKEN_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    entries: _containers.RepeatedCompositeFieldContainer[AuditEntry]
    next_token: str
    error_message: str
    def __init__(self, success: bool = ..., entries: _Optional[_Iterable[_Union[AuditEntry, _Mapping]]] = ..., next_token: _Optional[str] = ..., error_message: _Optional[str] = ...) -> None: ...

class AuditEntry(_message.Message):
    __slots__ = ("timestamp", "namespace_path", "key", "change_type", "old_value", "new_value", "changed_by", "change_reason", "version", "session_id")
    TIMESTAMP_FIELD_NUMBER: _ClassVar[int]
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    KEY_FIELD_NUMBER: _ClassVar[int]
    CHANGE_TYPE_FIELD_NUMBER: _ClassVar[int]
    OLD_VALUE_FIELD_NUMBER: _ClassVar[int]
    NEW_VALUE_FIELD_NUMBER: _ClassVar[int]
    CHANGED_BY_FIELD_NUMBER: _ClassVar[int]
    CHANGE_REASON_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    SESSION_ID_FIELD_NUMBER: _ClassVar[int]
    timestamp: _timestamp_pb2.Timestamp
    namespace_path: str
    key: str
    change_type: ChangeType
    old_value: ConfigValue
    new_value: ConfigValue
    changed_by: str
    change_reason: str
    version: str
    session_id: str
    def __init__(self, timestamp: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., namespace_path: _Optional[str] = ..., key: _Optional[str] = ..., change_type: _Optional[_Union[ChangeType, str]] = ..., old_value: _Optional[_Union[ConfigValue, _Mapping]] = ..., new_value: _Optional[_Union[ConfigValue, _Mapping]] = ..., changed_by: _Optional[str] = ..., change_reason: _Optional[str] = ..., version: _Optional[str] = ..., session_id: _Optional[str] = ...) -> None: ...

class BackupNamespaceRequest(_message.Message):
    __slots__ = ("namespace_path", "backup_name", "include_metadata")
    NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    BACKUP_NAME_FIELD_NUMBER: _ClassVar[int]
    INCLUDE_METADATA_FIELD_NUMBER: _ClassVar[int]
    namespace_path: str
    backup_name: str
    include_metadata: bool
    def __init__(self, namespace_path: _Optional[str] = ..., backup_name: _Optional[str] = ..., include_metadata: bool = ...) -> None: ...

class BackupNamespaceResponse(_message.Message):
    __slots__ = ("success", "backup_id", "created_at", "config_count", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    BACKUP_ID_FIELD_NUMBER: _ClassVar[int]
    CREATED_AT_FIELD_NUMBER: _ClassVar[int]
    CONFIG_COUNT_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    backup_id: str
    created_at: _timestamp_pb2.Timestamp
    config_count: int
    error_message: str
    def __init__(self, success: bool = ..., backup_id: _Optional[str] = ..., created_at: _Optional[_Union[datetime.datetime, _timestamp_pb2.Timestamp, _Mapping]] = ..., config_count: _Optional[int] = ..., error_message: _Optional[str] = ...) -> None: ...

class RestoreNamespaceRequest(_message.Message):
    __slots__ = ("backup_id", "target_namespace_path", "overwrite_existing", "dry_run")
    BACKUP_ID_FIELD_NUMBER: _ClassVar[int]
    TARGET_NAMESPACE_PATH_FIELD_NUMBER: _ClassVar[int]
    OVERWRITE_EXISTING_FIELD_NUMBER: _ClassVar[int]
    DRY_RUN_FIELD_NUMBER: _ClassVar[int]
    backup_id: str
    target_namespace_path: str
    overwrite_existing: bool
    dry_run: bool
    def __init__(self, backup_id: _Optional[str] = ..., target_namespace_path: _Optional[str] = ..., overwrite_existing: bool = ..., dry_run: bool = ...) -> None: ...

class RestoreNamespaceResponse(_message.Message):
    __slots__ = ("success", "restored_count", "conflicts", "error_message")
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    RESTORED_COUNT_FIELD_NUMBER: _ClassVar[int]
    CONFLICTS_FIELD_NUMBER: _ClassVar[int]
    ERROR_MESSAGE_FIELD_NUMBER: _ClassVar[int]
    success: bool
    restored_count: int
    conflicts: _containers.RepeatedScalarFieldContainer[str]
    error_message: str
    def __init__(self, success: bool = ..., restored_count: _Optional[int] = ..., conflicts: _Optional[_Iterable[str]] = ..., error_message: _Optional[str] = ...) -> None: ...
