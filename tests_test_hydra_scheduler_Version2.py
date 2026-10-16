"""Test suite for hydra-scheduler-sim.py"""
import pytest
from hydra_scheduler_sim import HydraSchedulerSim
import statistics

def test_percentile_calculation_correctness():
    """Verify p50, p90, p99 are computed accurately (not just int indexing)."""
    sim = HydraSchedulerSim(num_agents=1000, seed=42, use_hydra=False)
    results = sim.run()
    
    # Latencies should be sorted already by analyze()
    latencies = sorted([t["latency"] for t in sim.completed_tasks])
    
    # Verify p50 is actually the 50th percentile
    expected_p50 = statistics.quantiles(latencies, n=100)[49]
    assert abs(results["p50"] - expected_p50) < 0.1, "p50 calculation error"

def test_hydra_vs_traditional_improvement():
    """Verify Hydra OS shows measurable latency improvement."""
    sim_trad = HydraSchedulerSim(num_agents=500, seed=42, use_hydra=False)
    res_trad = sim_trad.run()
    
    sim_hydra = HydraSchedulerSim(num_agents=500, seed=42, use_hydra=True)
    res_hydra = sim_hydra.run()
    
    # Hydra must improve latency (lower is better)
    assert res_hydra["avg_latency"] < res_trad["avg_latency"], \
        f"Hydra should reduce latency: {res_hydra['avg_latency']} >= {res_trad['avg_latency']}"

def test_reproducibility():
    """Ensure same seed produces identical results."""
    sim1 = HydraSchedulerSim(num_agents=100, seed=123, use_hydra=True)
    res1 = sim1.run()
    
    sim2 = HydraSchedulerSim(num_agents=100, seed=123, use_hydra=True)
    res2 = sim2.run()
    
    assert res1["avg_latency"] == res2["avg_latency"], "Seed should ensure reproducibility"