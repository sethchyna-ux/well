from http.server import BaseHTTPRequestHandler
import json
import logging
import time
from urllib.parse import urlparse, parse_qs

logger = logging.getLogger(__name__)

# In-memory cache with TTL
_cache = {"data": None, "timestamp": 0, "ttl": 300}

class handler(BaseHTTPRequestHandler):
    def do_GET(self):
        try:
            params = parse_qs(urlparse(self.path).query)
            num_agents = int(params.get('agents', [500])[0])
            
            # Validate
            if not 1 <= num_agents <= 10000:
                return self._error(400, "agents must be 1-10000")
            
            # Cache hit?
            if time.time() - _cache["timestamp"] < _cache["ttl"]:
                payload = _cache["data"]
            else:
                # Run both simulations
                logger.info(f"Running simulations ({num_agents} agents)")
                from hydra_scheduler_sim import HydraSchedulerSim
                
                sim_trad = HydraSchedulerSim(num_agents=num_agents, use_hydra=False)
                sim_hydra = HydraSchedulerSim(num_agents=num_agents, use_hydra=True)
                
                payload = {
                    "traditional": extract_metrics(sim_trad.run()),
                    "hydra": extract_metrics(sim_hydra.run()),
                    "timestamp": time.time()
                }
                
                _cache.update({"data": payload, "timestamp": time.time()})
            
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', 'https://chiemera.net')
            self.end_headers()
            self.wfile.write(json.dumps(payload).encode())
            
        except Exception as e:
            logger.exception("Simulation failed")
            self._error(500, f"Simulation error: {e}")
    
    def _error(self, code, msg):
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps({"error": msg}).encode())

def extract_metrics(result):
    return {k: result[k] for k in ["p50", "p90", "p99", "avg_latency", "avg_swap", "avg_wait", "throughput"]}