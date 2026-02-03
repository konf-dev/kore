"""
Kore Agent Runtime Server - E001 Experiment

A safe execution environment for LLM agents with:
- Pre-flight static analysis
- Capability-based access control
- Session management

Usage:
    cd experiments/E001-agent-runtime
    pip install -r requirements.txt
    python server.py
"""

import subprocess
import json
import time
import uuid
from pathlib import Path
from typing import Optional
from dataclasses import dataclass, field
from datetime import datetime

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn

# ============================================================================
# Configuration
# ============================================================================

KORE_ROOT = Path(__file__).parent.parent.parent
KORE_BIN = KORE_ROOT / "target" / "release" / "kore"

# ============================================================================
# Capability Profiles
# ============================================================================

# Effects are categories: fs, io, net, env, time, spawn, exec, mem
PROFILES = {
    "pure": {
        "name": "pure",
        "description": "Pure computation only. No IO effects allowed.",
        "allowed_effects": set(),
    },
    "read-only": {
        "name": "read-only",
        "description": "Read-only filesystem and network. Console IO allowed.",
        "allowed_effects": {"fs", "io"},  # fs includes read, io for print
    },
    "local-io": {
        "name": "local-io",
        "description": "Local filesystem, console, time, and env. No network.",
        "allowed_effects": {"fs", "io", "time", "env"},
    },
    "full": {
        "name": "full",
        "description": "Full access. Use with caution!",
        "allowed_effects": {"*"},
    },
}

# ============================================================================
# Session Management
# ============================================================================

@dataclass
class ExecutionRecord:
    timestamp: str
    code: str
    success: bool
    duration_us: int
    effects_used: list[str]

@dataclass 
class Session:
    id: str
    created_at: str
    profile: dict
    definitions: dict = field(default_factory=dict)
    audit_log: list = field(default_factory=list)

sessions: dict[str, Session] = {}

# ============================================================================
# Kore Interface
# ============================================================================

def ensure_kore_built():
    """Ensure kore is built in release mode."""
    if not KORE_BIN.exists():
        print("Building kore in release mode...")
        result = subprocess.run(
            ["cargo", "build", "--release", "--bin", "kore"],
            cwd=KORE_ROOT,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise RuntimeError(f"Failed to build kore: {result.stderr}")
    return KORE_BIN

def run_kore(code: str, mode: str = "run") -> tuple[dict, int]:
    """
    Run kore and return (result, duration_us).
    
    Modes:
    - "run": Execute code, return stack
    - "analyze": Static analysis only
    - "effects": Get IO effects
    """
    kore_bin = ensure_kore_built()
    
    start = time.perf_counter_ns()
    
    if mode == "analyze":
        # Use effect-infer tool
        wrapped = f'[ {code} ] effect-infer'
    elif mode == "effects":
        # Use io-effects tool
        wrapped = f'[ {code} ] io-effects'
    elif mode == "pure":
        # Check if pure
        wrapped = f'[ {code} ] pure?'
    else:
        wrapped = code
    
    result = subprocess.run(
        [str(kore_bin), "-e", wrapped],
        capture_output=True,
        text=True,
        timeout=10,
    )
    
    duration_us = (time.perf_counter_ns() - start) // 1000
    
    if result.returncode != 0:
        return {"error": result.stderr.strip()}, duration_us
    
    # Parse output (stack values, one per line)
    output = result.stdout.strip()
    
    return {"output": output, "raw": result.stdout}, duration_us

def analyze_code(code: str) -> dict:
    """Analyze code and return effect information."""
    # Get effects
    effects_result, _ = run_kore(code, mode="effects")
    pure_result, _ = run_kore(code, mode="pure")
    
    # Parse effects list from output
    effects_str = effects_result.get("output", "[]")
    try:
        # Kore outputs lists like: [ "fs" "io" ]
        # Convert to Python list
        effects = []
        if effects_str and effects_str != "[]":
            # Simple parsing for ["fs", "io"] style output
            import re
            effects = re.findall(r'"([^"]*)"', effects_str)
    except:
        effects = []
    
    is_pure = pure_result.get("output", "").strip().lower() == "true"
    
    return {
        "io_effects": effects,
        "pure": is_pure,
        "required_caps": effects,
    }

def check_capabilities(required: list[str], profile: dict) -> list[str]:
    """Check if profile allows required capabilities. Return missing ones."""
    allowed = profile["allowed_effects"]
    if "*" in allowed:
        return []
    return [cap for cap in required if cap not in allowed]

# ============================================================================
# API Models
# ============================================================================

class AnalyzeRequest(BaseModel):
    code: str

class ExecuteRequest(BaseModel):
    code: str
    profile: str = "pure"
    session_id: Optional[str] = None

class DefineRequest(BaseModel):
    name: str
    code: str
    session_id: str

class CreateSessionRequest(BaseModel):
    profile: str = "pure"

# ============================================================================
# FastAPI App
# ============================================================================

app = FastAPI(
    title="Kore Agent Runtime",
    description="Safe execution environment for LLM agents",
    version="0.1.0",
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.get("/health")
async def health():
    """Health check."""
    return {"status": "healthy", "kore_bin": str(KORE_BIN)}

@app.get("/profiles")
async def list_profiles():
    """List available capability profiles."""
    return list(PROFILES.values())

@app.post("/analyze")
async def analyze(req: AnalyzeRequest):
    """
    Static analysis of Kore code.
    Returns IO effects and whether code is pure.
    """
    start = time.perf_counter_ns()
    
    analysis = analyze_code(req.code)
    
    analysis_time_us = (time.perf_counter_ns() - start) // 1000
    analysis["analysis_time_us"] = analysis_time_us
    
    return analysis

@app.post("/execute")
async def execute(req: ExecuteRequest):
    """
    Execute Kore code with capability checking.
    """
    # Get profile
    profile = PROFILES.get(req.profile)
    if not profile:
        raise HTTPException(400, f"Unknown profile: {req.profile}")
    
    # Analyze to get required capabilities
    analysis = analyze_code(req.code)
    required = analysis["required_caps"]
    
    # Check capabilities
    missing = check_capabilities(required, profile)
    if missing:
        raise HTTPException(
            403,
            f"Capability denied. Missing: {missing}. "
            f"Profile '{req.profile}' allows: {list(profile['allowed_effects'])}"
        )
    
    # Execute
    result, duration_us = run_kore(req.code, mode="run")
    
    # Log to session if provided
    if req.session_id and req.session_id in sessions:
        session = sessions[req.session_id]
        session.audit_log.append(ExecutionRecord(
            timestamp=datetime.utcnow().isoformat(),
            code=req.code,
            success="error" not in result,
            duration_us=duration_us,
            effects_used=required,
        ))
    
    return {
        "result": result.get("output"),
        "execution_time_us": duration_us,
        "effects_used": required,
    }

@app.post("/sessions")
async def create_session(req: CreateSessionRequest):
    """Create a new session with given profile."""
    profile = PROFILES.get(req.profile)
    if not profile:
        raise HTTPException(400, f"Unknown profile: {req.profile}")
    
    session = Session(
        id=str(uuid.uuid4()),
        created_at=datetime.utcnow().isoformat(),
        profile=profile,
    )
    sessions[session.id] = session
    
    return {
        "id": session.id,
        "profile": session.profile,
        "created_at": session.created_at,
    }

@app.get("/sessions/{session_id}")
async def get_session(session_id: str):
    """Get session info."""
    if session_id not in sessions:
        raise HTTPException(404, f"Session not found: {session_id}")
    
    session = sessions[session_id]
    return {
        "id": session.id,
        "profile": session.profile,
        "definitions": list(session.definitions.keys()),
        "execution_count": len(session.audit_log),
        "created_at": session.created_at,
    }

@app.delete("/sessions/{session_id}")
async def delete_session(session_id: str):
    """Delete a session."""
    if session_id in sessions:
        del sessions[session_id]
        return {"deleted": True}
    return {"deleted": False}

# ============================================================================
# Main
# ============================================================================

def print_banner():
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║                    KORE AGENT RUNTIME                     ║")
    print("║              E001 Experiment - Safe Execution             ║")
    print("╠═══════════════════════════════════════════════════════════╣")
    print("║  Endpoints:                                               ║")
    print("║    POST /analyze   - Static effect analysis               ║")
    print("║    POST /execute   - Execute with capability check        ║")
    print("║    POST /sessions  - Create session                       ║")
    print("║    GET  /profiles  - List capability profiles             ║")
    print("║    GET  /health    - Health check                         ║")
    print("╚═══════════════════════════════════════════════════════════╝")
    print()

if __name__ == "__main__":
    print_banner()
    ensure_kore_built()
    print(f"  🚀 Server starting on http://127.0.0.1:3000")
    print()
    uvicorn.run(app, host="127.0.0.1", port=3000, log_level="info")
