# syntax=docker/dockerfile:1.7
FROM gradle:8.10.2-jdk21 AS builder
WORKDIR /workspace

COPY settings.gradle.kts build.gradle.kts gradle.properties ./
RUN gradle --no-daemon dependencies > /dev/null 2>&1 || true

COPY src ./src
RUN gradle --no-daemon clean bootJar -x test

FROM gcr.io/distroless/java21-debian12:nonroot
WORKDIR /app

COPY --from=builder /workspace/build/libs/user-service-*.jar /app/app.jar

EXPOSE 8084

USER nonroot
ENTRYPOINT ["java", "-XX:+ExitOnOutOfMemoryError", "-jar", "/app/app.jar"]
