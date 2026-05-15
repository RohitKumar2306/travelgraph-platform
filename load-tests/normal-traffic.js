import { sleep } from "k6";
import { mixedReadTraffic, summaryOutput } from "./lib/common.js";

export const options = {
  scenarios: {
    normal_traffic: {
      executor: "constant-vus",
      vus: Number(__ENV.VUS || 100),
      duration: __ENV.DURATION || "5m",
      gracefulStop: "30s"
    }
  }
};

export default function () {
  mixedReadTraffic({}, { profile: "normal" });
  sleep(Math.random() * 0.5);
}

export function handleSummary(data) {
  return summaryOutput(data, "normal-traffic");
}

