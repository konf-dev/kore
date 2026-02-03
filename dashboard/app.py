#!/usr/bin/env python3
"""
Kore Benchmark Dashboard
A minimal portal for managing Kore vs Baseline experiments
"""

import os
import json
import subprocess
import asyncio
from datetime import datetime
from pathlib import Path
from typing import Optional
import threading

from fastapi import FastAPI, WebSocket, WebSocketDisconnect, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, FileResponse
from pydantic import BaseModel
import uvicorn

app = FastAPI(title="Kore Benchmark Dashboard")

# Configuration
KORE_ROOT = Path(__file__).parent.parent
BENCHMARKS_DIR = KORE_ROOT / "benchmarks" / "dashboard"
PROMPTS_DIR = KORE_ROOT

# State
class ExperimentState:
    def __init__(self):
        self.running = False
        self.paused = False
        self.kore_container: Optional[str] = None
        self.baseline_container: Optional[str] = None
        self.current_task: Optional[str] = None
        self.task_history: list = []
        self.run_id: Optional[str] = None
        self.run_dir: Optional[Path] = None
        self.chat_history: dict = {"kore": [], "baseline": []}  # Chat history per agent

state = ExperimentState()

# Models
class TaskRequest(BaseModel):
    task: str

class StartRequest(BaseModel):
    model: str = "google/gemini-2.5-flash-preview"

class ChatRequest(BaseModel):
    message: str
    agent: str  # "kore" or "baseline"

# WebSocket connections for live logs
log_connections: list[WebSocket] = []

async def broadcast_log(message: str, source: str = "system"):
    """Send log message to all connected clients"""
    data = json.dumps({
        "timestamp": datetime.now().isoformat(),
        "source": source,
        "message": message
    })
    for ws in log_connections[:]:
        try:
            await ws.send_text(data)
        except:
            log_connections.remove(ws)

def run_cmd(cmd: str, capture: bool = True) -> tuple[int, str]:
    """Run shell command"""
    result = subprocess.run(cmd, shell=True, capture_output=capture, text=True)
    return result.returncode, result.stdout + result.stderr

def is_container_running(name: str) -> bool:
    """Check if a Docker container is running"""
    if not name:
        return False
    code, out = run_cmd(f"docker ps -q -f name=^{name}$")
    return bool(out.strip())

def is_container_exists(name: str) -> bool:
    """Check if a Docker container exists (running or stopped)"""
    if not name:
        return False
    code, out = run_cmd(f"docker ps -aq -f name=^{name}$")
    return bool(out.strip())

def get_env():
    """Load environment variables"""
    env_file = KORE_ROOT / ".env"
    env = os.environ.copy()
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            if "=" in line and not line.startswith("#"):
                key, val = line.split("=", 1)
                env[key.strip()] = val.strip().strip('"')
    return env

# API Routes
@app.get("/", response_class=HTMLResponse)
async def index():
    return FileResponse(Path(__file__).parent / "static" / "index.html")

@app.get("/api/status")
async def get_status():
    """Get current experiment status"""
    # Actually check Docker container states
    kore_running = is_container_running(state.kore_container)
    baseline_running = is_container_running(state.baseline_container)
    kore_exists = is_container_exists(state.kore_container)
    baseline_exists = is_container_exists(state.baseline_container)
    
    # Sync state with actual Docker state
    if state.running:
        if not kore_exists and not baseline_exists:
            # Containers were removed externally
            state.running = False
            state.paused = False
        elif not kore_running and not baseline_running and kore_exists:
            # Containers exist but not running = paused
            state.paused = True
    
    return {
        "running": state.running,
        "paused": state.paused,
        "kore_running": kore_running,
        "baseline_running": baseline_running,
        "kore_container": state.kore_container,
        "baseline_container": state.baseline_container,
        "current_task": state.current_task,
        "task_history": state.task_history,
        "run_id": state.run_id
    }

@app.post("/api/start")
async def start_experiment(req: StartRequest):
    """Start new experiment with both agents"""
    if state.running and not state.paused:
        raise HTTPException(400, "Experiment already running")
    
    env = get_env()
    state.run_id = datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
    state.run_dir = BENCHMARKS_DIR / state.run_id
    state.run_dir.mkdir(parents=True, exist_ok=True)
    (state.run_dir / "kore" / "mnt").mkdir(parents=True, exist_ok=True)
    (state.run_dir / "baseline").mkdir(parents=True, exist_ok=True)
    
    # Make directories writable
    run_cmd(f"chmod -R 777 {state.run_dir}")
    
    state.kore_container = f"kore-dash-{state.run_id[:10]}"
    state.baseline_container = f"base-dash-{state.run_id[:10]}"
    
    model = req.model or env.get("KORE_MODEL", "google/gemini-2.5-flash-preview")
    
    await broadcast_log(f"Starting experiment {state.run_id}", "system")
    await broadcast_log(f"Model: {model}", "system")
    
    # Start Kore container
    kore_cmd = f"""docker run -d --name {state.kore_container} \
        -e "KORE_GOAL=Waiting for task" \
        -e "KORE_PERSISTENT=true" \
        -e "KORE_CAPS=all" \
        -e "KORE_MODEL={model}" \
        -e "OPENAI_API_KEY={env.get('OPENAI_API_KEY', '')}" \
        -e "OPENAI_BASE_URL={env.get('OPENAI_BASE_URL', '')}" \
        -v "{state.run_dir}/kore:/world" \
        -v "{state.run_dir}/kore/mnt:/mnt" \
        kore-world:latest kore-agent"""
    
    code, out = run_cmd(kore_cmd)
    if code != 0:
        await broadcast_log(f"Failed to start Kore: {out}", "error")
        raise HTTPException(500, f"Failed to start Kore: {out}")
    
    await broadcast_log("Kore agent started", "kore")
    
    # Start Baseline container
    baseline_cmd = f"""docker run -d --name {state.baseline_container} \
        -e "AGENT_GOAL=Waiting for task" \
        -e "AGENT_PERSISTENT=true" \
        -e "AGENT_MODEL={model}" \
        -e "OPENAI_API_KEY={env.get('OPENAI_API_KEY', '')}" \
        -e "OPENAI_BASE_URL={env.get('OPENAI_BASE_URL', '')}" \
        -v "{state.run_dir}/baseline:/workspace" \
        baseline-agent:latest"""
    
    code, out = run_cmd(baseline_cmd)
    if code != 0:
        await broadcast_log(f"Failed to start Baseline: {out}", "error")
        raise HTTPException(500, f"Failed to start Baseline: {out}")
    
    await broadcast_log("Baseline agent started", "baseline")
    
    # Verify containers are actually running
    await asyncio.sleep(1)  # Give Docker a moment
    kore_ok = is_container_running(state.kore_container)
    baseline_ok = is_container_running(state.baseline_container)
    
    if not kore_ok or not baseline_ok:
        # Get logs to see what went wrong
        kore_logs = run_cmd(f"docker logs {state.kore_container} 2>&1")[1] if not kore_ok else ""
        baseline_logs = run_cmd(f"docker logs {state.baseline_container} 2>&1")[1] if not baseline_ok else ""
        error_msg = f"Containers failed to start. Kore: {kore_ok}, Baseline: {baseline_ok}"
        if kore_logs:
            error_msg += f"\nKore logs: {kore_logs[-500:]}"
        if baseline_logs:
            error_msg += f"\nBaseline logs: {baseline_logs[-500:]}"
        await broadcast_log(error_msg, "error")
        raise HTTPException(500, error_msg)
    
    state.running = True
    state.paused = False
    state.task_history = []
    state.chat_history = {"kore": [], "baseline": []}  # Clear chat history on new experiment
    
    return {
        "status": "started", 
        "run_id": state.run_id,
        "kore_running": kore_ok,
        "baseline_running": baseline_ok
    }

@app.post("/api/pause")
async def pause_experiment():
    """Pause experiment (stop containers but keep state)"""
    if not state.running:
        raise HTTPException(400, "No experiment running")
    
    await broadcast_log("Pausing experiment...", "system")
    
    kore_stopped = False
    baseline_stopped = False
    
    if state.kore_container:
        code, out = run_cmd(f"docker stop {state.kore_container}")
        kore_stopped = code == 0
    if state.baseline_container:
        code, out = run_cmd(f"docker stop {state.baseline_container}")
        baseline_stopped = code == 0
    
    # Verify they actually stopped
    kore_running = is_container_running(state.kore_container)
    baseline_running = is_container_running(state.baseline_container)
    
    if kore_running or baseline_running:
        await broadcast_log(f"Warning: Some containers still running. Kore: {kore_running}, Baseline: {baseline_running}", "error")
    
    state.paused = True
    await broadcast_log("Experiment paused - containers stopped", "system")
    
    return {
        "status": "paused",
        "kore_stopped": not kore_running,
        "baseline_stopped": not baseline_running
    }

@app.post("/api/resume")
async def resume_experiment():
    """Resume paused experiment"""
    if not state.paused:
        raise HTTPException(400, "Experiment not paused")
    
    await broadcast_log("Resuming experiment...", "system")
    
    if state.kore_container:
        run_cmd(f"docker start {state.kore_container}")
    if state.baseline_container:
        run_cmd(f"docker start {state.baseline_container}")
    
    await asyncio.sleep(1)  # Give Docker a moment
    
    # Verify they actually started
    kore_running = is_container_running(state.kore_container)
    baseline_running = is_container_running(state.baseline_container)
    
    if not kore_running or not baseline_running:
        await broadcast_log(f"Warning: Some containers failed to start. Kore: {kore_running}, Baseline: {baseline_running}", "error")
    
    state.paused = False
    await broadcast_log("Experiment resumed", "system")
    
    return {
        "status": "resumed",
        "kore_running": kore_running,
        "baseline_running": baseline_running
    }

@app.post("/api/stop")
async def stop_experiment():
    """Stop and clean up experiment"""
    await broadcast_log("Stopping experiment...", "system")
    
    kore_removed = False
    baseline_removed = False
    
    if state.kore_container:
        code, out = run_cmd(f"docker rm -f {state.kore_container}")
        kore_removed = code == 0
        if kore_removed:
            state.kore_container = None
    if state.baseline_container:
        code, out = run_cmd(f"docker rm -f {state.baseline_container}")
        baseline_removed = code == 0
        if baseline_removed:
            state.baseline_container = None
    
    # Verify removal
    kore_exists = is_container_exists(state.kore_container) if state.kore_container else False
    baseline_exists = is_container_exists(state.baseline_container) if state.baseline_container else False
    
    if kore_exists or baseline_exists:
        await broadcast_log(f"Warning: Some containers still exist. Kore: {kore_exists}, Baseline: {baseline_exists}", "error")
    
    state.running = False
    state.paused = False
    state.current_task = None
    
    await broadcast_log("Experiment stopped", "system")
    
    return {
        "status": "stopped",
        "kore_removed": not kore_exists,
        "baseline_removed": not baseline_exists
    }

@app.post("/api/task")
async def send_task(req: TaskRequest):
    """Send task to both agents"""
    if not state.running or state.paused:
        raise HTTPException(400, "Experiment not running")
    
    task = req.task.strip()
    state.current_task = task
    
    await broadcast_log(f"Sending task: {task}", "system")
    
    # Clear done files first
    if state.run_dir:
        (state.run_dir / "kore" / "mnt" / "done.txt").unlink(missing_ok=True)
        (state.run_dir / "baseline" / "done.txt").unlink(missing_ok=True)
        
        # Write to inboxes
        (state.run_dir / "kore" / "mnt" / "inbox.txt").write_text(task)
        (state.run_dir / "baseline" / "inbox.txt").write_text(task)
    
    task_entry = {
        "task": task,
        "sent_at": datetime.now().isoformat(),
        "kore_done": False,
        "baseline_done": False
    }
    state.task_history.append(task_entry)
    
    await broadcast_log("Task sent to both agents", "system")
    
    return {"status": "sent", "task": task}

@app.get("/api/logs/{agent}")
async def get_logs(agent: str, lines: int = 50):
    """Get recent logs from an agent"""
    container = None
    if agent == "kore":
        container = state.kore_container
    elif agent == "baseline":
        container = state.baseline_container
    
    if not container:
        return {"logs": f"No {agent} container configured", "error": True}
    
    # Check if container exists
    if not is_container_exists(container):
        return {"logs": f"Container {container} does not exist", "error": True}
    
    code, out = run_cmd(f"docker logs --tail {lines} {container} 2>&1")
    if code != 0:
        return {"logs": f"Error getting logs: {out}", "error": True}
    
    return {"logs": out if out.strip() else "(no output yet)", "error": False, "container": container}

@app.get("/api/files/{agent}")
async def get_files(agent: str):
    """List files in agent workspace"""
    if not state.run_dir:
        return {"files": []}
    
    if agent == "kore":
        path = state.run_dir / "kore" / "mnt"
    else:
        path = state.run_dir / "baseline"
    
    if not path.exists():
        return {"files": []}
    
    files = []
    for f in path.iterdir():
        if f.is_file():
            files.append({
                "name": f.name,
                "size": f.stat().st_size,
                "modified": datetime.fromtimestamp(f.stat().st_mtime).isoformat()
            })
    
    return {"files": sorted(files, key=lambda x: x["name"])}

@app.get("/api/file/{agent}/{filename}")
async def get_file_content(agent: str, filename: str):
    """Get content of a specific file"""
    if not state.run_dir:
        raise HTTPException(404, "No experiment running")
    
    if agent == "kore":
        path = state.run_dir / "kore" / "mnt" / filename
    else:
        path = state.run_dir / "baseline" / filename
    
    if not path.exists():
        raise HTTPException(404, "File not found")
    
    try:
        content = path.read_text()
    except:
        content = "(binary file)"
    
    return {"content": content}

@app.get("/api/prompts")
async def get_prompts():
    """Get list of prompt files"""
    prompts = []
    for name in ["genesis-prompt.md", "Cargo.toml", "README.md"]:
        path = PROMPTS_DIR / name
        if path.exists():
            prompts.append({"name": name, "path": str(path)})
    
    # Also include baseline prompt
    baseline_prompt = KORE_ROOT / "benchmarks" / "baseline" / "baseline_agent.py"
    if baseline_prompt.exists():
        prompts.append({"name": "baseline_agent.py", "path": str(baseline_prompt)})
    
    return {"prompts": prompts}

@app.get("/api/prompt/{name}")
async def get_prompt(name: str):
    """Get content of a prompt file"""
    if name == "baseline_agent.py":
        path = KORE_ROOT / "benchmarks" / "baseline" / "baseline_agent.py"
    else:
        path = PROMPTS_DIR / name
    
    if not path.exists():
        raise HTTPException(404, "Prompt not found")
    
    return {"content": path.read_text()}

@app.get("/api/completion")
async def check_completion():
    """Check if agents have completed their tasks"""
    if not state.run_dir:
        return {"kore_done": False, "baseline_done": False}
    
    kore_done_file = state.run_dir / "kore" / "mnt" / "done.txt"
    baseline_done_file = state.run_dir / "baseline" / "done.txt"
    
    kore_done = kore_done_file.exists()
    baseline_done = baseline_done_file.exists()
    
    kore_result = kore_done_file.read_text() if kore_done else None
    baseline_result = baseline_done_file.read_text() if baseline_done else None
    
    # Update task history
    if state.task_history:
        state.task_history[-1]["kore_done"] = kore_done
        state.task_history[-1]["baseline_done"] = baseline_done
        if kore_result:
            state.task_history[-1]["kore_result"] = kore_result
        if baseline_result:
            state.task_history[-1]["baseline_result"] = baseline_result
    
    return {
        "kore_done": kore_done,
        "baseline_done": baseline_done,
        "kore_result": kore_result,
        "baseline_result": baseline_result
    }

@app.post("/api/chat")
async def send_chat(req: ChatRequest):
    """Send a chat message to a specific agent"""
    if not state.running or state.paused:
        raise HTTPException(400, "Experiment not running")
    
    agent = req.agent.lower()
    if agent not in ["kore", "baseline"]:
        raise HTTPException(400, "Invalid agent")
    
    message = req.message.strip()
    
    # Add user message to history
    state.chat_history[agent].append({
        "role": "user",
        "content": message,
        "timestamp": datetime.now().isoformat()
    })
    
    await broadcast_log(f"[Chat→{agent}] {message[:50]}...", "system")
    
    # Clear done file and write to inbox
    if state.run_dir:
        if agent == "kore":
            done_file = state.run_dir / "kore" / "mnt" / "done.txt"
            inbox_file = state.run_dir / "kore" / "mnt" / "inbox.txt"
        else:
            done_file = state.run_dir / "baseline" / "done.txt"
            inbox_file = state.run_dir / "baseline" / "inbox.txt"
        
        done_file.unlink(missing_ok=True)
        inbox_file.write_text(message)
    
    return {"status": "sent", "agent": agent}

@app.get("/api/chat/{agent}")
async def get_chat_history(agent: str):
    """Get chat history for an agent"""
    if agent not in ["kore", "baseline"]:
        raise HTTPException(400, "Invalid agent")
    
    return {"history": state.chat_history.get(agent, [])}

@app.get("/api/chat/{agent}/response")
async def check_chat_response(agent: str):
    """Check if agent has responded (done.txt exists) and get logs"""
    if not state.run_dir:
        return {"done": False, "response": None}
    
    if agent == "kore":
        done_file = state.run_dir / "kore" / "mnt" / "done.txt"
        container = state.kore_container
    else:
        done_file = state.run_dir / "baseline" / "done.txt"
        container = state.baseline_container
    
    done = done_file.exists()
    response = done_file.read_text() if done else None
    
    # Get recent logs as context
    logs = ""
    if container:
        code, logs = run_cmd(f"docker logs --tail 30 {container} 2>&1")
    
    # If done, add assistant response to history
    if done and response:
        # Check if we already added this response
        history = state.chat_history.get(agent, [])
        if not history or history[-1].get("role") != "assistant":
            state.chat_history[agent].append({
                "role": "assistant",
                "content": response,
                "timestamp": datetime.now().isoformat(),
                "logs": logs[-2000:]  # Last 2000 chars of logs
            })
    
    return {"done": done, "response": response, "logs": logs[-2000:]}

@app.delete("/api/chat/{agent}")
async def clear_chat_history(agent: str):
    """Clear chat history for an agent"""
    if agent not in ["kore", "baseline"]:
        raise HTTPException(400, "Invalid agent")
    
    state.chat_history[agent] = []
    return {"status": "cleared"}

@app.websocket("/ws/logs")
async def websocket_logs(websocket: WebSocket):
    """WebSocket for live log streaming"""
    await websocket.accept()
    log_connections.append(websocket)
    
    try:
        while True:
            # Keep connection alive and stream logs periodically
            await asyncio.sleep(2)
            
            # Stream latest logs from containers
            if state.kore_container and state.running and not state.paused:
                code, out = run_cmd(f"docker logs --tail 5 {state.kore_container} 2>&1")
                if out.strip():
                    await websocket.send_text(json.dumps({
                        "timestamp": datetime.now().isoformat(),
                        "source": "kore",
                        "message": out.strip()[-500:]  # Last 500 chars
                    }))
            
            if state.baseline_container and state.running and not state.paused:
                code, out = run_cmd(f"docker logs --tail 5 {state.baseline_container} 2>&1")
                if out.strip():
                    await websocket.send_text(json.dumps({
                        "timestamp": datetime.now().isoformat(),
                        "source": "baseline", 
                        "message": out.strip()[-500:]
                    }))
    except WebSocketDisconnect:
        log_connections.remove(websocket)

# Mount static files
static_dir = Path(__file__).parent / "static"
static_dir.mkdir(exist_ok=True)
app.mount("/static", StaticFiles(directory=str(static_dir)), name="static")

if __name__ == "__main__":
    print(f"🚀 Kore Dashboard starting at http://localhost:8080")
    print(f"📁 Kore root: {KORE_ROOT}")
    uvicorn.run(app, host="0.0.0.0", port=8080)
