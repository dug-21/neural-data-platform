#!/usr/bin/env python3
"""Debug WebSocket connection to see what messages we're receiving."""

import asyncio
import json
import websockets
import os
from datetime import datetime

async def debug_alpaca_websocket():
    api_key = os.environ.get('ALPACA_API_KEY')
    api_secret = os.environ.get('ALPACA_API_SECRET')
    
    if not api_key or not api_secret:
        print("Missing API credentials")
        return
    
    ws_url = "wss://stream.data.alpaca.markets/v2/iex"
    symbols = ["AAPL", "MSFT"]
    
    print(f"Connecting to {ws_url}...")
    
    async with websockets.connect(ws_url) as websocket:
        # Connection message
        msg = await websocket.recv()
        print(f"1. Connection: {msg}")
        
        # Send auth
        auth = {"action": "auth", "key": api_key, "secret": api_secret}
        await websocket.send(json.dumps(auth))
        print(f"2. Sent auth: {json.dumps(auth)}")
        
        # Auth response
        msg = await websocket.recv()
        print(f"3. Auth response: {msg}")
        
        # Subscribe
        sub = {"action": "subscribe", "bars": symbols}
        await websocket.send(json.dumps(sub))
        print(f"4. Sent subscribe: {json.dumps(sub)}")
        
        # Listen for messages
        print("\n5. Listening for messages...")
        message_count = 0
        start_time = datetime.now()
        
        while message_count < 20 and (datetime.now() - start_time).seconds < 300:
            try:
                msg = await asyncio.wait_for(websocket.recv(), timeout=5.0)
                message_count += 1
                print(f"\nMessage {message_count}: {msg[:200]}...")
                
                # Parse and show message type
                try:
                    data = json.loads(msg)
                    if isinstance(data, list):
                        for item in data:
                            print(f"  - Type: {item.get('T', 'unknown')}, Symbol: {item.get('S', 'N/A')}")
                    else:
                        print(f"  - Type: {data.get('T', 'unknown')}")
                except:
                    pass
                    
            except asyncio.TimeoutError:
                print(".", end="", flush=True)
            except Exception as e:
                print(f"\nError: {e}")
                break
        
        print(f"\n\nReceived {message_count} messages in {(datetime.now() - start_time).seconds} seconds")

if __name__ == "__main__":
    asyncio.run(debug_alpaca_websocket())