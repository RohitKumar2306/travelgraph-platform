# syntax=docker/dockerfile:1.7
# ---- Build stage --------------------------------------------------------
# Uses the official Gradle image with JDK 21 to compile and package the
# Spring Boot fat jar. We don't ship the Gradle wrapper jar; the image
# already provides Gradle and is reproducible via the pinned tag.
FROM gradle:8.10.2-jdk21 AS builder
WORKDIR /workspace

# Cache deps separately from sources for faster rebuilds.
COPY settings.gradle.kts build.gradle.kts gradle.properties ./
RUN gradle --no-daemon dependencies > /dev/null 2>&1 || true

COPY src ./src
RUN gradle --no-daemon clean bootJar -x test

# ---- Runtime stage ------------------------------------------------------
# Distroless gives us no shell, no package manager, minimal CVE surface.
# The :nonroot variant runs as UID 65532. Healthcheck is performed
# externally (compose / k8s) via /actuator/health since distroless lacks curl.
FROM gcr.io/distroless/java21-debian12:nonroot
WORKDIR /app

COPY --from=builder /workspace/build/libs/property-service-*.jar /app/app.jar

EXPOSE 8081

USER nonroot
ENTRYPOINT ["java", "-XX:+ExitOnOutOfMemoryError", "-jar", "/app/app.jar"]
