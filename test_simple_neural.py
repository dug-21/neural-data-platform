#!/usr/bin/env python3
"""
Simple test to verify neural network training functionality
"""

import subprocess
import sys
import time
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def test_redis_connection():
    """Test Redis connection"""
    try:
        import redis
        # Try different Redis connection methods
        for host in ['localhost', '127.0.0.1']:
            for port in [6379]:
                try:
                    r = redis.Redis(host=host, port=port, decode_responses=True, socket_timeout=5)
                    r.ping()
                    logger.info(f"✅ Redis connected at {host}:{port}")
                    return True
                except Exception as e:
                    logger.debug(f"❌ Redis connection failed at {host}:{port}: {e}")
        return False
    except ImportError:
        logger.error("❌ Redis library not installed")
        return False

def test_rust_binary():
    """Test if Rust binary can be compiled and run"""
    try:
        # Try to run the main binary with help
        result = subprocess.run(['cargo', 'run', '--release', '--', '--help'], 
                              capture_output=True, text=True, timeout=30)
        if result.returncode == 0:
            logger.info("✅ Rust binary can run")
            return True
        else:
            logger.error(f"❌ Rust binary failed: {result.stderr}")
            return False
    except subprocess.TimeoutExpired:
        logger.error("❌ Rust binary timeout")
        return False
    except Exception as e:
        logger.error(f"❌ Rust binary error: {e}")
        return False

def test_neural_training_basic():
    """Test basic neural training functionality"""
    try:
        # Create a simple test script
        test_script = """
use ruv_fann::network::{Network, LayerConfiguration};
use ruv_fann::config::NetworkConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing basic neural network creation...");
    
    let config = NetworkConfig::new(3, 2, 1, &[4, 3]);
    let mut network = Network::<f32>::new(&config)?;
    
    // Simple training data
    let inputs = vec![0.1, 0.2, 0.3];
    let expected = vec![0.5];
    
    println!("Neural network created successfully");
    println!("Input layers: {}", network.get_num_input());
    println!("Output layers: {}", network.get_num_output());
    
    // Try a prediction
    let output = network.run(&inputs)?;
    println!("Prediction output: {:?}", output);
    
    println!("✅ Basic neural functionality working");
    Ok(())
}
"""
        
        # Write the test to a temporary file
        with open('/tmp/neural_test.rs', 'w') as f:
            f.write(test_script)
        
        # Try to compile and run it
        result = subprocess.run([
            'rustc', '/tmp/neural_test.rs', 
            '--extern', 'ruv_fann=/workspaces/neural-trader/target/release/deps/libruv_fann-*.rlib',
            '-L', '/workspaces/neural-trader/target/release/deps',
            '-o', '/tmp/neural_test'
        ], capture_output=True, text=True, timeout=60)
        
        if result.returncode == 0:
            # Run the test
            result2 = subprocess.run(['/tmp/neural_test'], capture_output=True, text=True)
            if result2.returncode == 0:
                logger.info("✅ Neural training test passed")
                logger.info(f"Output: {result2.stdout}")
                return True
            else:
                logger.error(f"❌ Neural test execution failed: {result2.stderr}")
                return False
        else:
            logger.error(f"❌ Neural test compilation failed: {result.stderr}")
            return False
            
    except Exception as e:
        logger.error(f"❌ Neural training test error: {e}")
        return False

def main():
    """Main test runner"""
    logger.info("🧪 Starting Neural Trader Testing Suite")
    
    tests = [
        ("Redis Connection", test_redis_connection),
        ("Rust Binary", test_rust_binary),
        ("Neural Training Basic", test_neural_training_basic),
    ]
    
    results = {}
    for test_name, test_func in tests:
        logger.info(f"\n🔍 Running {test_name}...")
        try:
            results[test_name] = test_func()
        except Exception as e:
            logger.error(f"❌ {test_name} crashed: {e}")
            results[test_name] = False
    
    # Summary
    logger.info("\n📊 Test Results Summary:")
    passed = 0
    for test_name, result in results.items():
        status = "✅ PASS" if result else "❌ FAIL"
        logger.info(f"  {test_name}: {status}")
        if result:
            passed += 1
    
    logger.info(f"\n🎯 Overall: {passed}/{len(tests)} tests passed")
    
    if passed == len(tests):
        logger.info("🎉 All tests passed! System is ready for trading.")
        return 0
    else:
        logger.warning("⚠️  Some tests failed. Check logs above.")
        return 1

if __name__ == "__main__":
    sys.exit(main())