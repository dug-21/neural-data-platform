"""
Configuration Store Client Example
Demonstrates usage of the generated gRPC stubs
"""

import grpc
import config_store_pb2
import config_store_pb2_grpc


class ConfigClient:
    """Example configuration client implementation"""
    
    def __init__(self, server_address: str = "localhost:50051"):
        self.channel = grpc.insecure_channel(server_address)
        self.stub = config_store_pb2_grpc.ConfigStoreServiceStub(self.channel)
    
    def get_config(self, namespace_path: str, key: str, version: str = None):
        """Get a configuration value"""
        request = config_store_pb2.GetConfigRequest(
            namespace_path=namespace_path,
            key=key,
            version=version or "",
            include_metadata=True
        )
        
        try:
            response = self.stub.GetConfig(request)
            if response.success:
                return response.value, response.metadata
            else:
                raise ValueError(f"Config retrieval failed: {response.error_message}")
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def set_config(self, namespace_path: str, key: str, value, change_reason: str):
        """Set a configuration value"""
        # Create ConfigValue based on Python type
        config_value = config_store_pb2.ConfigValue()
        
        if isinstance(value, str):
            config_value.type = config_store_pb2.VALUE_TYPE_STRING
            config_value.string_value = value
        elif isinstance(value, bool):
            config_value.type = config_store_pb2.VALUE_TYPE_BOOL
            config_value.bool_value = value
        elif isinstance(value, int):
            config_value.type = config_store_pb2.VALUE_TYPE_INT
            config_value.int_value = value
        elif isinstance(value, float):
            config_value.type = config_store_pb2.VALUE_TYPE_FLOAT
            config_value.float_value = value
        else:
            # For complex objects, use JSON
            import json
            from google.protobuf.struct_pb2 import Struct
            config_value.type = config_store_pb2.VALUE_TYPE_JSON
            config_value.json_value.update(json.loads(json.dumps(value)))
        
        request = config_store_pb2.SetConfigRequest(
            namespace_path=namespace_path,
            key=key,
            value=config_value,
            change_reason=change_reason
        )
        
        try:
            response = self.stub.SetConfig(request)
            if response.success:
                return response.new_version
            else:
                raise ValueError(f"Config update failed: {response.error_message}")
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def watch_config(self, namespace_path: str, keys: list = None):
        """Watch for configuration changes"""
        request = config_store_pb2.WatchConfigRequest(
            namespace_path=namespace_path,
            keys=keys or [],
            include_initial_values=True
        )
        
        try:
            for event in self.stub.WatchConfig(request):
                yield event
        except grpc.RpcError as e:
            raise ConnectionError(f"gRPC error: {e}")
    
    def close(self):
        """Close the gRPC channel"""
        self.channel.close()


# Example usage
if __name__ == "__main__":
    client = ConfigClient()
    
    try:
        # Get configuration
        value, metadata = client.get_config(
            "/neural-trading/data-ingestion",
            "sources.primary.symbols"
        )
        print(f"Config value: {value}")
        
        # Set configuration
        new_version = client.set_config(
            "/neural-trading/data-ingestion",
            "sources.primary.rate_limits.requests_per_minute",
            250,
            "Updated rate limit for better performance"
        )
        print(f"Updated to version: {new_version}")
        
    finally:
        client.close()
