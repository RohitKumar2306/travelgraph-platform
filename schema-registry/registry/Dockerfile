# syntax=docker/dockerfile:1.7
FROM node:20-bookworm-slim AS node-helper
WORKDIR /helper
COPY src/main/resources/node-helper/package.json ./
RUN npm install --omit=dev
COPY src/main/resources/node-helper/compose.mjs ./

FROM gradle:8.10.2-jdk21 AS builder
WORKDIR /workspace
COPY settings.gradle.kts build.gradle.kts ./
RUN gradle --no-daemon dependencies > /dev/null 2>&1 || true
COPY src ./src
RUN gradle --no-daemon clean bootJar -x test

FROM eclipse-temurin:21-jre-jammy
WORKDIR /app
COPY --from=node-helper /usr/local/bin/node /usr/local/bin/node
COPY --from=builder /workspace/build/libs/travelgraph-schema-registry-*.jar /app/app.jar
COPY --from=node-helper /helper /app/node-helper
EXPOSE 8090
ENTRYPOINT ["java", "-XX:+ExitOnOutOfMemoryError", "-jar", "/app/app.jar"]
