import http from "k6/http";
import { sleep } from "k6";
import { Counter } from "k6/metrics";
import { mixedReadTraffic, summaryOutput } from "./lib/common.js";

const chaosToggles = new Counter("chaos_toggles_total");

export const options = {
  scenarios: {
    failure_traffic: {
      executor: "constant-vus",
      vus: Number(__ENV.VUS || 200),
      duration: __ENV.DURATION || "5m",
      gracefulStop: "30s",
      exec: "traffic"
    },
    pricing_latency_toggle: {
      executor: "constant-arrival-rate",
      rate: 1,
      timeUnit: "30s",
      duration: __ENV.DURATION || "5m",
      preAllocatedVUs: 1,
      maxVUs: 1,
      exec: "togglePricingLatency"
    }
  }
};

export function traffic() {
  mixedReadTraffic({}, { profile: "failure-scenario" });
  sleep(Math.random() * 0.35);
}

export function togglePricingLatency() {
  const chaosUrl = __ENV.CHAOS_URL;
  if (!chaosUrl) {
    return;
  }
  const res = http.post(
    chaosUrl,
    JSON.stringify({
      service: "pricing-service",
      latencyMs: Number(__ENV.CHAOS_LATENCY_MS || 500),
      durationSeconds: Number(__ENV.CHAOS_DURATION_SECONDS || 30)
    }),
    { headers: { "content-type": "application/json" }, tags: { operation: "chaos-toggle" } }
  );
  if (res.status >= 200 && res.status < 300) {
    chaosToggles.add(1);
  }
}

export function handleSummary(data) {
  return summaryOutput(data, "failure-scenario");
}

