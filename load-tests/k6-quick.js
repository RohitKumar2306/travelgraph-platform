import http from "k6/http";
import { check, sleep } from "k6";

export const options = {
  scenarios: {
    graphql_quick: {
      executor: "constant-arrival-rate",
      rate: 15,
      timeUnit: "1s",
      duration: "30s",
      preAllocatedVUs: 10,
      maxVUs: 30
    }
  }
};

const url = __ENV.ROUTER_URL || "http://localhost:8080/graphql";
const payload = JSON.stringify({
  operationName: "ObservabilityQuick",
  query: `query ObservabilityQuick {
    searchProperties(city: "Austin", limit: 5) {
      id
      name
      rating
      price { totalAmount currency }
      reviewSummary { count averageRating }
    }
  }`
});

export default function () {
  const res = http.post(url, payload, {
    headers: {
      "content-type": "application/json",
      "apollographql-client-name": "k6-quick",
      "apollographql-client-version": "phase-6"
    }
  });
  check(res, {
    "status is 200": (r) => r.status === 200,
    "has data": (r) => r.body && r.body.includes('"data"')
  });
  sleep(1);
}
