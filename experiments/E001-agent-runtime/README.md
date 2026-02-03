# E001: Agent Runtime Experiment

**Status:** ✅ COMPLETE (Safety) / ⚠️ Latency needs optimization  
**Started:** 2026-02-03  
**Owner:** Kore Team

## Results

| Goal | Target | Actual | Status |
|------|--------|--------|--------|
| Analysis latency | <5ms p95 | ~8ms p95 | ⚠️ Subprocess overhead |
| False negatives | 0% | 0% | ✅ **PASS** |
| Profile enforcement | 100% | 100% | ✅ **PASS** |

### Key Achievement
**Static effect analysis correctly blocks unauthorized IO operations with 0% false negatives.**

### Latency Note
Current latency (~8ms) is due to subprocess spawning. For production:
- Option A: Accept 8ms (still fast for agent use)
- Option B: PyO3 FFI binding to Kore library
- Option C: Long-running Kore daemon with IPC

## Quick Start

```bash
# 1. Build Kore (if needed)
cd ../../
cargo build --release

# 2. Install Python deps
cd experiments/E001-agent-runtime
pip install -r requirements.txt

# 3. Start server
python server.py

# 4. Run tests (in another terminal)
python test_client.py

# 5. Run benchmarks
python benchmark.py
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/profiles` | GET | List capability profiles |
| `/analyze` | POST | Static effect analysis |
| `/execute` | POST | Execute with capability check |
| `/sessions` | POST | Create session |
| `/sessions/{id}` | GET | Get session info |
| `/sessions/{id}` | DELETE | Delete session |

## Capability Profiles

| Profile | Effects Allowed | Use Case |
|---------|-----------------|----------|
| `pure` | None | Pure computation, max safety |
| `read-only` | fs-read, net-read | Data retrieval |
| `local-io` | fs, io, time, env | Local operations |
| `full` | All | Trusted code only |

## Example Usage

### Analyze Code

```bash
curl -X POST http://127.0.0.1:3000/analyze \
  -H "Content-Type: application/json" \
  -d '{"code": "1 2 +"}'
```

Response:
```json
{
  "io_effects": [],
  "pure": true,
  "required_caps": [],
  "analysis_time_us": 1234
}
```

### Execute Code

```bash
curl -X POST http://127.0.0.1:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"code": "[ 1 2 3 4 5 ] sum", "profile": "pure"}'
```

Response:
```json
{
  "result": "15",
  "execution_time_us": 5678,
  "effects_used": []
}
```

### Blocked Execution (Safety Demo)

```bash
curl -X POST http://127.0.0.1:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"code": "\"hello\" print", "profile": "pure"}'
```

Response:
```json
{
  "detail": "Capability denied. Missing: ['io']. Profile 'pure' allows: []"
}
```

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   LLM Agent     │────▶│  Agent Runtime   │────▶│      Kore       │
│   (Client)      │     │  (FastAPI)       │     │   (CLI/Lib)     │
└─────────────────┘     └──────────────────┘     └─────────────────┘
                               │
                               ▼
                        ┌──────────────────┐
                        │  Static Analysis │
                        │  - Effect Infer  │
                        │  - IO Detection  │
                        │  - Purity Check  │
                        └──────────────────┘
```

## Files

```
E001-agent-runtime/
├── EXPERIMENT.md      # Full experiment spec
├── README.md          # This file
├── requirements.txt   # Python dependencies
├── server.py          # FastAPI server
├── benchmark.py       # Benchmark suite
├── test_client.py     # Test client
├── kore/              # Test Kore programs
│   ├── pure_computation.kore
│   ├── io_effects.kore
│   └── agent_safe.kore
├── data/              # Test data
└── results/           # Benchmark results
```

## Success Criteria

1. **Analysis Latency**: p95 < 5ms for typical agent code
2. **Safety**: 0% false negatives (never allow unauthorized effects)
3. **Profiles**: All 4 profiles correctly enforce capabilities

## Notes

- Server runs on port 3000
- Requires Kore built in release mode (`cargo build --release`)
- Results saved to `results/` directory
