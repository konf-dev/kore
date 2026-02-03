#!/usr/bin/env python3
"""
Baseline Agent - Standard LLM agent with shell access

This represents the "status quo" approach:
- Full shell access (can run any command)
- Python available
- No special language or constraints
- Standard system prompt

Used for benchmarking against Kore agent.

Modes:
- Single task: Set AGENT_GOAL, runs once
- Persistent: Set AGENT_PERSISTENT=true, reads tasks from inbox.txt
"""

import os
import sys
import json
import subprocess
import time
from typing import Optional
from openai import OpenAI

# Ensure unbuffered output
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

# Configuration
GOAL = os.environ.get("AGENT_GOAL", "")
MODEL = os.environ.get("AGENT_MODEL", "gpt-4o")
MAX_ITERATIONS = int(os.environ.get("AGENT_MAX_ITERATIONS", "30"))
PERSISTENT = os.environ.get("AGENT_PERSISTENT", "").lower() == "true"
WORKSPACE = "/workspace"
INBOX = os.path.join(WORKSPACE, "inbox.txt")
DONE_FILE = os.path.join(WORKSPACE, "done.txt")
BASE_URL = os.environ.get("OPENAI_BASE_URL", None)

SYSTEM_PROMPT = """You are an AI assistant that can execute shell commands and write code to accomplish tasks.

You have access to:
- Full shell access (bash)
- Python 3.11
- Node.js
- Common tools: curl, git, jq, wget

To execute a command, respond with a JSON block:
```json
{"action": "shell", "command": "your command here"}
```

To write a file:
```json
{"action": "write", "path": "filename.txt", "content": "file content"}
```

To read a file:
```json
{"action": "read", "path": "filename.txt"}
```

When you have completed the task, respond with:
```json
{"action": "done", "summary": "what you accomplished"}
```

Always explain your thinking before taking an action.
Work in the /workspace directory.
Be efficient and complete the task as quickly as possible.
"""

def execute_shell(command: str) -> dict:
    """Execute a shell command and return result."""
    try:
        result = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=600,  # 10 minutes for ML training
            cwd=WORKSPACE
        )
        return {
            "stdout": result.stdout,
            "stderr": result.stderr,
            "returncode": result.returncode
        }
    except subprocess.TimeoutExpired:
        return {"error": "Command timed out after 600 seconds"}
    except Exception as e:
        return {"error": str(e)}

def write_file(path: str, content: str) -> dict:
    """Write content to a file."""
    try:
        full_path = os.path.join(WORKSPACE, path.lstrip("/"))
        os.makedirs(os.path.dirname(full_path) or WORKSPACE, exist_ok=True)
        with open(full_path, "w") as f:
            f.write(content)
        return {"success": True, "path": full_path}
    except Exception as e:
        return {"error": str(e)}

def read_file(path: str) -> dict:
    """Read content from a file."""
    try:
        full_path = os.path.join(WORKSPACE, path.lstrip("/"))
        with open(full_path, "r") as f:
            return {"content": f.read()}
    except Exception as e:
        return {"error": str(e)}

def extract_json(text: str) -> Optional[dict]:
    """Extract JSON from response text."""
    import re
    
    # Look for ```json blocks first
    match = re.search(r'```json\s*(.*?)\s*```', text, re.DOTALL)
    if match:
        try:
            return json.loads(match.group(1))
        except json.JSONDecodeError:
            pass
    
    # Look for ``` blocks (without json tag)
    match = re.search(r'```\s*(\{.*?\})\s*```', text, re.DOTALL)
    if match:
        try:
            return json.loads(match.group(1))
        except json.JSONDecodeError:
            pass
    
    # Try to find JSON object anywhere in text (greedy match for action objects)
    match = re.search(r'\{"action"\s*:\s*"[^"]+"\s*,?\s*[^}]*\}', text)
    if match:
        try:
            return json.loads(match.group(0))
        except json.JSONDecodeError:
            pass
    
    # Try to find any JSON object on a line
    for line in text.split('\n'):
        line = line.strip()
        if line.startswith('{') and line.endswith('}'):
            try:
                return json.loads(line)
            except json.JSONDecodeError:
                pass
    
    return None

def run_agent():
    """Main agent loop."""
    if not GOAL:
        print("ERROR: AGENT_GOAL not set")
        sys.exit(1)
    
    # Support custom base URL (e.g., OpenRouter)
    if BASE_URL:
        client = OpenAI(base_url=BASE_URL)
    else:
        client = OpenAI()
    
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Your task: {GOAL}"}
    ]
    
    print(f"=" * 70)
    print(f"BASELINE AGENT")
    print(f"Goal: {GOAL}")
    print(f"Model: {MODEL}")
    print(f"Base URL: {BASE_URL or 'default'}")
    print(f"Max iterations: {MAX_ITERATIONS}")
    print(f"=" * 70)
    print()
    
    for iteration in range(MAX_ITERATIONS):
        print(f"--- Iteration {iteration + 1}/{MAX_ITERATIONS} ---")
        
        # Get LLM response
        try:
            response = client.chat.completions.create(
                model=MODEL,
                messages=messages,
                max_tokens=4096,
                temperature=0.7
            )
            assistant_message = response.choices[0].message.content
        except Exception as e:
            print(f"ERROR: LLM call failed: {e}")
            break
        
        print(f"Assistant: {assistant_message[:200]}...")
        messages.append({"role": "assistant", "content": assistant_message})
        
        # Parse action
        action = extract_json(assistant_message)
        
        if not action:
            # No action found, ask for clarification
            messages.append({
                "role": "user", 
                "content": "Please respond with a valid JSON action block."
            })
            continue
        
        action_type = action.get("action")
        
        if action_type == "done":
            print(f"\n✅ TASK COMPLETED")
            print(f"Summary: {action.get('summary', 'No summary provided')}")
            return True
        
        elif action_type == "shell":
            command = action.get("command", "")
            print(f"Executing: {command}")
            result = execute_shell(command)
            print(f"Result: {json.dumps(result)[:200]}...")
            messages.append({
                "role": "user",
                "content": f"Command result:\n```\n{json.dumps(result, indent=2)}\n```"
            })
        
        elif action_type == "write":
            path = action.get("path", "")
            content = action.get("content", "")
            print(f"Writing: {path}")
            result = write_file(path, content)
            print(f"Result: {result}")
            messages.append({
                "role": "user",
                "content": f"Write result: {json.dumps(result)}"
            })
        
        elif action_type == "read":
            path = action.get("path", "")
            print(f"Reading: {path}")
            result = read_file(path)
            content_preview = result.get("content", "")[:200] if "content" in result else str(result)
            print(f"Result: {content_preview}...")
            messages.append({
                "role": "user",
                "content": f"File content:\n```\n{result.get('content', result)}\n```"
            })
        
        else:
            messages.append({
                "role": "user",
                "content": f"Unknown action: {action_type}. Use shell, write, read, or done."
            })
        
        print()
    
    print(f"\n❌ MAX ITERATIONS REACHED")
    return False

def check_inbox():
    """Check inbox for new task, return task or None."""
    if os.path.exists(INBOX):
        try:
            with open(INBOX, "r") as f:
                content = f.read().strip()
            if content:
                # Clear inbox
                with open(INBOX, "w") as f:
                    f.write("")
                return content
        except Exception:
            pass
    return None

def signal_done(summary: str):
    """Write done signal with summary."""
    try:
        with open(DONE_FILE, "w") as f:
            f.write(summary)
        print(f"✅ Done signal written to {DONE_FILE}")
    except Exception as e:
        print(f"❌ Failed to write done signal: {e}")

def run_persistent():
    """Persistent mode - wait for tasks in inbox."""
    print(f"=" * 70)
    print(f"BASELINE AGENT (PERSISTENT MODE)")
    print(f"Model: {MODEL}")
    print(f"Inbox: {INBOX}")
    print(f"Waiting for tasks...")
    print(f"=" * 70)
    print(flush=True)
    
    # Support custom base URL
    if BASE_URL:
        client = OpenAI(base_url=BASE_URL)
    else:
        client = OpenAI()
    
    while True:
        task = check_inbox()
        
        if task:
            print(f"\n📥 NEW TASK: {task}", flush=True)
            
            messages = [
                {"role": "system", "content": SYSTEM_PROMPT + "\n\nYou are in persistent mode. After completing a task, new tasks will be sent."},
                {"role": "user", "content": f"Your task: {task}"}
            ]
            
            # Run task with iterations
            for iteration in range(MAX_ITERATIONS):
                print(f"--- Iteration {iteration + 1}/{MAX_ITERATIONS} ---", flush=True)
                
                try:
                    response = client.chat.completions.create(
                        model=MODEL,
                        messages=messages,
                        max_tokens=4096,
                        temperature=0.7
                    )
                    assistant_message = response.choices[0].message.content
                except Exception as e:
                    print(f"ERROR: LLM call failed: {e}", flush=True)
                    break
                
                print(f"Assistant: {assistant_message[:300]}...", flush=True)
                messages.append({"role": "assistant", "content": assistant_message})
                
                action = extract_json(assistant_message)
                
                if not action:
                    messages.append({
                        "role": "user", 
                        "content": "Please respond with a valid JSON action block."
                    })
                    continue
                
                action_type = action.get("action")
                
                if action_type == "done":
                    summary = action.get('summary', 'Task completed')
                    print(f"\n✅ TASK COMPLETED: {summary}", flush=True)
                    signal_done(summary)
                    break
                
                elif action_type == "shell":
                    command = action.get("command", "")
                    print(f"Executing: {command}", flush=True)
                    result = execute_shell(command)
                    print(f"Result: {json.dumps(result)[:200]}...", flush=True)
                    messages.append({
                        "role": "user",
                        "content": f"Command result:\n```\n{json.dumps(result, indent=2)}\n```"
                    })
                
                elif action_type == "write":
                    path = action.get("path", "")
                    content = action.get("content", "")
                    print(f"Writing: {path}", flush=True)
                    result = write_file(path, content)
                    print(f"Result: {result}", flush=True)
                    messages.append({
                        "role": "user",
                        "content": f"Write result: {json.dumps(result)}"
                    })
                
                elif action_type == "read":
                    path = action.get("path", "")
                    print(f"Reading: {path}", flush=True)
                    result = read_file(path)
                    content_preview = result.get("content", "")[:200] if "content" in result else str(result)
                    print(f"Result: {content_preview}...", flush=True)
                    messages.append({
                        "role": "user",
                        "content": f"File content:\n```\n{result.get('content', result)}\n```"
                    })
                
                else:
                    messages.append({
                        "role": "user",
                        "content": f"Unknown action: {action_type}. Use shell, write, read, or done."
                    })
            else:
                # Max iterations reached
                signal_done("Task incomplete - max iterations reached")
        
        # Poll for new tasks
        time.sleep(1)

if __name__ == "__main__":
    if PERSISTENT:
        run_persistent()
    else:
        success = run_agent()
        sys.exit(0 if success else 1)
