import { sleep } from "k6";
import { mixedReadTraffic, summaryOutput } from "./lib/common.js";

export const options = {
  scenarios: {
    high_throughput: {
      executor: "constant-vus",
      vus: Number(__ENV.VUS || 500),
      duration: __ENV.DURATION || "10m",
      gracefulStop: "45s"
    }
  }
};

export default function () {
  mixedReadTraffic({}, { profile: "high-throughput" });
  sleep(Math.random() * 0.2);
}

export function handleSummary(data) {
  return summaryOutput(data, "high-throughput");
}

