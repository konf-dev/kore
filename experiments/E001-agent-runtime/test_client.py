#!/usr/bin/env python3
"""
Simple test client for E001 Agent Runtime.

Usage:
    python test_client.py
"""

import httpx

SERVER = "http://127.0.0.1:3000"

def test_api():
    print("\n🧪 Testing Kore Agent Runtime API\n")
    
    with httpx.Client() as client:
        # 1. Health check
        print("1. Health check")
        r = client.get(f"{SERVER}/health")
        print(f"   {r.json()}\n")
        
        # 2. List profiles
        print("2. Available profiles")
        r = client.get(f"{SERVER}/profiles")
        for p in r.json():
            print(f"   - {p['name']}: {p['description']}")
        print()
        
        # 3. Analyze pure code
        print("3. Analyze pure code: '1 2 +'")
        r = client.post(f"{SERVER}/analyze", json={"code": "1 2 +"})
        print(f"   {r.json()}\n")
        
        # 4. Analyze IO code
        print("4. Analyze IO code: '\"hello\" print'")
        r = client.post(f"{SERVER}/analyze", json={"code": '"hello" print'})
        print(f"   {r.json()}\n")
        
        # 5. Execute pure code with pure profile
        print("5. Execute '1 2 +' with pure profile")
        r = client.post(f"{SERVER}/execute", json={
            "code": "1 2 +",
            "profile": "pure"
        })
        print(f"   Status: {r.status_code}")
        print(f"   {r.json()}\n")
        
        # 6. Try to execute IO code with pure profile (should fail)
        print("6. Execute '\"hello\" print' with pure profile (should fail)")
        r = client.post(f"{SERVER}/execute", json={
            "code": '"hello" print',
            "profile": "pure"
        })
        print(f"   Status: {r.status_code}")
        print(f"   {r.json()}\n")
        
        # 7. Create session
        print("7. Create session with local-io profile")
        r = client.post(f"{SERVER}/sessions", json={"profile": "local-io"})
        session = r.json()
        print(f"   Session ID: {session['id']}\n")
        
        # 8. Execute with session
        print("8. Execute with session")
        r = client.post(f"{SERVER}/execute", json={
            "code": "[ 1 2 3 4 5 ] [ 2 * ] map sum",
            "profile": "local-io",
            "session_id": session["id"]
        })
        print(f"   {r.json()}\n")
        
        # 9. Check session
        print("9. Check session")
        r = client.get(f"{SERVER}/sessions/{session['id']}")
        print(f"   {r.json()}\n")
        
        print("✅ All tests completed!")

if __name__ == "__main__":
    test_api()
