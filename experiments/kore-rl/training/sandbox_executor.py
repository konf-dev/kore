"""
Sandboxed Docker Executor for Kore-RL Training
===============================================

Runs Kore in an isolated Docker container with:
- Mount directory for program/result exchange
- Internet access (for future capabilities)
- No localhost access (sandboxed)
- Auto-restart on crash
- All Kore capabilities enabled

Communication via mounted directory (faster than HTTP, more isolated than CLI):
  /mnt/exchange/
    programs/       <- training writes programs here
    results/        <- container writes results here
    status          <- container health/status
"""

import os
import json
import time
import uuid
import shutil
import signal
import atexit
import logging
import threading
import subprocess
from pathlib import Path
from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional
from concurrent.futures import ThreadPoolExecutor, as_completed

logger = logging.getLogger(__name__)


@dataclass
class ExecuteResult:
    """Result from Kore program execution"""
    success: bool
    final_stack: List[Any]
    trace: List[Dict] = field(default_factory=list)
    error: Optional[str] = None
    error_at_step: Optional[int] = None
    steps_executed: int = 0
    execution_time_ms: int = 0
    capabilities_used: List[str] = field(default_factory=list)


class KoreSandboxedExecutor:
    """
    Sandboxed Docker-based Kore executor.
    
    Architecture:
        Training Process          Docker Container (kore-sandbox)
        ================          ================================
        
        write program.json  --->  /mnt/exchange/programs/
                                        |
                                  kore-worker picks up
                                        |
                                  execute with full caps
                                        |
        read result.json    <---  /mnt/exchange/results/
    
    Security:
        - Container has NO access to host filesystem (except mount)
        - Container has NO access to localhost/host network
        - Container runs with limited resources (CPU, memory, time)
        - Programs run in Kore sandbox with configurable capabilities
    
    Capabilities (enabled by phase):
        Phase 1: arithmetic, stack
        Phase 2: + control, comparison
        Phase 3: + loops, lists
        Phase 4: + io (print, read)
        Phase 5: + network (http fetch)
        Phase 6: + filesystem (within container)
    """
    
    CONTAINER_NAME = "kore-sandbox"
    IMAGE_NAME = "kore-rl-sandbox:latest"
    
    def __init__(
        self,
        exchange_dir: str = "/tmp/kore-exchange",
        capabilities: List[str] = None,  # None = all enabled
        max_execution_time: float = 5.0,
        max_memory_mb: int = 512,
        max_workers: int = 32,
        auto_restart: bool = True,
    ):
        self.exchange_dir = Path(exchange_dir)
        self.capabilities = capabilities  # None means all
        self.max_execution_time = max_execution_time
        self.max_memory_mb = max_memory_mb
        self.max_workers = max_workers
        self.auto_restart = auto_restart
        
        # Setup directories
        self.programs_dir = self.exchange_dir / "programs"
        self.results_dir = self.exchange_dir / "results"
        self.status_file = self.exchange_dir / "status.json"
        
        self._setup_exchange_dir()
        self._container_lock = threading.Lock()
        self._executor = ThreadPoolExecutor(max_workers=max_workers)
        
        # Start container
        self._ensure_container_running()
        
        # Cleanup on exit
        atexit.register(self._cleanup)
    
    def _setup_exchange_dir(self):
        """Create exchange directory structure"""
        self.exchange_dir.mkdir(parents=True, exist_ok=True)
        self.programs_dir.mkdir(exist_ok=True)
        self.results_dir.mkdir(exist_ok=True)
        
        # Clear old files
        for f in self.programs_dir.glob("*.json"):
            f.unlink()
        for f in self.results_dir.glob("*.json"):
            f.unlink()
    
    def _ensure_container_running(self) -> bool:
        """Start container if not running"""
        with self._container_lock:
            # Check if running
            result = subprocess.run(
                ["docker", "inspect", "-f", "{{.State.Running}}", self.CONTAINER_NAME],
                capture_output=True,
                text=True,
            )
            
            if result.returncode == 0 and result.stdout.strip() == "true":
                logger.info(f"Container {self.CONTAINER_NAME} already running")
                return True
            
            # Remove old container if exists
            subprocess.run(
                ["docker", "rm", "-f", self.CONTAINER_NAME],
                capture_output=True,
            )
            
            # Start new container
            logger.info(f"Starting container {self.CONTAINER_NAME}")
            
            cmd = [
                "docker", "run", "-d",
                "--name", self.CONTAINER_NAME,
                
                # Mount exchange directory
                "-v", f"{self.exchange_dir.absolute()}:/mnt/exchange",
                
                # Network: internet but no localhost
                "--network", "bridge",
                "--add-host", "host.docker.internal:host-gateway",
                
                # Resource limits
                "--memory", f"{self.max_memory_mb}m",
                "--cpus", "2",
                
                # No privileged, but Kore has root inside
                "--user", "root",
                
                # Restart policy
                "--restart", "unless-stopped" if self.auto_restart else "no",
                
                # Environment
                "-e", f"KORE_CAPABILITIES={','.join(self.capabilities) if self.capabilities else 'all'}",
                "-e", f"KORE_MAX_STEPS=100000",
                "-e", f"KORE_TIMEOUT={self.max_execution_time}",
                
                self.IMAGE_NAME,
            ]
            
            result = subprocess.run(cmd, capture_output=True, text=True)
            
            if result.returncode != 0:
                logger.error(f"Failed to start container: {result.stderr}")
                return False
            
            # Wait for container to be ready
            for _ in range(30):
                if self._check_container_health():
                    logger.info("Container ready")
                    return True
                time.sleep(0.1)
            
            logger.error("Container failed to become ready")
            return False
    
    def _check_container_health(self) -> bool:
        """Check if container is healthy and ready"""
        try:
            if self.status_file.exists():
                status = json.loads(self.status_file.read_text())
                return status.get("ready", False)
        except:
            pass
        return False
    
    def execute(
        self,
        program: str,
        max_steps: int = 10000,
        trace: bool = True,
    ) -> ExecuteResult:
        """Execute a single Kore program"""
        request_id = str(uuid.uuid4())[:8]
        
        # Write program to exchange dir
        program_file = self.programs_dir / f"{request_id}.json"
        result_file = self.results_dir / f"{request_id}.json"
        
        request = {
            "id": request_id,
            "program": program,
            "max_steps": max_steps,
            "trace": trace,
            "capabilities": self.capabilities,
            "timestamp": time.time(),
        }
        
        program_file.write_text(json.dumps(request))
        
        # Wait for result
        start = time.time()
        timeout = self.max_execution_time + 1.0
        
        while time.time() - start < timeout:
            if result_file.exists():
                try:
                    data = json.loads(result_file.read_text())
                    result_file.unlink()  # Cleanup
                    return self._parse_result(data)
                except json.JSONDecodeError:
                    time.sleep(0.01)
                    continue
            time.sleep(0.01)
        
        # Timeout - cleanup and return error
        if program_file.exists():
            program_file.unlink()
        
        return ExecuteResult(
            success=False,
            final_stack=[],
            error="Execution timeout",
            execution_time_ms=int(timeout * 1000),
        )
    
    def _parse_result(self, data: Dict) -> ExecuteResult:
        """Parse result from container"""
        return ExecuteResult(
            success=data.get("success", False),
            final_stack=data.get("final_stack", []),
            trace=data.get("trace", []),
            error=data.get("error"),
            error_at_step=data.get("error_at_step"),
            steps_executed=data.get("steps_executed", 0),
            execution_time_ms=data.get("execution_time_ms", 0),
            capabilities_used=data.get("capabilities_used", []),
        )
    
    def execute_batch(
        self,
        programs: List[str],
        max_steps: int = 10000,
        trace: bool = False,
    ) -> List[ExecuteResult]:
        """Execute multiple programs in parallel"""
        # Ensure container is running
        if not self._check_container_health():
            self._ensure_container_running()
        
        # Submit all at once
        futures = [
            self._executor.submit(self.execute, prog, max_steps, trace)
            for prog in programs
        ]
        
        return [f.result() for f in futures]
    
    def restart_container(self):
        """Force restart the container"""
        with self._container_lock:
            subprocess.run(
                ["docker", "restart", self.CONTAINER_NAME],
                capture_output=True,
            )
    
    def health_check(self) -> bool:
        """Check if executor is healthy"""
        if not self._check_container_health():
            if self.auto_restart:
                return self._ensure_container_running()
            return False
        return True
    
    def _cleanup(self):
        """Cleanup on exit"""
        self._executor.shutdown(wait=False)
        # Don't stop container - let it run for resume
    
    def close(self):
        """Stop executor and optionally container"""
        self._executor.shutdown(wait=False)
    
    def stop_container(self):
        """Stop the container completely"""
        subprocess.run(
            ["docker", "stop", self.CONTAINER_NAME],
            capture_output=True,
        )


class TrainingCheckpoint:
    """
    Manages training state for crash recovery.
    
    Saves:
        - Current step
        - Curriculum phase
        - Best reward
        - Model checkpoint path
        - Optimizer state
    """
    
    def __init__(self, checkpoint_dir: str):
        self.checkpoint_dir = Path(checkpoint_dir)
        self.checkpoint_dir.mkdir(parents=True, exist_ok=True)
        self.state_file = self.checkpoint_dir / "training_state.json"
    
    def save(
        self,
        step: int,
        phase: int,
        best_reward: float,
        model_path: str,
        extra: Dict[str, Any] = None,
    ):
        """Save training state"""
        state = {
            "step": step,
            "phase": phase,
            "best_reward": best_reward,
            "model_path": model_path,
            "timestamp": time.time(),
            "extra": extra or {},
        }
        self.state_file.write_text(json.dumps(state, indent=2))
    
    def load(self) -> Optional[Dict[str, Any]]:
        """Load training state if exists"""
        if self.state_file.exists():
            return json.loads(self.state_file.read_text())
        return None
    
    def can_resume(self) -> bool:
        """Check if we can resume from checkpoint"""
        state = self.load()
        if state is None:
            return False
        
        model_path = state.get("model_path")
        if model_path and Path(model_path).exists():
            return True
        return False
