#!/usr/bin/env python3
"""
Kore Sandbox Worker
===================

Runs inside the Docker container.
Watches /mnt/exchange/programs for new programs.
Executes them and writes results to /mnt/exchange/results.

This worker runs with root privileges inside the container,
giving Kore full capabilities (file access, network, etc.).
"""

import os
import sys
import json
import time
import signal
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import logging

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Paths
EXCHANGE_DIR = Path("/mnt/exchange")
PROGRAMS_DIR = EXCHANGE_DIR / "programs"
RESULTS_DIR = EXCHANGE_DIR / "results"
STATUS_FILE = EXCHANGE_DIR / "status.json"

# Config from environment
KORE_BINARY = os.environ.get("KORE_BINARY", "/usr/local/bin/kore-train")
KORE_CAPABILITIES = os.environ.get("KORE_CAPABILITIES", "all")
KORE_MAX_STEPS = int(os.environ.get("KORE_MAX_STEPS", "100000"))
KORE_TIMEOUT = float(os.environ.get("KORE_TIMEOUT", "5"))
MAX_WORKERS = int(os.environ.get("MAX_WORKERS", "16"))

# Shutdown flag
shutdown_requested = False


def signal_handler(signum, frame):
    global shutdown_requested
    logger.info(f"Received signal {signum}, shutting down...")
    shutdown_requested = True


def update_status(ready: bool, message: str = "", stats: dict = None):
    """Update status file for health checks"""
    status = {
        "ready": ready,
        "message": message,
        "timestamp": time.time(),
        "capabilities": KORE_CAPABILITIES,
        "stats": stats or {},
    }
    STATUS_FILE.write_text(json.dumps(status, indent=2))


def execute_program(request: dict) -> dict:
    """
    Execute a single Kore program.
    
    Args:
        request: {
            "id": str,
            "program": str,
            "max_steps": int,
            "trace": bool,
            "capabilities": list or None,
        }
    
    Returns:
        Result dict to write to results directory.
    """
    request_id = request.get("id", "unknown")
    program = request.get("program", "")
    max_steps = request.get("max_steps", KORE_MAX_STEPS)
    trace = request.get("trace", False)
    
    start = time.time()
    
    # Build command
    cmd = [
        KORE_BINARY,
        "--trace" if trace else "--no-trace",
        "--max-steps", str(max_steps),
    ]
    
    try:
        result = subprocess.run(
            cmd,
            input=program,
            capture_output=True,
            text=True,
            timeout=KORE_TIMEOUT,
        )
        
        elapsed_ms = int((time.time() - start) * 1000)
        
        if result.returncode != 0 and not result.stdout:
            return {
                "id": request_id,
                "success": False,
                "final_stack": [],
                "error": result.stderr.strip() or f"Exit code {result.returncode}",
                "execution_time_ms": elapsed_ms,
            }
        
        try:
            data = json.loads(result.stdout)
            data["id"] = request_id
            data["execution_time_ms"] = elapsed_ms
            return data
        except json.JSONDecodeError:
            return {
                "id": request_id,
                "success": False,
                "final_stack": [],
                "error": f"Invalid JSON output: {result.stdout[:200]}",
                "execution_time_ms": elapsed_ms,
            }
    
    except subprocess.TimeoutExpired:
        return {
            "id": request_id,
            "success": False,
            "final_stack": [],
            "error": "Execution timeout",
            "execution_time_ms": int(KORE_TIMEOUT * 1000),
        }
    except Exception as e:
        return {
            "id": request_id,
            "success": False,
            "final_stack": [],
            "error": str(e),
            "execution_time_ms": int((time.time() - start) * 1000),
        }


def process_program_file(program_file: Path):
    """Process a single program file"""
    try:
        request = json.loads(program_file.read_text())
        request_id = request.get("id", program_file.stem)
        
        # Execute
        result = execute_program(request)
        
        # Write result
        result_file = RESULTS_DIR / f"{request_id}.json"
        result_file.write_text(json.dumps(result))
        
        # Remove program file
        program_file.unlink()
        
        return True
    except Exception as e:
        logger.error(f"Error processing {program_file}: {e}")
        return False


def main():
    global shutdown_requested
    
    # Setup signal handlers
    signal.signal(signal.SIGTERM, signal_handler)
    signal.signal(signal.SIGINT, signal_handler)
    
    # Verify kore binary
    try:
        result = subprocess.run(
            [KORE_BINARY, "--help"],
            capture_output=True,
            timeout=5,
        )
        logger.info(f"Kore binary ready: {KORE_BINARY}")
    except Exception as e:
        logger.error(f"Kore binary not available: {e}")
        update_status(False, "Kore binary not available")
        sys.exit(1)
    
    # Ensure directories exist
    PROGRAMS_DIR.mkdir(parents=True, exist_ok=True)
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    
    # Clear old files
    for f in PROGRAMS_DIR.glob("*.json"):
        f.unlink()
    for f in RESULTS_DIR.glob("*.json"):
        f.unlink()
    
    # Mark ready
    update_status(True, "Worker ready")
    logger.info(f"Worker started with {MAX_WORKERS} threads")
    logger.info(f"Capabilities: {KORE_CAPABILITIES}")
    logger.info(f"Watching: {PROGRAMS_DIR}")
    
    # Stats
    total_executed = 0
    total_success = 0
    
    # Thread pool for parallel execution
    executor = ThreadPoolExecutor(max_workers=MAX_WORKERS)
    
    try:
        while not shutdown_requested:
            # Find program files
            program_files = list(PROGRAMS_DIR.glob("*.json"))
            
            if program_files:
                # Process in parallel
                futures = [
                    executor.submit(process_program_file, pf)
                    for pf in program_files
                ]
                
                for future in futures:
                    try:
                        if future.result(timeout=KORE_TIMEOUT + 1):
                            total_success += 1
                        total_executed += 1
                    except:
                        pass
                
                # Update stats
                update_status(True, "Worker running", {
                    "total_executed": total_executed,
                    "total_success": total_success,
                    "success_rate": total_success / max(total_executed, 1),
                })
            else:
                # No work, sleep briefly
                time.sleep(0.001)  # 1ms poll interval
    
    except KeyboardInterrupt:
        logger.info("Keyboard interrupt")
    
    finally:
        executor.shutdown(wait=False)
        update_status(False, "Worker shutdown")
        logger.info(f"Worker shutdown. Executed {total_executed} programs, {total_success} successful")


if __name__ == "__main__":
    main()
