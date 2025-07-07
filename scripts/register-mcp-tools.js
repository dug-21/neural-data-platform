#!/usr/bin/env node

/**
 * Register Neural Trader MCP tools with ruv-swarm
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Load tool definitions
const toolsConfig = JSON.parse(
  fs.readFileSync(path.join(__dirname, '../config/mcp-tools.json'), 'utf8')
);

// Function to register a single tool
function registerTool(tool) {
  console.log(`📝 Registering tool: ${tool.name}`);
  
  const command = `npx ruv-swarm mcp tool register \
    --name "${tool.name}" \
    --description "${tool.description}" \
    --schema '${JSON.stringify(tool.inputSchema)}'`;
  
  try {
    execSync(command, { stdio: 'inherit' });
    console.log(`✅ Successfully registered: ${tool.name}`);
  } catch (error) {
    console.error(`❌ Failed to register ${tool.name}:`, error.message);
  }
}

// Main registration process
async function main() {
  console.log('🚀 Starting Neural Trader MCP tool registration');
  console.log(`📋 Found ${toolsConfig.tools.length} tools to register`);
  
  // Register each tool
  for (const tool of toolsConfig.tools) {
    registerTool(tool);
  }
  
  console.log('\n✨ Registration complete!');
  console.log('\n📡 You can now use these tools in Claude:');
  toolsConfig.tools.forEach(tool => {
    console.log(`   - mcp__neural-trader__${tool.name}`);
  });
}

// Run registration
main().catch(error => {
  console.error('❌ Registration failed:', error);
  process.exit(1);
});