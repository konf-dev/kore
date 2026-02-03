#!/usr/bin/env python3
"""
Kore Benchmark Dashboard v2
Robust experiment monitoring with persistent state and auto-recovery
"""

import os
import json
import subprocess
import asyncio
from datetime import datetime
from pathlib import Path
from typing import Optional
from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket, WebSocketDisconnect, HTTPException
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
from pydantic import BaseModel
import uvicorn

# =============================================================================
# CONFIGURATION
# =============================================================================

KORE_ROOT = Path(__file__).parent.parent
BENCHMARKS_DIR = KORE_ROOT / "benchmarks" / "dashboard"
STATE_FILE = BENCHMARKS_DIR / ".dashboard_state.json"

# =============================================================================
# STATE MANAGEMENT - Persistent & Recoverable
# =============================================================================

class ExperimentState:
    """Persistent experiment state with automatic disk sync"""
    
    def __init__(self):
        self.running = False
        self.paused = False
        self.kore_only = False
        self.kore_container: Optional[str] = None
        self.baseline_container: Optional[str] = None
        self.current_task: Optional[str] = None
        self.task_history: list = []
        self.run_id: Optional[str] = None
        self.model: Optional[str] = None
        self.chat_history: dict = {"kore": [], "baseline": []}
    
    @property
    def run_dir(self) -> Optional[Path]:
        """Compute run_dir from run_id"""
        if self.run_id:
            return BENCHMARKS_DIR / self.run_id
        return None
    
    def to_dict(self) -> dict:
        return {
            "running": self.running,
            "paused": self.paused,
            "kore_only": self.kore_only,
            "kore_container": self.kore_container,
            "baseline_container": self.baseline_container,
            "current_task": self.current_task,
            "task_history": self.task_history,
            "run_id": self.run_id,
            "model": self.model,
            "chat_history": self.chat_history,
        }
    
    def from_dict(self, data: dict):
        self.running = data.get("running", False)
        self.paused = data.get("paused", False)
        self.kore_only = data.get("kore_only", False)
        self.kore_container = data.get("kore_container")
        self.baseline_container = data.get("baseline_container")
        self.current_task = data.get("current_task")
        self.task_history = data.get("task_history", [])
        self.run_id = data.get("run_id")
        self.model = data.get("model")
        self.chat_history = data.get("chat_history", {"kore": [], "baseline": []})
    
    def save(self):
        """Persist state to disk"""
        STATE_FILE.parent.mkdir(parents=True, exist_ok=True)
        STATE_FILE.write_text(json.dumps(self.to_dict(), indent=2))
    
    def load(self):
        """Load state from disk"""
        if STATE_FILE.exists():
            try:
                self.from_dict(json.loads(STATE_FILE.read_text()))
            except Exception as e:
                print(f"Warning: Could not load state: {e}")
    
    def reset(self):
        """Reset to clean state"""
        self.__init__()
        self.save()

state = ExperimentState()

# =============================================================================
# DOCKER HELPERS
# =============================================================================

def run_cmd(cmd: str, timeout: int = 30) -> tuple[int, str]:
    """Run shell command with timeout"""
    try:
        result = subprocess.run(
            cmd, shell=True, capture_output=True, text=True, timeout=timeout
        )
        return result.returncode, result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return -1, "Command timed out"
    except Exception as e:
        return -1, str(e)

def docker_container_status(name: str) -> dict:
    """Get detailed container status"""
    if not name:
        return {"exists": False, "running": False, "status": "not configured"}
    
    code, out = run_cmd(f'docker inspect {name} --format "{{{{.State.Status}}}}" 2>/dev/null')
    if code != 0:
        return {"exists": False, "running": False, "status": "not found"}
    
    status = out.strip()
    return {
        "exists": True,
        "running": status == "running",
        "status": status
    }

def docker_kill_all_experiments():
    """Kill all experiment containers"""
    run_cmd('docker rm -f $(docker ps -aq --filter "name=kore-dash") 2>/dev/null')
    run_cmd('docker rm -f $(docker ps -aq --filter "name=base-dash") 2>/dev/null')

def get_env() -> dict:
    """Load environment variables from .env file"""
    env_file = KORE_ROOT / ".env"
    env = os.environ.copy()
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            line = line.strip()
            if "=" in line and not line.startswith("#"):
                key, val = line.split("=", 1)
                env[key.strip()] = val.strip().strip('"').strip("'")
    return env

# =============================================================================
# STATE RECOVERY
# =============================================================================

def recover_state():
    """Recover state from running containers or disk"""
    state.load()
    
    # Verify containers match saved state
    if state.kore_container:
        kore_status = docker_container_status(state.kore_container)
        if not kore_status["exists"]:
            # Container gone, check if we can find one
            code, out = run_cmd('docker ps -a --filter "name=kore-dash" --format "{{.Names}}" | head -1')
            if out.strip():
                state.kore_container = out.strip()
                # Extract run_id from container name
                if state.kore_container.startswith("kore-dash-"):
                    date_part = state.kore_container.replace("kore-dash-", "")
                    # Find matching run directory
                    for d in BENCHMARKS_DIR.iterdir():
                        if d.is_dir() and d.name.startswith(date_part):
                            state.run_id = d.name
                            break
        
        kore_status = docker_container_status(state.kore_container)
        state.running = kore_status["running"]
        state.paused = kore_status["exists"] and not kore_status["running"]
    
    # Also check baseline
    if state.baseline_container and not state.kore_only:
        baseline_status = docker_container_status(state.baseline_container)
        if not baseline_status["exists"]:
            state.baseline_container = None
    
    state.save()
    print(f"State recovered: running={state.running}, run_id={state.run_id}")

# =============================================================================
# LIFESPAN - Auto-recovery on startup
# =============================================================================

@asynccontextmanager
async def lifespan(app):
    """Startup and shutdown events"""
    print("🔄 Recovering state...")
    recover_state()
    yield
    print("💾 Saving state...")
    state.save()

# =============================================================================
# APP SETUP
# =============================================================================

app = FastAPI(title="Kore Dashboard", lifespan=lifespan)

# WebSocket connections
log_connections: list[WebSocket] = []

async def broadcast(msg: str, source: str = "system"):
    """Broadcast to all WebSocket clients"""
    data = json.dumps({"timestamp": datetime.now().isoformat(), "source": source, "message": msg})
    for ws in log_connections[:]:
        try:
            await ws.send_text(data)
        except:
            log_connections.remove(ws)

# =============================================================================
# MODELS
# =============================================================================

class StartRequest(BaseModel):
    model: str = "anthropic/claude-sonnet-4"
    kore_only: bool = True

class TaskRequest(BaseModel):
    task: str

class ChatRequest(BaseModel):
    message: str
    agent: str

# =============================================================================
# API ROUTES
# =============================================================================

@app.get("/")
async def index():
    return FileResponse(Path(__file__).parent / "static" / "index.html")

@app.get("/api/status")
async def get_status():
    """Get comprehensive experiment status"""
    kore_status = docker_container_status(state.kore_container)
    baseline_status = docker_container_status(state.baseline_container) if not state.kore_only else {"exists": False, "running": False, "status": "disabled"}
    
    # Sync state with reality
    if state.running and not kore_status["running"]:
        if kore_status["exists"]:
            state.paused = True
        else:
            state.running = False
            state.paused = False
    
    return {
        "running": state.running,
        "paused": state.paused,
        "kore_only": state.kore_only,
        "run_id": state.run_id,
        "model": state.model,
        "kore": {
            "container": state.kore_container,
            **kore_status
        },
        "baseline": {
            "container": state.baseline_container,
            **baseline_status
        },
        "current_task": state.current_task,
        "task_history": state.task_history[-10:],  # Last 10 tasks
    }

@app.get("/api/experiments")
async def list_experiments():
    """List all experiment runs"""
    experiments = []
    if BENCHMARKS_DIR.exists():
        for d in sorted(BENCHMARKS_DIR.iterdir(), reverse=True):
            if d.is_dir() and not d.name.startswith("."):
                kore_storage = d / "kore" / ".kore_storage"
                rom_count = len(list(kore_storage.glob("*.json"))) if kore_storage.exists() else 0
                experiments.append({
                    "run_id": d.name,
                    "active": d.name == state.run_id,
                    "rom_keys": rom_count,
                    "has_kore": (d / "kore").exists(),
                    "has_baseline": (d / "baseline").exists(),
                })
    return {"experiments": experiments[:20]}  # Last 20

@app.post("/api/start")
async def start_experiment(req: StartRequest):
    """Start new experiment"""
    if state.running and not state.paused:
        raise HTTPException(400, "Experiment already running. Stop it first.")
    
    env = get_env()
    
    # Create new run
    state.run_id = datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
    state.model = req.model
    state.kore_only = req.kore_only
    state.kore_container = f"kore-dash-{state.run_id[:10]}"
    state.baseline_container = f"base-dash-{state.run_id[:10]}" if not req.kore_only else None
    state.task_history = []
    state.chat_history = {"kore": [], "baseline": []}
    
    # Create directories
    run_dir = BENCHMARKS_DIR / state.run_id
    (run_dir / "kore" / "mnt").mkdir(parents=True, exist_ok=True)
    if not req.kore_only:
        (run_dir / "baseline").mkdir(parents=True, exist_ok=True)
    run_cmd(f"chmod -R 777 {run_dir}")
    
    # Start Kore container
    kore_cmd = f"""docker run -d --name {state.kore_container} \
        -e "KORE_GOAL=Autonomous agent ready" \
        -e "KORE_PERSISTENT=true" \
        -e "KORE_CAPS=all" \
        -e "KORE_MODEL={req.model}" \
        -e "KORE_MAX_ITERATIONS=500" \
        -e "OPENAI_API_KEY={env.get('OPENAI_API_KEY', '')}" \
        -e "OPENAI_BASE_URL={env.get('OPENAI_BASE_URL', '')}" \
        -v "{run_dir}/kore:/world" \
        -v "{run_dir}/kore/mnt:/mnt" \
        kore-world:latest kore-agent"""
    
    code, out = run_cmd(kore_cmd)
    if code != 0:
        raise HTTPException(500, f"Failed to start Kore: {out}")
    
    # Start baseline if needed
    if not req.kore_only:
        baseline_cmd = f"""docker run -d --name {state.baseline_container} \
            -e "AGENT_GOAL=Waiting for task" \
            -e "AGENT_PERSISTENT=true" \
            -e "AGENT_MODEL={req.model}" \
            -e "AGENT_MAX_ITERATIONS=100" \
            -e "OPENAI_API_KEY={env.get('OPENAI_API_KEY', '')}" \
            -e "OPENAI_BASE_URL={env.get('OPENAI_BASE_URL', '')}" \
            -v "{run_dir}/baseline:/workspace" \
            baseline-agent:latest"""
        code, out = run_cmd(baseline_cmd)
        if code != 0:
            run_cmd(f"docker rm -f {state.kore_container}")
            raise HTTPException(500, f"Failed to start Baseline: {out}")
    
    state.running = True
    state.paused = False
    state.save()
    
    await broadcast(f"Started experiment {state.run_id}", "system")
    return {"status": "started", "run_id": state.run_id}

@app.post("/api/stop")
async def stop_experiment():
    """Stop and cleanup experiment"""
    if state.kore_container:
        run_cmd(f"docker rm -f {state.kore_container}")
    if state.baseline_container:
        run_cmd(f"docker rm -f {state.baseline_container}")
    
    old_run = state.run_id
    state.running = False
    state.paused = False
    state.kore_container = None
    state.baseline_container = None
    state.save()
    
    await broadcast(f"Stopped experiment {old_run}", "system")
    return {"status": "stopped"}

@app.post("/api/pause")
async def pause_experiment():
    """Pause experiment"""
    if not state.running:
        raise HTTPException(400, "No experiment running")
    
    if state.kore_container:
        run_cmd(f"docker stop {state.kore_container}")
    if state.baseline_container:
        run_cmd(f"docker stop {state.baseline_container}")
    
    state.paused = True
    state.save()
    return {"status": "paused"}

@app.post("/api/resume")
async def resume_experiment():
    """Resume paused experiment"""
    if not state.paused:
        raise HTTPException(400, "Experiment not paused")
    
    if state.kore_container:
        run_cmd(f"docker start {state.kore_container}")
    if state.baseline_container:
        run_cmd(f"docker start {state.baseline_container}")
    
    state.paused = False
    state.running = True
    state.save()
    return {"status": "resumed"}

@app.post("/api/attach/{run_id}")
async def attach_experiment(run_id: str):
    """Attach to an existing experiment directory"""
    run_dir = BENCHMARKS_DIR / run_id
    if not run_dir.exists():
        raise HTTPException(404, f"Experiment {run_id} not found")
    
    # Find any running containers for this run
    code, out = run_cmd(f'docker ps --filter "name=kore-dash-{run_id[:10]}" --format "{{{{.Names}}}}"')
    kore_container = out.strip() if out.strip() else None
    
    code, out = run_cmd(f'docker ps --filter "name=base-dash-{run_id[:10]}" --format "{{{{.Names}}}}"')
    baseline_container = out.strip() if out.strip() else None
    
    state.run_id = run_id
    state.kore_container = kore_container
    state.baseline_container = baseline_container
    state.kore_only = baseline_container is None
    state.running = kore_container is not None
    state.paused = False
    state.save()
    
    return {"status": "attached", "run_id": run_id, "kore_running": kore_container is not None}

# =============================================================================
# STORAGE API - ROM/RAM monitoring
# =============================================================================

@app.get("/api/storage")
async def get_storage():
    """Get Kore storage status (ROM)"""
    if not state.run_dir:
        return {"error": "No experiment", "rom": {"keys": {}, "count": 0, "total_size": 0}}
    
    rom_dir = state.run_dir / "kore" / ".kore_storage"
    rom_keys = {}
    
    if rom_dir.exists():
        for f in rom_dir.iterdir():
            if f.is_file() and f.suffix == ".json":
                key = f.stem
                size = f.stat().st_size
                try:
                    content = json.loads(f.read_text())
                    if isinstance(content, dict) and "body" in content:
                        # It's a tool definition - extract full stats
                        meta = content.get("meta", {})
                        stats = meta.get("stats", {})
                        rom_keys[key] = {
                            "type": "tool",
                            "size": size,
                            "name": content.get("name", key),
                            "calls": stats.get("calls", 0),
                            "failures": stats.get("failures", 0),
                            "time_ms": stats.get("time_ms", 0),
                            "status": meta.get("status", "ok"),
                            "doc": meta.get("doc", ""),
                            "sig": meta.get("sig", ""),
                        }
                    else:
                        preview = str(content)[:80]
                        rom_keys[key] = {"type": "value", "size": size, "preview": preview}
                except:
                    rom_keys[key] = {"type": "raw", "size": size}
    
    return {
        "run_id": state.run_id,
        "rom": {
            "path": str(rom_dir),
            "keys": rom_keys,
            "count": len(rom_keys),
            "total_size": sum(v["size"] for v in rom_keys.values())
        }
    }

@app.get("/api/storage/rom/{key}")
async def get_rom_key(key: str):
    """Get content of a ROM key"""
    if not state.run_dir:
        raise HTTPException(404, "No experiment")
    
    rom_file = state.run_dir / "kore" / ".kore_storage" / f"{key}.json"
    if not rom_file.exists():
        raise HTTPException(404, f"Key '{key}' not found")
    
    return {"key": key, "content": json.loads(rom_file.read_text())}

@app.get("/api/agent-status")
async def get_agent_status():
    """Get agent's self-reported status from /world/status.json"""
    if not state.run_dir:
        return {"error": "No experiment", "status": None}
    
    status_file = state.run_dir / "kore" / "status.json"
    if not status_file.exists():
        return {"error": None, "status": None, "message": "Agent hasn't written status yet"}
    
    try:
        content = json.loads(status_file.read_text())
        return {"error": None, "status": content}
    except Exception as e:
        return {"error": str(e), "status": None}

@app.get("/api/agent-learnings")
async def get_agent_learnings():
    """Get agent's learnings from /world/learnings/"""
    if not state.run_dir:
        return {"error": "No experiment", "learnings": {}}
    
    learnings_dir = state.run_dir / "kore" / "learnings"
    learnings = {}
    
    if learnings_dir.exists():
        for f in learnings_dir.iterdir():
            if f.is_file() and f.suffix == ".md":
                try:
                    learnings[f.stem] = f.read_text()[:2000]  # First 2000 chars
                except:
                    learnings[f.stem] = "(unreadable)"
    
    return {"error": None, "learnings": learnings}

# =============================================================================
# LOGS API
# =============================================================================

@app.get("/api/logs/{agent}")
async def get_logs(agent: str, lines: int = 100):
    """Get container logs"""
    container = state.kore_container if agent == "kore" else state.baseline_container
    if not container:
        return {"logs": f"No {agent} container", "error": True}
    
    status = docker_container_status(container)
    if not status["exists"]:
        return {"logs": f"Container {container} not found", "error": True}
    
    code, out = run_cmd(f"docker logs --tail {lines} {container} 2>&1")
    return {"logs": out or "(empty)", "error": code != 0, "container": container, "status": status["status"]}

# =============================================================================
# FILES API
# =============================================================================

@app.get("/api/files/{agent}")
async def get_files(agent: str):
    """List files in agent workspace"""
    if not state.run_dir:
        return {"files": [], "error": "No experiment"}
    
    if agent == "kore":
        paths = [state.run_dir / "kore" / "mnt", state.run_dir / "kore"]
    else:
        paths = [state.run_dir / "baseline"]
    
    files = []
    for base in paths:
        if base.exists():
            for f in base.rglob("*"):
                if f.is_file() and ".kore_storage" not in str(f):
                    rel = f.relative_to(state.run_dir / ("kore" if agent == "kore" else "baseline"))
                    files.append({
                        "name": str(rel),
                        "size": f.stat().st_size,
                        "modified": datetime.fromtimestamp(f.stat().st_mtime).isoformat()
                    })
    
    return {"files": sorted(files, key=lambda x: x["name"])}

@app.get("/api/file/{agent}/{filename:path}")
async def get_file(agent: str, filename: str):
    """Get file content"""
    if not state.run_dir:
        raise HTTPException(404, "No experiment")
    
    base = state.run_dir / ("kore" if agent == "kore" else "baseline")
    filepath = base / filename
    
    if not filepath.exists():
        raise HTTPException(404, "File not found")
    
    try:
        return {"content": filepath.read_text()}
    except:
        return {"content": "(binary)", "binary": True}

# =============================================================================
# TASK & CHAT API
# =============================================================================

@app.post("/api/task")
async def send_task(req: TaskRequest):
    """Send task to agents via inbox"""
    if not state.running or state.paused:
        raise HTTPException(400, "Experiment not running")
    
    task = req.task.strip()
    state.current_task = task
    
    if state.run_dir:
        # Clear done, write inbox
        (state.run_dir / "kore" / "mnt" / "done.txt").unlink(missing_ok=True)
        (state.run_dir / "kore" / "mnt" / "inbox.txt").write_text(task)
        
        if not state.kore_only:
            (state.run_dir / "baseline" / "done.txt").unlink(missing_ok=True)
            (state.run_dir / "baseline" / "inbox.txt").write_text(task)
    
    state.task_history.append({
        "task": task,
        "sent_at": datetime.now().isoformat(),
        "kore_done": False,
        "baseline_done": state.kore_only
    })
    state.save()
    
    await broadcast(f"Task: {task[:50]}...", "system")
    return {"status": "sent", "task": task}

@app.post("/api/chat")
async def send_chat(req: ChatRequest):
    """Send chat message to specific agent"""
    if not state.running or state.paused:
        raise HTTPException(400, "Experiment not running")
    
    agent = req.agent.lower()
    message = req.message.strip()
    
    state.chat_history[agent].append({
        "role": "user",
        "content": message,
        "timestamp": datetime.now().isoformat()
    })
    
    if state.run_dir:
        if agent == "kore":
            (state.run_dir / "kore" / "mnt" / "done.txt").unlink(missing_ok=True)
            (state.run_dir / "kore" / "mnt" / "inbox.txt").write_text(message)
        else:
            (state.run_dir / "baseline" / "done.txt").unlink(missing_ok=True)
            (state.run_dir / "baseline" / "inbox.txt").write_text(message)
    
    state.save()
    return {"status": "sent", "agent": agent}

@app.get("/api/chat/{agent}")
async def get_chat(agent: str):
    """Get chat history"""
    return {"history": state.chat_history.get(agent, [])}

@app.get("/api/chat/{agent}/response")
async def check_response(agent: str):
    """Check for agent response"""
    if not state.run_dir:
        return {"done": False, "response": None}
    
    done_file = state.run_dir / ("kore/mnt" if agent == "kore" else "baseline") / "done.txt"
    container = state.kore_container if agent == "kore" else state.baseline_container
    
    done = done_file.exists()
    response = done_file.read_text() if done else None
    
    logs = ""
    if container:
        _, logs = run_cmd(f"docker logs --tail 30 {container} 2>&1")
    
    if done and response:
        history = state.chat_history.get(agent, [])
        if not history or history[-1].get("role") != "assistant":
            state.chat_history[agent].append({
                "role": "assistant",
                "content": response,
                "timestamp": datetime.now().isoformat()
            })
            state.save()
    
    return {"done": done, "response": response, "logs": logs[-2000:]}

@app.delete("/api/chat/{agent}")
async def clear_chat(agent: str):
    """Clear chat history"""
    state.chat_history[agent] = []
    state.save()
    return {"status": "cleared"}

@app.get("/api/completion")
async def check_completion():
    """Check task completion"""
    if not state.run_dir:
        return {"kore_done": False, "baseline_done": False}
    
    kore_done = (state.run_dir / "kore" / "mnt" / "done.txt").exists()
    baseline_done = state.kore_only or (state.run_dir / "baseline" / "done.txt").exists()
    
    if state.task_history:
        state.task_history[-1]["kore_done"] = kore_done
        state.task_history[-1]["baseline_done"] = baseline_done
    
    return {"kore_done": kore_done, "baseline_done": baseline_done}

# =============================================================================
# PROMPTS API
# =============================================================================

@app.get("/api/prompts")
async def get_prompts():
    """List prompt files"""
    prompts = []
    for name in ["genesis-prompt.md", "README.md"]:
        if (KORE_ROOT / name).exists():
            prompts.append({"name": name})
    return {"prompts": prompts}

@app.get("/api/prompt/{name}")
async def get_prompt(name: str):
    """Get prompt content"""
    path = KORE_ROOT / name
    if not path.exists():
        raise HTTPException(404, "Not found")
    return {"content": path.read_text()}

# =============================================================================
# WEBSOCKET
# =============================================================================

@app.websocket("/ws/logs")
async def ws_logs(websocket: WebSocket):
    """Live log streaming"""
    await websocket.accept()
    log_connections.append(websocket)
    
    try:
        while True:
            await asyncio.sleep(2)
            if state.kore_container and state.running and not state.paused:
                _, out = run_cmd(f"docker logs --tail 5 {state.kore_container} 2>&1")
                if out.strip():
                    await websocket.send_text(json.dumps({
                        "timestamp": datetime.now().isoformat(),
                        "source": "kore",
                        "message": out.strip()[-500:]
                    }))
    except WebSocketDisconnect:
        log_connections.remove(websocket)

# =============================================================================
# STATIC FILES
# =============================================================================

static_dir = Path(__file__).parent / "static"
static_dir.mkdir(exist_ok=True)
app.mount("/static", StaticFiles(directory=str(static_dir)), name="static")

# =============================================================================
# MAIN
# =============================================================================

if __name__ == "__main__":
    print("=" * 60)
    print("🚀 KORE DASHBOARD v2")
    print(f"📁 Root: {KORE_ROOT}")
    print(f"📊 Benchmarks: {BENCHMARKS_DIR}")
    print("=" * 60)
    uvicorn.run(app, host="0.0.0.0", port=8080)
