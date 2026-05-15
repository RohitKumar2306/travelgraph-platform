import { group, sleep } from "k6";
import { mixedReadTraffic, summaryOutput } from "./lib/common.js";

export const options = {
  scenarios: {
    cache_comparison: {
      executor: "constant-vus",
      vus: Number(__ENV.VUS || 100),
      duration: __ENV.DURATION || "5m",
      gracefulStop: "30s"
    }
  }
};

export default function () {
  group("cache bypassed", () => {
    mixedReadTraffic({ "x-bypass-cache": "true" }, { cache_mode: "bypassed" });
  });
  group("cache enabled", () => {
    mixedReadTraffic({}, { cache_mode: "enabled" });
  });
  sleep(Math.random() * 0.25);
}

export function handleSummary(data) {
  return summaryOutput(data, "cache-comparison");
}

