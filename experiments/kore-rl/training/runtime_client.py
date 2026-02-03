"""
Kore Runtime Client
===================

HTTP client for communicating with the Kore runtime container.
"""

import httpx
from dataclasses import dataclass, field
from typing import List, Optional, Any, Dict
import json
import asyncio
from concurrent.futures import ThreadPoolExecutor


@dataclass
class TraceEntry:
    """Single execution step"""
    step: int
    op: str
    op_type: str  # "push" or "call"
    stack_before: List[Any]
    stack_after: List[Any]
    success: bool
    error: Optional[str] = None


@dataclass
class ExecuteResult:
    """Result of executing a Kore program"""
    success: bool
    final_stack: List[Any]
    trace: List[TraceEntry]
    error: Optional[str] = None
    error_at_step: Optional[int] = None
    steps_executed: int = 0
    execution_time_ms: int = 0


class KoreRuntimeClient:
    """
    Client for Kore Runtime container.
    
    Usage:
        client = KoreRuntimeClient("http://localhost:8080")
        result = client.execute("3 4 add")
        print(result.final_stack)  # [7]
    """
    
    def __init__(
        self,
        base_url: str = "http://localhost:8080",
        timeout: float = 5.0,
        max_retries: int = 3,
    ):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.max_retries = max_retries
        self._client = httpx.Client(timeout=timeout)
        self._async_client = None
        self._executor = ThreadPoolExecutor(max_workers=32)
    
    def execute(
        self,
        program: str,
        initial_stack: List[Any] = None,
        max_steps: int = 10000,
        timeout_ms: int = 5000,
    ) -> ExecuteResult:
        """Execute a Kore program synchronously."""
        payload = {
            "program": program,
            "initial_stack": initial_stack or [],
            "max_steps": max_steps,
            "timeout_ms": timeout_ms,
        }
        
        for attempt in range(self.max_retries):
            try:
                response = self._client.post(
                    f"{self.base_url}/execute",
                    json=payload,
                )
                response.raise_for_status()
                return self._parse_response(response.json())
            except httpx.HTTPStatusError as e:
                if attempt == self.max_retries - 1:
                    return ExecuteResult(
                        success=False,
                        final_stack=[],
                        trace=[],
                        error=f"HTTP error: {e.response.status_code}",
                    )
            except httpx.RequestError as e:
                if attempt == self.max_retries - 1:
                    return ExecuteResult(
                        success=False,
                        final_stack=[],
                        trace=[],
                        error=f"Request error: {str(e)}",
                    )
        
        return ExecuteResult(
            success=False,
            final_stack=[],
            trace=[],
            error="Max retries exceeded",
        )
    
    async def execute_async(
        self,
        program: str,
        initial_stack: List[Any] = None,
        max_steps: int = 10000,
        timeout_ms: int = 5000,
    ) -> ExecuteResult:
        """Execute a Kore program asynchronously."""
        if self._async_client is None:
            self._async_client = httpx.AsyncClient(timeout=self.timeout)
        
        payload = {
            "program": program,
            "initial_stack": initial_stack or [],
            "max_steps": max_steps,
            "timeout_ms": timeout_ms,
        }
        
        try:
            response = await self._async_client.post(
                f"{self.base_url}/execute",
                json=payload,
            )
            response.raise_for_status()
            return self._parse_response(response.json())
        except Exception as e:
            return ExecuteResult(
                success=False,
                final_stack=[],
                trace=[],
                error=str(e),
            )
    
    def execute_batch(
        self,
        programs: List[str],
        initial_stacks: Optional[List[List[Any]]] = None,
    ) -> List[ExecuteResult]:
        """Execute multiple programs in parallel."""
        if initial_stacks is None:
            initial_stacks = [[] for _ in programs]
        
        async def run_all():
            tasks = [
                self.execute_async(prog, stack)
                for prog, stack in zip(programs, initial_stacks)
            ]
            return await asyncio.gather(*tasks)
        
        loop = asyncio.new_event_loop()
        try:
            results = loop.run_until_complete(run_all())
        finally:
            loop.close()
        
        return results
    
    def _parse_response(self, data: Dict) -> ExecuteResult:
        """Parse JSON response into ExecuteResult."""
        trace = [
            TraceEntry(
                step=entry["step"],
                op=entry["op"],
                op_type=entry["op_type"],
                stack_before=entry["stack_before"],
                stack_after=entry["stack_after"],
                success=entry["success"],
                error=entry.get("error"),
            )
            for entry in data.get("trace", [])
        ]
        
        return ExecuteResult(
            success=data["success"],
            final_stack=data["final_stack"],
            trace=trace,
            error=data.get("error"),
            error_at_step=data.get("error_at_step"),
            steps_executed=data.get("steps_executed", 0),
            execution_time_ms=data.get("execution_time_ms", 0),
        )
    
    def health_check(self) -> bool:
        """Check if runtime is healthy."""
        try:
            response = self._client.get(f"{self.base_url}/health")
            return response.status_code == 200
        except:
            return False
    
    def close(self):
        """Close client connections."""
        self._client.close()
        if self._async_client:
            asyncio.get_event_loop().run_until_complete(self._async_client.aclose())
        self._executor.shutdown()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()


class KoreRuntimePool:
    """
    Pool of Kore runtime clients for parallel execution.
    
    Distributes work across multiple container instances.
    """
    
    def __init__(self, urls: List[str]):
        self.clients = [KoreRuntimeClient(url) for url in urls]
        self._current = 0
    
    def execute(self, program: str, **kwargs) -> ExecuteResult:
        """Round-robin execution across pool."""
        client = self.clients[self._current]
        self._current = (self._current + 1) % len(self.clients)
        return client.execute(program, **kwargs)
    
    def execute_batch(self, programs: List[str]) -> List[ExecuteResult]:
        """Distribute batch across all clients."""
        # Split programs across clients
        n = len(self.clients)
        chunks = [programs[i::n] for i in range(n)]
        
        all_results = []
        for client, chunk in zip(self.clients, chunks):
            if chunk:
                results = client.execute_batch(chunk)
                all_results.extend(results)
        
        return all_results
    
    def close(self):
        for client in self.clients:
            client.close()


# Convenience function
def create_client(url: str = "http://localhost:8080") -> KoreRuntimeClient:
    """Create a Kore runtime client."""
    return KoreRuntimeClient(url)
