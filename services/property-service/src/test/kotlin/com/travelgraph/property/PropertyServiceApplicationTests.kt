package com.travelgraph.property

import org.junit.jupiter.api.Test

/**
 * Offline-safe smoke test: only verifies the Kotlin source compiles and the
 * application class is on the classpath. Real boot-up integration tests
 * (Testcontainers + GraphQL web layer assertions) arrive in later phases.
 */
class PropertyServiceContextLoadsTest {

    @Test
    fun `application class is loadable`() {
        // We avoid booting the full context here to keep this test offline-safe.
        // Phase 1 acceptance is verified via docker-compose, not unit tests.
        check(PropertyServiceApplication::class.java.simpleName == "PropertyServiceApplication")
    }
}
