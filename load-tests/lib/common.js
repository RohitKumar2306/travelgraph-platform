import http from "k6/http";
import { check } from "k6";
import crypto from "k6/crypto";
import encoding from "k6/encoding";
import { Counter, Rate } from "k6/metrics";

export const routerUrl = __ENV.ROUTER_URL || "http://localhost:8080/graphql";
export const prometheusUrl = __ENV.PROMETHEUS_URL || "http://localhost:9090";
export const resultsDir = __ENV.RESULTS_DIR || "load-tests/results";

export const httpErrorRate = new Rate("graphql_http_error_rate");
export const graphqlErrorRate = new Rate("graphql_error_rate");
export const successfulGraphqlRequests = new Counter("graphql_success_total");

export const propertyIds = [
  "11111111-1111-1111-1111-000000000001",
  "11111111-1111-1111-1111-000000000002",
  "22222222-2222-2222-2222-000000000001",
  "33333333-3333-3333-3333-000000000001",
  "44444444-4444-4444-4444-000000000002",
  "55555555-5555-5555-5555-000000000001"
];

export const userIds = [
  "80000000-0000-0000-0000-000000000001",
  "80000000-0000-0000-0000-000000000003",
  "80000000-0000-0000-0000-000000000004",
  "80000000-0000-0000-0000-000000000008"
];

export const cities = ["Austin", "Seattle", "Lisbon", "Tokyo", "Cape Town"];
export const tiers = ["BRONZE", "SILVER", "GOLD", "PLATINUM"];

export const queries = {
  propertySearch: `query PropertySearch($city: String!, $limit: Int!) {
    searchProperties(city: $city, limit: $limit) {
      id
      name
      city
      rating
      price { totalAmount currency }
      reviewSummary { count averageRating }
    }
  }`,
  priceOnly: `query PriceOnly($propertyId: ID!, $checkIn: String!, $checkOut: String!, $tier: String) {
    price(propertyId: $propertyId, checkIn: $checkIn, checkOut: $checkOut, loyaltyTier: $tier) {
      propertyId
      totalAmount
      currency
      nights
    }
  }`,
  propertyDetails: `query PropertyDetails($propertyId: ID!) {
    property(id: $propertyId) {
      id
      name
      description
      rating
      price { totalAmount currency }
      reviews(limit: 5) { id rating comment }
    }
  }`,
  bookings: `query Bookings($userId: ID!) {
    bookings(userId: $userId) {
      id
      propertyId
      userId
      status
      checkIn
      checkOut
      totalAmount
      currency
    }
  }`,
  availableRooms: `query AvailableRooms($propertyId: ID!, $checkIn: String!, $checkOut: String!) {
    availableRooms(propertyId: $propertyId, checkIn: $checkIn, checkOut: $checkOut) {
      code
      name
      capacity
      available
    }
  }`,
  me: `query Viewer {
    me {
      id
      email
      loyaltyStatus
      preferredCurrency
    }
  }`
};

export function pick(values) {
  return values[Math.floor(Math.random() * values.length)];
}

export function dateWindow() {
  const start = new Date(Date.UTC(2026, 5, 10 + Math.floor(Math.random() * 120)));
  const end = new Date(start.getTime() + (1 + Math.floor(Math.random() * 5)) * 86400000);
  return {
    checkIn: start.toISOString().slice(0, 10),
    checkOut: end.toISOString().slice(0, 10)
  };
}

function base64Url(value) {
  return encoding.b64encode(value, "rawurl");
}

export function jwtFor(userId = pick(userIds), tier = pick(tiers)) {
  const secret = __ENV.JWT_SECRET || "travelgraph-dev-jwt-secret";
  const email = `k6-${userId.slice(-4)}@example.com`;
  const header = base64Url(JSON.stringify({ alg: "HS256", typ: "JWT" }));
  const payload = base64Url(JSON.stringify({ sub: userId, loyalty_tier: tier, email }));
  const signature = crypto.hmac("sha256", secret, `${header}.${payload}`, "base64rawurl");
  return `${header}.${payload}.${signature}`;
}

export function graphql(operationName, query, variables = {}, extraHeaders = {}, tags = {}) {
  const token = jwtFor();
  const res = http.post(
    routerUrl,
    JSON.stringify({ operationName, query, variables }),
    {
      headers: Object.assign({
        "content-type": "application/json",
        "authorization": `Bearer ${token}`,
        "apollographql-client-name": __ENV.CLIENT_NAME || "k6-load-test",
        "apollographql-client-version": __ENV.CLIENT_VERSION || "phase-10"
      }, extraHeaders),
      tags: Object.assign({ operation: operationName }, tags)
    }
  );
  const httpOk = res.status === 200;
  httpErrorRate.add(!httpOk);

  let gqlOk = false;
  try {
    const body = res.json();
    gqlOk = httpOk && body && body.data && (!body.errors || body.errors.length === 0);
  } catch (_) {
    gqlOk = false;
  }
  graphqlErrorRate.add(!gqlOk);
  if (gqlOk) successfulGraphqlRequests.add(1);

  check(res, {
    "status is 200": () => httpOk,
    "GraphQL response has data": () => gqlOk
  });
  return res;
}

function metricValue(data, metric, key) {
  return data.metrics[metric] && data.metrics[metric].values
    ? data.metrics[metric].values[key]
    : null;
}

export function resultSummary(data, scenario) {
  const cacheHits = metricValue(data, "graphql_cache_hits_total", "count");
  const cacheMisses = metricValue(data, "graphql_cache_misses_total", "count");
  const cacheTotal = cacheHits !== null && cacheMisses !== null ? cacheHits + cacheMisses : null;
  return {
    scenario,
    measuredAt: new Date().toISOString(),
    target: routerUrl,
    vusMax: data.state ? data.state.vusMax : null,
    throughputRps: metricValue(data, "http_reqs", "rate"),
    requests: metricValue(data, "http_reqs", "count"),
    latencyMs: {
      p50: metricValue(data, "http_req_duration", "p(50)"),
      p95: metricValue(data, "http_req_duration", "p(95)"),
      p99: metricValue(data, "http_req_duration", "p(99)")
    },
    errorRates: {
      http: metricValue(data, "graphql_http_error_rate", "rate"),
      graphql: metricValue(data, "graphql_error_rate", "rate"),
      checks: metricValue(data, "checks", "rate")
    },
    cache: {
      hits: cacheHits,
      misses: cacheMisses,
      hitRatio: cacheTotal && cacheTotal > 0 ? cacheHits / cacheTotal : null,
      prometheusUrl
    }
  };
}

export function summaryOutput(data, scenario) {
  const file = `${resultsDir}/${scenario}-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  const summary = resultSummary(data, scenario);
  return {
    [file]: JSON.stringify(summary, null, 2),
    stdout: `${scenario}: ${summary.requests || 0} requests, p95=${summary.latencyMs.p95}ms, GraphQL error rate=${summary.errorRates.graphql}\n`
  };
}

export function mixedReadTraffic(extraHeaders = {}, tags = {}) {
  const selector = Math.random();
  const window = dateWindow();
  if (selector < 0.35) {
    return graphql("PropertySearch", queries.propertySearch, { city: pick(cities), limit: 10 }, extraHeaders, tags);
  }
  if (selector < 0.6) {
    return graphql("PropertyDetails", queries.propertyDetails, { propertyId: pick(propertyIds) }, extraHeaders, tags);
  }
  if (selector < 0.78) {
    return graphql(
      "PriceOnly",
      queries.priceOnly,
      { propertyId: pick(propertyIds), checkIn: window.checkIn, checkOut: window.checkOut, tier: pick(tiers) },
      extraHeaders,
      tags
    );
  }
  if (selector < 0.9) {
    return graphql(
      "AvailableRooms",
      queries.availableRooms,
      Object.assign({ propertyId: pick(propertyIds) }, window),
      extraHeaders,
      tags
    );
  }
  return graphql("Bookings", queries.bookings, { userId: pick(userIds) }, extraHeaders, tags);
}
